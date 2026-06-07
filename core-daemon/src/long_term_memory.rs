use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongTermMemoryEntry {
    pub id: String,
    pub category: LongTermMemoryCategory,
    pub content: String,
    pub confidence: f32,
    pub importance: f32,
    pub created_at_ms: u128,
    pub updated_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LongTermMemoryCategory {
    UserPreference,
    ProjectFact,
    Habit,
    PersonalContext,
    CompanionBehavior,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LongTermMemoryStore {
    pub entries: Vec<LongTermMemoryEntry>,
}

impl LongTermMemoryStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read long-term memory: {}", path.display()))?;

        let store = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse long-term memory: {}", path.display()))?;

        Ok(store)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;

        Ok(())
    }

    pub fn add_or_update(&mut self, entry: LongTermMemoryEntry) {
        let normalized = normalize(&entry.content);

        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|item| normalize(&item.content) == normalized)
        {
            existing.confidence = existing.confidence.max(entry.confidence);
            existing.importance = existing.importance.max(entry.importance);
            existing.updated_at_ms = entry.updated_at_ms;
            return;
        }

        self.entries.push(entry);

        self.entries
            .sort_by(|a, b| b.importance.total_cmp(&a.importance));

        if self.entries.len() > 100 {
            self.entries.truncate(100);
        }
    }

    pub fn top_entries(&self, limit: usize) -> Vec<LongTermMemoryEntry> {
        self.entries.iter().take(limit).cloned().collect()
    }
}

fn normalize(input: &str) -> String {
    input
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
