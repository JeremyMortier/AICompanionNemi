use std::time::Duration;

use anyhow::Result;
use tracing::{Level, info};
use tracing_subscriber::EnvFilter;

use crate::config::AppConfig;
use crate::events::{AppEvent, EventBus};
use crate::state::{ActiveWindowState, AppState};
use crate::tick::run_tick;

pub async fn run() -> Result<()> {
    init_tracing();

    let config = AppConfig::default();
    let mut state = AppState::new();
    let mut event_bus = EventBus::new();

    info!("Starting core-daemon...");
    info!("Companion name: {}", config.companion_name);
    info!("Tick interval: {} ms", config.tick_interval_ms);

    let mut interval = tokio::time::interval(Duration::from_millis(config.tick_interval_ms));

    loop {
        interval.tick().await;

        event_bus.push(AppEvent::Tick);

        process_events(&mut event_bus, &mut state, &config);
    }
}

fn process_events(event_bus: &mut EventBus, state: &mut AppState, config: &AppConfig) {
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
                let now = std::time::Instant::now();

                match &mut state.active_window {
                    Some(current) => {
                        let is_same =
                            current.title == title &&
                            current.process_id == process_id &&
                            current.process_name == process_name;

                        if is_same {
                            current.last_seen_at = now;

                            let duration = now.duration_since(current.first_seen_at);

                            info!(
                                tick = state.tick_count,
                                process_name = %process_name,
                                title = %title,
                                stable_for_ms = duration.as_millis(),
                                "window still active"
                            );
                        } else {
                            state.active_window = Some(ActiveWindowState {
                                title: title.clone(),
                                process_id,
                                process_name: process_name.clone(),
                                first_seen_at: now,
                                last_seen_at: now,
                            });

                            info!(
                                tick = state.tick_count,
                                process_name = %process_name,
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
                            first_seen_at: now,
                            last_seen_at: now,
                        });

                        info!(
                            tick = state.tick_count,
                            process_name = %process_name,
                            title = %title,
                            "initial active window detected"
                        );
                    }
                }
            }
        }
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
