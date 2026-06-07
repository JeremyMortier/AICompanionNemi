use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuriosityTopic {
    pub subject: String,
    pub curiosity_score: f32,
}
