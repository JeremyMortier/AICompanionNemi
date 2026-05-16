use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    pub user_request: String,
    pub summary: String,
    pub steps: Vec<String>,
    pub requires_confirmation: bool,
}
