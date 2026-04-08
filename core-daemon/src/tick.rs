use crate::config::AppConfig;
use crate::events::{AppEvent, EventBus};
use crate::observation::get_active_window_info;
use crate::state::AppState;
use tracing::warn;

pub fn run_tick(state: &mut AppState, _config: &AppConfig, event_bus: &mut EventBus) {
    state.increment_tick();

    match get_active_window_info() {
        Ok(Some(window)) => {
            event_bus.push(AppEvent::ActiveWindowDetected {
                title: window.title,
                process_id: window.process_id,
                process_name: window.process_name,
            });
        }
        Ok(None) => {}
        Err(error) => {
            warn!(tick = state.tick_count, error = %error, "failed to get active window info");
        }
    }
}
