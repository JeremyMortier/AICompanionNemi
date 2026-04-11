use std::time::Instant;

use crate::activity::UserActivity;
use crate::context::ContextInterpretation;
use crate::decision::ReactionDecision;
use crate::reaction::GeneratedReaction;

#[derive(Debug, Clone)]
pub struct ActiveWindowState {
    pub title: String,
    pub process_id: u32,
    pub process_name: String,
    pub activity: UserActivity,
    pub first_seen_at: Instant,
    pub last_seen_at: Instant,
    pub interpretation_requested: bool,
}

#[derive(Debug)]
pub struct AppState {
    pub tick_count: u64,
    pub active_window: Option<ActiveWindowState>,
    pub last_interpretation: Option<ContextInterpretation>,
    pub last_decision: Option<ReactionDecision>,
    pub last_reaction_at: Option<Instant>,
    pub last_generated_reaction: Option<GeneratedReaction>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tick_count: 0,
            active_window: None,
            last_interpretation: None,
            last_decision: None,
            last_reaction_at: None,
            last_generated_reaction: None,
        }
    }

    pub fn increment_tick(&mut self) {
        self.tick_count += 1;
    }
}
