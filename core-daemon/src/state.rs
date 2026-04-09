use std::time::Instant;

use crate::activity::UserActivity;
use crate::context::ContextInterpretation;

#[derive(Debug, Clone)]
pub struct ActiveWindowState {
    pub title: String,
    pub process_id: u32,
    pub process_name: String,
    pub activity: UserActivity,
    pub first_seen_at: Instant,
    pub last_seen_at: Instant,
    pub interpretation_requested: bool,
}

#[derive(Debug)]
pub struct AppState {
    pub tick_count: u64,
    pub active_window: Option<ActiveWindowState>,
    pub last_interpretation: Option<ContextInterpretation>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tick_count: 0,
            active_window: None,
            last_interpretation: None,
        }
    }

    pub fn increment_tick(&mut self) {
        self.tick_count += 1;
    }
}
