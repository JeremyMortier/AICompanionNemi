use std::collections::VecDeque;

use crate::activity::UserActivity;
use crate::context::ContextInterpretation;
use crate::decision::ReactionDecision;
use crate::mood::MoodState;
use crate::reaction::GeneratedReaction;

#[derive(Debug, Clone)]
pub struct ScreenCaptureEvent {
    pub path: String,
    pub screen_index: usize,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    ActiveWindowDetected {
        title: String,
        process_id: u32,
        process_name: String,
    },
    ScreensCaptured {
        captures: Vec<ScreenCaptureEvent>,
    },
    MoodUpdated(MoodState),
    ContextInterpretationRequested {
        title: String,
        process_name: String,
        heuristic_activity: UserActivity,
        stable_for_ms: u128,
    },
    ContextInterpreted {
        interpretation: ContextInterpretation,
        stable_for_ms: u128,
    },
    ReactionDecisionMade(ReactionDecision),
    ReactionGenerationRequested {
        decision: ReactionDecision,
        interpretation: ContextInterpretation,
        recent_reactions: Vec<String>,
        mood: MoodState,
    },
    ReactionGenerated(GeneratedReaction),
}

#[derive(Debug)]
pub struct EventBus {
    queue: VecDeque<AppEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn push(&mut self, event: AppEvent) {
        self.queue.push_back(event);
    }

    pub fn pop(&mut self) -> Option<AppEvent> {
        self.queue.pop_front()
    }
}
