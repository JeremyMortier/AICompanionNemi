use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{Level, error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::activity::classify_activity;
use crate::config::AppConfig;
use crate::context::ContextInterpretation;
use crate::decision::{ReactionDecision, decide_reaction};
use crate::events::{AppEvent, EventBus};
use crate::llm::LlmClient;
use crate::mood::MoodState;
use crate::reaction::GeneratedReaction;
use crate::server::{SharedSnapshot, run_server};
use crate::snapshot::{ActiveWindowSnapshot, AppSnapshot, InterpretationSnapshot, MoodSnapshot};
use crate::state::{ActiveWindowState, AppState};
use crate::tick::run_tick;

pub async fn run() -> Result<()> {
    init_tracing();

    let config = AppConfig::default();
    let mut state = AppState::new();
    let mut event_bus = EventBus::new();
    let llm = LlmClient::new(
        "http://127.0.0.1:11434".to_string(),
        "gemma3:4b".to_string(),
    );

    let shared_snapshot: SharedSnapshot = Arc::new(RwLock::new(build_snapshot(&state, &config)));

    {
        let server_snapshot = Arc::clone(&shared_snapshot);
        tokio::spawn(async move {
            if let Err(err) = run_server(server_snapshot).await {
                error!(error = %err, "server task failed");
            }
        });
    }

    info!("Starting core-daemon...");
    info!("Companion name: {}", config.companion_name);
    info!("Tick interval: {} ms", config.tick_interval_ms);
    info!("Local API available at http://127.0.0.1:7878/state");

    let mut interval = tokio::time::interval(Duration::from_millis(config.tick_interval_ms));

    loop {
        interval.tick().await;
        event_bus.push(AppEvent::Tick);
        process_events(&mut event_bus, &mut state, &config, &llm, &shared_snapshot).await;
    }
}

async fn process_events(
    event_bus: &mut EventBus,
    state: &mut AppState,
    config: &AppConfig,
    llm: &LlmClient,
    shared_snapshot: &SharedSnapshot,
) {
    while let Some(event) = event_bus.pop() {
        match event {
            AppEvent::Tick => {
                run_tick(state, config, event_bus);
            }
            AppEvent::ActiveWindowDetected {
                title,
                process_id,
                process_name,
            } => {
                let now = Instant::now();
                let activity = classify_activity(&process_name, &title);

                match &mut state.active_window {
                    Some(current) => {
                        let is_same = current.title == title
                            && current.process_id == process_id
                            && current.process_name == process_name;

                        if is_same {
                            current.last_seen_at = now;

                            let stable_for_ms =
                                now.duration_since(current.first_seen_at).as_millis();

                            info!(
                                tick = state.tick_count,
                                process_name = %process_name,
                                activity = ?current.activity,
                                title = %title,
                                stable_for_ms = stable_for_ms,
                                "window still active"
                            );

                            let mut next_mood = state.mood.clone();
                            next_mood.update_from_activity(&current.activity, stable_for_ms);
                            event_bus.push(AppEvent::MoodUpdated(next_mood));

                            if should_request_interpretation_for_current_window(
                                config,
                                current,
                                stable_for_ms,
                                now,
                            ) {
                                current.last_interpretation_requested_at = Some(now);

                                event_bus.push(AppEvent::ContextInterpretationRequested {
                                    title: current.title.clone(),
                                    process_name: current.process_name.clone(),
                                    heuristic_activity: current.activity.clone(),
                                    stable_for_ms,
                                });
                            }
                        } else {
                            state.active_window = Some(ActiveWindowState {
                                title: title.clone(),
                                process_id,
                                process_name: process_name.clone(),
                                activity: activity.clone(),
                                first_seen_at: now,
                                last_seen_at: now,
                                last_interpretation_requested_at: None,
                            });

                            let mut next_mood = state.mood.clone();
                            next_mood.update_from_activity(&activity, 0);
                            event_bus.push(AppEvent::MoodUpdated(next_mood));

                            info!(
                                tick = state.tick_count,
                                process_name = %process_name,
                                activity = ?activity,
                                title = %title,
                                "active window changed"
                            );
                        }
                    }
                    None => {
                        state.active_window = Some(ActiveWindowState {
                            title: title.clone(),
                            process_id,
                            process_name: process_name.clone(),
                            activity: activity.clone(),
                            first_seen_at: now,
                            last_seen_at: now,
                            last_interpretation_requested_at: None,
                        });

                        let mut next_mood = state.mood.clone();
                        next_mood.update_from_activity(&activity, 0);
                        event_bus.push(AppEvent::MoodUpdated(next_mood));

                        info!(
                            tick = state.tick_count,
                            process_name = %process_name,
                            activity = ?activity,
                            title = %title,
                            "initial active window detected"
                        );
                    }
                }
            }
            AppEvent::MoodUpdated(new_mood) => {
                handle_mood_updated(state, new_mood);
            }
            AppEvent::ContextInterpretationRequested {
                title,
                process_name,
                heuristic_activity,
                stable_for_ms,
            } => {
                info!(
                    process_name = %process_name,
                    title = %title,
                    activity = ?heuristic_activity,
                    stable_for_ms = stable_for_ms,
                    "requesting context interpretation"
                );

                match llm
                    .interpret_context(&process_name, &title, &heuristic_activity, stable_for_ms)
                    .await
                {
                    Ok(result) => {
                        event_bus.push(AppEvent::ContextInterpreted {
                            interpretation: result,
                            stable_for_ms,
                        });
                    }
                    Err(err) => {
                        error!(error = %err, "failed to interpret context with llm");
                    }
                }
            }
            AppEvent::ContextInterpreted {
                interpretation,
                stable_for_ms,
            } => {
                handle_interpreted_context(state, event_bus, config, interpretation, stable_for_ms);
            }
            AppEvent::ReactionDecisionMade(decision) => {
                handle_reaction_decision(state, event_bus, decision);
            }
            AppEvent::ReactionGenerationRequested {
                decision,
                interpretation,
                recent_reactions,
                mood,
            } => {
                match llm
                    .generate_reaction(
                        &interpretation,
                        &decision,
                        &recent_reactions,
                        &config.persona,
                        &mood,
                    )
                    .await
                {
                    Ok(generated) => {
                        event_bus.push(AppEvent::ReactionGenerated(generated));
                    }
                    Err(err) => {
                        error!(error = %err, "failed to generate reaction with llm");
                    }
                }
            }
            AppEvent::ReactionGenerated(generated) => {
                handle_generated_reaction(state, generated);
            }
            AppEvent::ScreenCaptured {
                path,
                width,
                height,
            } => {
                info!(
                    path = %path,
                    width = width,
                    height = height,
                    "screen captured"
                );

                state.last_screen_capture_path = Some(path);
            }
        }

        sync_snapshot(shared_snapshot, state, config).await;
    }
}

fn handle_mood_updated(state: &mut AppState, new_mood: MoodState) {
    let changed = std::mem::discriminant(&state.mood.current)
        != std::mem::discriminant(&new_mood.current)
        || state.mood.intensity != new_mood.intensity;

    if changed {
        info!(
            mood = ?new_mood.current,
            intensity = new_mood.intensity,
            "mood updated"
        );
    }

    state.mood = new_mood;
}

fn handle_interpreted_context(
    state: &mut AppState,
    event_bus: &mut EventBus,
    config: &AppConfig,
    interpretation: ContextInterpretation,
    stable_for_ms: u128,
) {
    info!(
        activity = ?interpretation.activity,
        confidence = interpretation.confidence,
        should_comment = interpretation.should_comment,
        summary = %interpretation.summary,
        "context interpreted"
    );

    state.last_interpretation = Some(interpretation.clone());

    let decision = decide_reaction(
        state,
        config,
        &interpretation,
        stable_for_ms,
        Instant::now(),
    );

    event_bus.push(AppEvent::ReactionDecisionMade(decision));
}

fn handle_reaction_decision(
    state: &mut AppState,
    event_bus: &mut EventBus,
    decision: ReactionDecision,
) {
    match &decision {
        ReactionDecision::StaySilent { reason } => {
            info!(reason = %reason, "reaction decision: stay silent");
        }
        ReactionDecision::LightComment { reason } => {
            info!(reason = %reason, "reaction decision: light comment");
            state.last_reaction_at = Some(Instant::now());

            if let Some(interpretation) = state.last_interpretation.clone() {
                event_bus.push(AppEvent::ReactionGenerationRequested {
                    decision: decision.clone(),
                    interpretation,
                    recent_reactions: state.recent_reaction_memory.recent_texts(),
                    mood: state.mood.clone(),
                });
            }
        }
        ReactionDecision::CuriousComment { reason } => {
            info!(reason = %reason, "reaction decision: curious comment");
            state.last_reaction_at = Some(Instant::now());

            if let Some(interpretation) = state.last_interpretation.clone() {
                event_bus.push(AppEvent::ReactionGenerationRequested {
                    decision: decision.clone(),
                    interpretation,
                    recent_reactions: state.recent_reaction_memory.recent_texts(),
                    mood: state.mood.clone(),
                });
            }
        }
    }

    state.last_decision = Some(decision);
}

fn handle_generated_reaction(state: &mut AppState, generated: GeneratedReaction) {
    if state.recent_reaction_memory.is_too_similar(&generated.text) {
        warn!(
            reaction = %generated.text,
            "generated reaction dropped because it is too similar to recent history"
        );
        return;
    }

    info!(reaction = %generated.text, "generated reaction");

    state.last_generated_reaction = Some(generated.clone());
    state.recent_reaction_memory.push(generated);
}

fn should_request_interpretation(
    config: &AppConfig,
    activity: &crate::activity::UserActivity,
    stable_for_ms: u128,
) -> bool {
    if stable_for_ms < config.interpretation_threshold_ms {
        return false;
    }

    match activity {
        crate::activity::UserActivity::Unknown => true,
        crate::activity::UserActivity::Browsing => true,
        crate::activity::UserActivity::Watching => true,
        crate::activity::UserActivity::Coding => true,
        crate::activity::UserActivity::Chatting => false,
        crate::activity::UserActivity::Gaming => true,
    }
}

async fn sync_snapshot(shared_snapshot: &SharedSnapshot, state: &AppState, config: &AppConfig) {
    let snapshot = build_snapshot(state, config);
    let mut guard = shared_snapshot.write().await;
    *guard = snapshot;
}

fn build_snapshot(state: &AppState, config: &AppConfig) -> AppSnapshot {
    AppSnapshot {
        companion_name: config.companion_name.clone(),
        tick_count: state.tick_count,
        active_window: state
            .active_window
            .as_ref()
            .map(|window| ActiveWindowSnapshot {
                title: window.title.clone(),
                process_id: window.process_id,
                process_name: window.process_name.clone(),
                activity: format!("{:?}", window.activity),
            }),
        last_interpretation: state.last_interpretation.as_ref().map(|interp| {
            InterpretationSnapshot {
                activity: format!("{:?}", interp.activity),
                confidence: interp.confidence,
                summary: interp.summary.clone(),
                should_comment: interp.should_comment,
            }
        }),
        last_decision: state.last_decision.as_ref().map(|d| format!("{:?}", d)),
        last_generated_reaction: state
            .last_generated_reaction
            .as_ref()
            .map(|r| r.text.clone()),
        mood: MoodSnapshot {
            current: format!("{:?}", state.mood.current),
            intensity: state.mood.intensity,
        },
        last_screen_capture_path: state.last_screen_capture_path.clone(),
    }
}

fn init_tracing() {
    let filter = EnvFilter::builder()
        .with_default_directive(Level::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

fn should_request_interpretation_for_current_window(
    config: &AppConfig,
    current: &ActiveWindowState,
    stable_for_ms: u128,
    now: Instant,
) -> bool {
    if !should_request_interpretation(config, &current.activity, stable_for_ms) {
        return false;
    }

    match current.last_interpretation_requested_at {
        None => true,
        Some(last_time) => {
            now.duration_since(last_time).as_millis() >= config.reinterpret_same_window_cooldown_ms
        }
    }
}
