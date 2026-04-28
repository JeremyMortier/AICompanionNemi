use crate::config::AppConfig;
use crate::events::ScreenCaptureEvent;
use crate::events::{AppEvent, EventBus};
use crate::observation::get_active_window_info;
use crate::screen::capture_all_screens;
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

    if state.tick_count.is_multiple_of(5) {
        match capture_all_screens("tmp/screenshots") {
            Ok(captures) => {
                event_bus.push(AppEvent::ScreensCaptured {
                    captures: captures
                        .into_iter()
                        .map(|capture| ScreenCaptureEvent {
                            path: capture.path.display().to_string(),
                            screen_index: capture.screen_index,
                            width: capture.width,
                            height: capture.height,
                        })
                        .collect(),
                });
            }
            Err(error) => {
                warn!(tick = state.tick_count, error = %error, "failed to capture screens");
            }
        }
    }
}
