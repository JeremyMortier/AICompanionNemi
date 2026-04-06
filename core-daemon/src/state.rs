#[derive(Debug)]
pub struct AppState {
    pub tick_count: u64,
    pub last_observation_summary: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tick_count: 0,
            last_observation_summary: None,
        }
    }

    pub fn increment_tick(&mut self) {
        self.tick_count += 1;
    }
}