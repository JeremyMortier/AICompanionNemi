use std::time::{Duration, Instant};

use anyhow::Result;
use tracing::{Level, error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::activity::classify_activity;
use crate::config::AppConfig;
use crate::context::ContextInterpretation;
use crate::decision::{ReactionDecision, decide_reaction};
use crate::events::{AppEvent, EventBus};
use crate::llm::LlmClient;
use crate::reaction::GeneratedReaction;
use crate::state::{ActiveWindowState, AppState};
use crate::tick::run_tick;

const INTERPRETATION_THRESHOLD_MS: u128 = 5_000;

pub async fn run() -> Result<()> {
    init_tracing();

    let config = AppConfig::default();
    let mut state = AppState::new();
    let mut event_bus = EventBus::new();
    let llm = LlmClient::new(
        "http://127.0.0.1:11434".to_string(),
        "gemma3:4b".to_string(),
    );

    info!("Starting core-daemon...");
    info!("Companion name: {}", config.companion_name);
    info!("Tick interval: {} ms", config.tick_interval_ms);

    let mut interval = tokio::time::interval(Duration::from_millis(config.tick_interval_ms));

    loop {
        interval.tick().await;
        event_bus.push(AppEvent::Tick);
        process_events(&mut event_bus, &mut state, &config, &llm).await;
    }
}

async fn process_events(
    event_bus: &mut EventBus,
    state: &mut AppState,
    config: &AppConfig,
    llm: &LlmClient,
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

                            if !current.interpretation_requested
                                && should_request_interpretation(&current.activity, stable_for_ms)
                            {
                                current.interpretation_requested = true;

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
                                interpretation_requested: false,
                            });

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
                            interpretation_requested: false,
                        });

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
            } => {
                match llm
                    .generate_reaction(
                        &interpretation,
                        &decision,
                        &recent_reactions,
                        &config.persona,
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
        }
    }
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
    activity: &crate::activity::UserActivity,
    stable_for_ms: u128,
) -> bool {
    if stable_for_ms < INTERPRETATION_THRESHOLD_MS {
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
