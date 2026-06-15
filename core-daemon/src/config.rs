use serde::{Deserialize, Serialize};

use crate::persona::PersonaProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeProfile {
    DevLight,
    Balanced,
    Heavy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub companion_name: String,
    pub tick_interval_ms: u64,
    pub verbose_logs: bool,

    pub runtime_profile: RuntimeProfile,

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

    pub chat_model: String,
    pub vision_model: String,
    pub reasoning_model: String,
    pub fast_model: String,

    pub auto_screen_capture_enabled: bool,
    pub screen_capture_every_ticks: u64,
    pub auto_vision_enabled: bool,
    pub auto_ocr_enabled: bool,
    pub auto_assessment_enabled: bool,
    pub auto_memory_learning_enabled: bool,
    pub auto_memory_summary_enabled: bool,
    pub auto_curiosity_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::balanced()
    }
}

impl AppConfig {
    pub fn dev_light() -> Self {
        Self {
            companion_name: "Nemi".to_string(),
            tick_interval_ms: 2_000,
            verbose_logs: true,

            runtime_profile: RuntimeProfile::DevLight,

            interpretation_threshold_ms: 4_000,

            cooldown_coding_secs: 20,
            cooldown_browsing_secs: 15,
            cooldown_watching_secs: 15,
            cooldown_chatting_secs: 20,
            cooldown_gaming_secs: 12,
            cooldown_unknown_secs: 15,

            reinterpret_same_window_cooldown_ms: 20_000,

            debug_force_reaction_in_gaming: false,

            persona: PersonaProfile::nemi_default(),

            tesseract_path: "tesseract".to_string(),
            ocr_enabled: true,

            chat_model: "gemma3:4b".to_string(),
            vision_model: "qwen2.5vl:7b".to_string(),
            reasoning_model: "gemma3:4b".to_string(),
            fast_model: "gemma3:4b".to_string(),

            auto_screen_capture_enabled: false,
            screen_capture_every_ticks: 10,
            auto_vision_enabled: true,
            auto_ocr_enabled: true,
            auto_assessment_enabled: true,
            auto_memory_learning_enabled: false,
            auto_memory_summary_enabled: false,
            auto_curiosity_enabled: false,
        }
    }

    pub fn balanced() -> Self {
        Self {
            runtime_profile: RuntimeProfile::Balanced,
            tick_interval_ms: 2_000,
            interpretation_threshold_ms: 4_000,
            reinterpret_same_window_cooldown_ms: 20_000,
            cooldown_coding_secs: 30,
            cooldown_browsing_secs: 20,
            cooldown_watching_secs: 20,
            cooldown_chatting_secs: 30,
            cooldown_gaming_secs: 20,
            cooldown_unknown_secs: 20,
            debug_force_reaction_in_gaming: false,
            auto_screen_capture_enabled: true,
            screen_capture_every_ticks: 15,
            auto_vision_enabled: true,
            auto_ocr_enabled: true,
            auto_assessment_enabled: true,
            auto_memory_learning_enabled: false,
            auto_memory_summary_enabled: false,
            auto_curiosity_enabled: false,
            ..Self::dev_light()
        }
    }

    pub fn heavy() -> Self {
        Self {
            runtime_profile: RuntimeProfile::Heavy,
            tick_interval_ms: 1_500,
            interpretation_threshold_ms: 3_000,
            reinterpret_same_window_cooldown_ms: 12_000,
            cooldown_coding_secs: 15,
            cooldown_browsing_secs: 10,
            cooldown_watching_secs: 10,
            cooldown_chatting_secs: 15,
            cooldown_gaming_secs: 10,
            cooldown_unknown_secs: 10,
            auto_screen_capture_enabled: true,
            screen_capture_every_ticks: 10,
            auto_vision_enabled: true,
            auto_ocr_enabled: true,
            auto_assessment_enabled: true,
            auto_memory_learning_enabled: true,
            auto_memory_summary_enabled: true,
            auto_curiosity_enabled: true,
            ..Self::dev_light()
        }
    }
}
