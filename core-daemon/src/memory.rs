use std::collections::VecDeque;

use crate::reaction::GeneratedReaction;

const MAX_RECENT_REACTIONS: usize = 8;

#[derive(Debug, Clone)]
pub struct RecentReactionMemory {
    items: VecDeque<GeneratedReaction>,
}

impl RecentReactionMemory {
    pub fn new() -> Self {
        Self {
            items: VecDeque::with_capacity(MAX_RECENT_REACTIONS),
        }
    }

    pub fn push(&mut self, reaction: GeneratedReaction) {
        if self.items.len() >= MAX_RECENT_REACTIONS {
            self.items.pop_front();
        }

        self.items.push_back(reaction);
    }

    pub fn recent_texts(&self) -> Vec<String> {
        self.items.iter().map(|item| item.text.clone()).collect()
    }

    pub fn is_too_similar(&self, candidate: &str) -> bool {
        let normalized_candidate = normalize(candidate);

        self.items.iter().any(|existing| {
            let normalized_existing = normalize(&existing.text);

            normalized_existing == normalized_candidate
                || contains_with_min_length(&normalized_existing, &normalized_candidate, 18)
                || contains_with_min_length(&normalized_candidate, &normalized_existing, 18)
        })
    }
}

fn normalize(input: &str) -> String {
    input
        .to_lowercase()
        .replace(['\n', '\r', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_with_min_length(a: &str, b: &str, min_len: usize) -> bool {
    a.len() >= min_len && b.len() >= min_len && a.contains(b)
}
