use std::time::{Duration, Instant};

use crate::activity::UserActivity;
use crate::context::ContextInterpretation;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub enum ReactionDecision {
    StaySilent { reason: String },
    LightComment { reason: String },
    CuriousComment { reason: String },
}

pub fn decide_reaction(
    state: &AppState,
    interpretation: &ContextInterpretation,
    stable_for_ms: u128,
    now: Instant,
) -> ReactionDecision {
    if interpretation.confidence < 0.65 {
        return ReactionDecision::StaySilent {
            reason: "confidence too low".to_string(),
        };
    }

    if stable_for_ms < 100 {
        return ReactionDecision::StaySilent {
            reason: "context not stable long enough".to_string(),
        };
    }

    if let Some(last_reaction_at) = state.last_reaction_at {
        let since_last_reaction = now.duration_since(last_reaction_at);
        if since_last_reaction < reaction_cooldown_for(&interpretation.activity) {
            return ReactionDecision::StaySilent {
                reason: format!(
                    "cooldown active ({} ms remaining approx)",
                    reaction_cooldown_for(&interpretation.activity)
                        .saturating_sub(since_last_reaction)
                        .as_millis()
                ),
            };
        }
    }

    match interpretation.activity {
        UserActivity::Gaming => ReactionDecision::CuriousComment {
            reason: "gaming debug mode: forcing reactions for testing".to_string(),
        },
        UserActivity::Chatting => ReactionDecision::StaySilent {
            reason: "chat activity prefers silence".to_string(),
        },
        UserActivity::Coding => {
            if interpretation.summary.to_lowercase().contains("tutorial")
                || interpretation
                    .summary
                    .to_lowercase()
                    .contains("educational")
                || interpretation.summary.to_lowercase().contains("learning")
            {
                ReactionDecision::LightComment {
                    reason: "coding context seems relaxed or educational".to_string(),
                }
            } else {
                ReactionDecision::StaySilent {
                    reason: "coding context likely requires focus".to_string(),
                }
            }
        }
        UserActivity::Watching => ReactionDecision::LightComment {
            reason: "passive watching allows light reactions".to_string(),
        },
        UserActivity::Browsing => {
            if interpretation.confidence >= 0.8 {
                ReactionDecision::CuriousComment {
                    reason: "browsing context seems stable and interpretable".to_string(),
                }
            } else {
                ReactionDecision::LightComment {
                    reason: "browsing context allows a light reaction".to_string(),
                }
            }
        }
        UserActivity::Unknown => ReactionDecision::StaySilent {
            reason: "unknown context prefers silence".to_string(),
        },
    }
}

fn reaction_cooldown_for(activity: &UserActivity) -> Duration {
    match activity {
        UserActivity::Coding => Duration::from_secs(10),
        UserActivity::Browsing => Duration::from_secs(10),
        UserActivity::Watching => Duration::from_secs(15),
        UserActivity::Chatting => Duration::from_secs(10),
        UserActivity::Gaming => Duration::from_secs(5),
        UserActivity::Unknown => Duration::from_secs(10),
    }
}
