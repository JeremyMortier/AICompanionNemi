use std::time::Instant;

use crate::action_plan::ActionPlan;
use crate::activity::UserActivity;
use crate::assessment::ContextAssessment;
use crate::attention::AttentionState;
use crate::chat::ChatMessage;
use crate::context::ContextInterpretation;
use crate::context_fusion::FusedContext;
use crate::decision::ReactionDecision;
use crate::long_term_memory::LongTermMemoryStore;
use crate::memory::MemoryEntry;
use crate::memory::RecentReactionMemory;
use crate::mood::MoodState;
use crate::reaction::GeneratedReaction;
use crate::visible_text::VisibleTextContext;

#[derive(Debug, Clone)]
pub struct ActiveWindowState {
    pub title: String,
    pub process_id: u32,
    pub process_name: String,
    pub activity: UserActivity,
    pub first_seen_at: Instant,
    pub last_seen_at: Instant,
    pub last_interpretation_requested_at: Option<Instant>,
    pub window_left: i32,
    pub window_top: i32,
    pub window_right: i32,
    pub window_bottom: i32,
}

#[derive(Debug)]
pub struct AppState {
    pub tick_count: u64,
    pub active_window: Option<ActiveWindowState>,
    pub last_interpretation: Option<ContextInterpretation>,
    pub last_decision: Option<ReactionDecision>,
    pub last_reaction_at: Option<Instant>,
    pub last_generated_reaction: Option<GeneratedReaction>,
    pub recent_reaction_memory: RecentReactionMemory,
    pub mood: MoodState,
    pub last_screen_captures: Vec<crate::events::ScreenCaptureEvent>,
    pub last_fused_context: Option<FusedContext>,
    pub chat_history: Vec<ChatMessage>,
    pub last_chat_reply: Option<String>,
    pub last_ocr_text: Option<String>,
    pub visible_text_context: Option<VisibleTextContext>,
    pub last_action_plan: Option<ActionPlan>,
    pub pending_action: Option<crate::actions::ExecutableAction>,
    pub last_assessment: Option<ContextAssessment>,
    pub short_term_memory: Vec<MemoryEntry>,
    pub short_term_memory_summary: Option<String>,
    pub attention: AttentionState,
    pub long_term_memory: LongTermMemoryStore,
    pub last_curiosity_question: Option<String>,
}

impl AppState {
    pub fn new(long_term_memory: LongTermMemoryStore) -> Self {
        Self {
            tick_count: 0,
            active_window: None,
            last_interpretation: None,
            last_decision: None,
            last_reaction_at: None,
            last_generated_reaction: None,
            recent_reaction_memory: RecentReactionMemory::new(),
            mood: MoodState::new(),
            last_screen_captures: Vec::new(),
            last_fused_context: None,
            chat_history: Vec::new(),
            last_chat_reply: None,
            last_ocr_text: None,
            visible_text_context: None,
            last_action_plan: None,
            pending_action: None,
            last_assessment: None,
            short_term_memory: Vec::new(),
            short_term_memory_summary: None,
            attention: AttentionState::default(),
            long_term_memory,
            last_curiosity_question: None,
        }
    }

    pub fn increment_tick(&mut self) {
        self.tick_count += 1;
    }
}

impl AppState {
    pub fn push_memory(&mut self, entry: MemoryEntry) {
        let normalized_new = normalize_memory_text(&entry.summary);

        let is_duplicate = self.short_term_memory.iter().rev().take(8).any(|existing| {
            if existing.category != entry.category {
                return false;
            }

            let normalized_existing = normalize_memory_text(&existing.summary);
            memory_similarity(&normalized_existing, &normalized_new) >= 0.55
        });

        if is_duplicate {
            return;
        }

        self.short_term_memory.push(entry);

        if self.short_term_memory.len() > 50 {
            self.short_term_memory.remove(0);
        }
    }
}

fn normalize_memory_text(input: &str) -> String {
    input
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn memory_similarity(a: &str, b: &str) -> f32 {
    let words_a = meaningful_words(a);
    let words_b = meaningful_words(b);

    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }

    let intersection = words_a.iter().filter(|word| words_b.contains(word)).count();

    let smaller = words_a.len().min(words_b.len());

    intersection as f32 / smaller as f32
}

fn meaningful_words(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_alphanumeric())
        .map(str::trim)
        .filter(|word| word.len() >= 4)
        .map(str::to_lowercase)
        .filter(|word| !is_memory_stop_word(word))
        .collect()
}

fn is_memory_stop_word(word: &str) -> bool {
    matches!(
        word,
        "situation"
            | "goal"
            | "user"
            | "screen"
            | "shows"
            | "displays"
            | "current"
            | "visible"
            | "information"
            | "including"
            | "potentially"
            | "active"
            | "actively"
            | "using"
            | "viewing"
    )
}
