use std::collections::VecDeque;

use crate::activity::UserActivity;
use crate::context::ContextInterpretation;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    ActiveWindowDetected {
        title: String,
        process_id: u32,
        process_name: String,
    },
    ContextInterpretationRequested {
        title: String,
        process_name: String,
        heuristic_activity: UserActivity,
        stable_for_ms: u128,
    },
    ContextInterpreted(ContextInterpretation),
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
