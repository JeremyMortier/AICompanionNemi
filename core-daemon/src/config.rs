use serde::{Deserialize, Serialize};

use crate::persona::PersonaProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub companion_name: String,
    pub tick_interval_ms: u64,
    pub verbose_logs: bool,

    pub interpretation_threshold_ms: u128,

    pub cooldown_coding_secs: u64,
    pub cooldown_browsing_secs: u64,
    pub cooldown_watching_secs: u64,
    pub cooldown_chatting_secs: u64,
    pub cooldown_gaming_secs: u64,
    pub cooldown_unknown_secs: u64,

    pub reinterpret_same_window_cooldown_ms: u128,

    pub debug_force_reaction_in_gaming: bool,

    pub persona: PersonaProfile,

    pub tesseract_path: String,
    pub ocr_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            companion_name: "Nemi".to_string(),
            tick_interval_ms: 1_000,
            verbose_logs: true,

            interpretation_threshold_ms: 1_000,

            cooldown_coding_secs: 6,
            cooldown_browsing_secs: 4,
            cooldown_watching_secs: 4,
            cooldown_chatting_secs: 6,
            cooldown_gaming_secs: 3,
            cooldown_unknown_secs: 4,

            reinterpret_same_window_cooldown_ms: 5_000,

            debug_force_reaction_in_gaming: true,

            persona: PersonaProfile::nemi_default(),

            tesseract_path: "tesseract".to_string(),
            ocr_enabled: true,
        }
    }
}
