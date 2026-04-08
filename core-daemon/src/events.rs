use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    ActiveWindowDetected {
        title: String,
        process_id: u32,
        process_name: String,
    },
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
