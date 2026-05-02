use serde::{Deserialize, Serialize};

use crate::activity::UserActivity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionInterpretation {
    pub detected_activity: UserActivity,
    pub confidence: f32,
    pub description: String,
}
