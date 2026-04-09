use std::time::{Duration, Instant};

use anyhow::Result;
use tracing::{Level, error, info};
use tracing_subscriber::EnvFilter;

use crate::activity::classify_activity;
use crate::config::AppConfig;
use crate::context::ContextInterpretation;
use crate::events::{AppEvent, EventBus};
use crate::llm::LlmClient;
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
                        event_bus.push(AppEvent::ContextInterpreted(result));
                    }
                    Err(err) => {
                        error!(error = %err, "failed to interpret context with llm");
                    }
                }
            }
            AppEvent::ContextInterpreted(result) => {
                handle_interpreted_context(state, result);
            }
        }
    }
}

fn handle_interpreted_context(state: &mut AppState, result: ContextInterpretation) {
    info!(
        activity = ?result.activity,
        confidence = result.confidence,
        should_comment = result.should_comment,
        summary = %result.summary,
        "context interpreted"
    );

    state.last_interpretation = Some(result);
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
        crate::activity::UserActivity::Gaming => false,
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
