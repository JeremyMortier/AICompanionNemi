use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct ActiveWindowSnapshot {
    pub title: String,
    pub process_id: u32,
    pub process_name: String,
    pub activity: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct InterpretationSnapshot {
    pub activity: String,
    pub confidence: f32,
    pub summary: String,
    pub should_comment: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MoodSnapshot {
    pub current: String,
    pub intensity: u8,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct AppSnapshot {
    pub companion_name: String,
    pub tick_count: u64,
    pub active_window: Option<ActiveWindowSnapshot>,
    pub last_interpretation: Option<InterpretationSnapshot>,
    pub last_decision: Option<String>,
    pub last_generated_reaction: Option<String>,
    pub mood: MoodSnapshot,
    pub last_screen_captures: Vec<ScreenCaptureSnapshot>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ScreenCaptureSnapshot {
    pub path: String,
    pub screen_index: usize,
    pub width: u32,
    pub height: u32,
}
