use std::time::{Duration, Instant};

use crate::activity::UserActivity;
use crate::config::AppConfig;
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
    config: &AppConfig,
    interpretation: &ContextInterpretation,
    stable_for_ms: u128,
    now: Instant,
) -> ReactionDecision {
    if interpretation.confidence < minimum_confidence_for_persona(config) {
        return ReactionDecision::StaySilent {
            reason: "confidence too low for current persona thresholds".to_string(),
        };
    }

    if stable_for_ms < minimum_stable_duration_for_persona(config) {
        return ReactionDecision::StaySilent {
            reason: "context not stable long enough".to_string(),
        };
    }

    if let Some(last_reaction_at) = state.last_reaction_at {
        let cooldown = reaction_cooldown_for(config, &interpretation.activity);
        let since_last_reaction = now.duration_since(last_reaction_at);

        if since_last_reaction < cooldown {
            return ReactionDecision::StaySilent {
                reason: format!(
                    "cooldown active ({} ms remaining approx)",
                    cooldown.saturating_sub(since_last_reaction).as_millis()
                ),
            };
        }
    }

    match interpretation.activity {
        UserActivity::Gaming => {
            if config.debug_force_reaction_in_gaming {
                return persona_escalated_decision(
                    config,
                    "gaming debug mode: forcing reactions for testing",
                );
            }

            if config.persona.boldness >= 75 && config.persona.discretion <= 45 {
                ReactionDecision::LightComment {
                    reason: "bold persona allows occasional gaming reactions".to_string(),
                }
            } else {
                ReactionDecision::StaySilent {
                    reason: "gaming mode prefers silence".to_string(),
                }
            }
        }
        UserActivity::Chatting => {
            if config.persona.discretion >= 60 {
                ReactionDecision::StaySilent {
                    reason: "chat activity prefers silence for discreet persona".to_string(),
                }
            } else {
                ReactionDecision::LightComment {
                    reason: "less discreet persona allows light chat reactions".to_string(),
                }
            }
        }
        UserActivity::Coding => {
            let summary = interpretation.summary.to_lowercase();

            if summary.contains("tutorial")
                || summary.contains("educational")
                || summary.contains("learning")
            {
                return persona_escalated_decision(
                    config,
                    "coding context seems relaxed or educational",
                );
            }

            if config.persona.boldness >= 82
                && config.persona.curiosity >= 75
                && config.persona.discretion <= 45
            {
                ReactionDecision::LightComment {
                    reason: "bold and curious persona allows coding interruption".to_string(),
                }
            } else {
                ReactionDecision::StaySilent {
                    reason: "coding context likely requires focus".to_string(),
                }
            }
        }
        UserActivity::Watching => {
            persona_escalated_decision(config, "passive watching allows persona-shaped reactions")
        }
        UserActivity::Browsing => {
            if interpretation.confidence >= 0.8 {
                persona_escalated_decision(
                    config,
                    "browsing context seems stable and interpretable",
                )
            } else if config.persona.playfulness >= 60 || config.persona.curiosity >= 65 {
                ReactionDecision::LightComment {
                    reason: "persona allows a light browsing reaction".to_string(),
                }
            } else {
                ReactionDecision::StaySilent {
                    reason: "persona remains too reserved for weak browsing signal".to_string(),
                }
            }
        }
        UserActivity::Unknown => {
            if config.persona.boldness >= 85 && config.persona.curiosity >= 85 {
                ReactionDecision::LightComment {
                    reason: "very bold persona tolerates ambiguous context".to_string(),
                }
            } else {
                ReactionDecision::StaySilent {
                    reason: "unknown context prefers silence".to_string(),
                }
            }
        }
    }
}

fn persona_escalated_decision(config: &AppConfig, reason: &str) -> ReactionDecision {
    if config.persona.curiosity >= 70 || config.persona.playfulness >= 75 {
        ReactionDecision::CuriousComment {
            reason: reason.to_string(),
        }
    } else {
        ReactionDecision::LightComment {
            reason: reason.to_string(),
        }
    }
}

fn minimum_confidence_for_persona(config: &AppConfig) -> f32 {
    if config.persona.discretion >= 75 {
        0.72
    } else if config.persona.boldness >= 75 {
        0.58
    } else {
        0.65
    }
}

//il faut traiter le dernier chat
fn minimum_stable_duration_for_persona(config: &AppConfig) -> u128 {
    if config.persona.discretion >= 75 {
        2_500
    } else if config.persona.boldness >= 75 {
        1_000
    } else {
        1_500
    }
}

fn reaction_cooldown_for(config: &AppConfig, activity: &UserActivity) -> Duration {
    let base = match activity {
        UserActivity::Coding => Duration::from_secs(config.cooldown_coding_secs),
        UserActivity::Browsing => Duration::from_secs(config.cooldown_browsing_secs),
        UserActivity::Watching => Duration::from_secs(config.cooldown_watching_secs),
        UserActivity::Chatting => Duration::from_secs(config.cooldown_chatting_secs),
        UserActivity::Gaming => Duration::from_secs(config.cooldown_gaming_secs),
        UserActivity::Unknown => Duration::from_secs(config.cooldown_unknown_secs),
    };

    if config.persona.boldness >= 80 && config.persona.discretion <= 40 {
        base.div_f32(1.6)
    } else if config.persona.discretion >= 80 {
        base.mul_f32(1.4)
    } else {
        base
    }
}
