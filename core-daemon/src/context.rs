use serde::{Deserialize, Serialize};

use crate::activity::UserActivity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextInterpretation {
    pub activity: UserActivity,
    pub confidence: f32,
    pub summary: String,
    pub should_comment: bool,
}
