use serde::{Deserialize, Serialize};

use crate::action_plan::{ProposedAction, ProposedActionKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutableAction {
    OpenApplication { application_name: String },

    OpenUrl { url: String },

    SearchWeb { query: String },
}

pub fn proposed_action_to_executable(proposed: &ProposedAction) -> Option<ExecutableAction> {
    match proposed.kind {
        ProposedActionKind::OpenApplication => Some(ExecutableAction::OpenApplication {
            application_name: proposed.target.clone(),
        }),

        ProposedActionKind::OpenUrl => {
            if proposed.target.starts_with("http://") || proposed.target.starts_with("https://") {
                Some(ExecutableAction::OpenUrl {
                    url: proposed.target.clone(),
                })
            } else {
                None
            }
        }

        ProposedActionKind::SearchWeb => Some(ExecutableAction::SearchWeb {
            query: proposed.target.clone(),
        }),

        _ => None,
    }
}
