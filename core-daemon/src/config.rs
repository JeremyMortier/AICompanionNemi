use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub companion_name: String,
    pub tick_interval_ms: u64,
    pub verbose_logs: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            companion_name: "Nemi".to_string(),
            tick_interval_ms: 2_000,
            verbose_logs: true,
        }
    }
}
