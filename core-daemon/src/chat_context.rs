use crate::{
    assessment::ContextAssessment, attention::AttentionState, companion_state::CompanionState, context_fusion::FusedContext,
    intent::UserIntent, long_term_memory::LongTermMemoryStore, memory::MemoryEntry,
    mood::MoodState, persona::PersonaProfile, visible_text::VisibleTextContext,
};

pub struct ChatGenerationContext<'a> {
    pub user_intent: &'a UserIntent,
    pub fused_context: Option<&'a FusedContext>,
    pub visible_text: Option<&'a VisibleTextContext>,
    pub assessment: Option<&'a ContextAssessment>,
    pub persona: &'a PersonaProfile,
    pub mood: &'a MoodState,
    pub short_term_memory: &'a [MemoryEntry],
    pub short_term_memory_summary: Option<&'a String>,
    pub attention: &'a AttentionState,
    pub long_term_memory: &'a LongTermMemoryStore,
    pub companion_state: &'a CompanionState,
}
