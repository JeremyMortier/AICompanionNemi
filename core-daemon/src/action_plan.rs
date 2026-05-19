use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    pub user_request: String,
    pub summary: String,
    pub steps: Vec<String>,
    pub proposed_action: ProposedAction,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposedActionKind {
    OpenApplication,
    OpenUrl,
    SearchWeb,
    ExplainScreen,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedAction {
    pub kind: ProposedActionKind,
    pub target: String,
}
