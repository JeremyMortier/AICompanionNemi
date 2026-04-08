#[derive(Debug, Clone)]
pub struct ActiveWindowState {
    pub title: String,
    pub process_id: u32,
    pub process_name: String,
}

#[derive(Debug)]
pub struct AppState {
    pub tick_count: u64,
    pub active_window: Option<ActiveWindowState>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tick_count: 0,
            active_window: None,
        }
    }

    pub fn increment_tick(&mut self) {
        self.tick_count += 1;
    }
}
