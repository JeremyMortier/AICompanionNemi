use serde::{Deserialize, Serialize};

use crate::long_term_memory::LongTermMemoryCategory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub should_store: bool,
    pub category: LongTermMemoryCategory,
    pub content: String,
    pub confidence: f32,
    pub importance: f32,
}
