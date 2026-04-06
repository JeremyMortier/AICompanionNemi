use crate::config::AppConfig;
use crate::state::AppState;
use tracing::info;

pub fn run_tick(state: &mut AppState, config: &AppConfig) {
    state.increment_tick();

    let summary = format!(
        "{} observe calmement le PC... tick #{}",
        config.companion_name, state.tick_count
    );

    state.last_observation_summary = Some(summary.clone());

    info!(tick = state.tick_count, summary = %summary);
}