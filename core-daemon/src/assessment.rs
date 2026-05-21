use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAssessment {
    pub situation: String,
    pub likely_user_goal: String,
    pub visible_clues: Vec<String>,
    pub uncertainties: Vec<String>,
    pub recommended_next_step: Option<String>,
    pub confidence: f32,
}
