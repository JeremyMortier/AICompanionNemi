use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionTarget {
    pub subject: String,
    pub interest_score: f32,
    pub seen_count: u32,
    pub last_seen_timestamp_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AttentionState {
    pub targets: Vec<AttentionTarget>,
}

impl AttentionState {
    pub fn observe_subject(&mut self, subject: &str, timestamp_ms: u128) {
        let normalized = normalize_subject(subject);

        if normalized.is_empty() {
            return;
        }

        if let Some(target) = self
            .targets
            .iter_mut()
            .find(|target| normalize_subject(&target.subject) == normalized)
        {
            target.seen_count += 1;
            target.last_seen_timestamp_ms = timestamp_ms;
            target.interest_score = (target.interest_score + 0.08).min(1.0);
            return;
        }

        self.targets.push(AttentionTarget {
            subject: subject.to_string(),
            interest_score: 0.35,
            seen_count: 1,
            last_seen_timestamp_ms: timestamp_ms,
        });

        self.targets
            .sort_by(|a, b| b.interest_score.total_cmp(&a.interest_score));

        if self.targets.len() > 20 {
            self.targets.truncate(20);
        }
    }

    pub fn top_targets(&self, limit: usize) -> Vec<AttentionTarget> {
        let mut targets = self.targets.clone();

        targets.sort_by(|a, b| {
            b.interest_score
                .total_cmp(&a.interest_score)
                .then(b.seen_count.cmp(&a.seen_count))
        });

        targets.into_iter().take(limit).collect()
    }

    pub fn most_interesting_subject(&self) -> Option<&AttentionTarget> {
        self.targets
            .iter()
            .filter(|target| target.interest_score >= 0.55 && target.seen_count >= 2)
            .max_by(|a, b| a.interest_score.total_cmp(&b.interest_score))
    }
}

fn normalize_subject(subject: &str) -> String {
    subject
        .to_lowercase()
        .replace(['\n', '\r', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl AttentionState {
    pub fn strong_targets(&self) -> Vec<AttentionTarget> {
        self.targets
            .iter()
            .filter(|target| target.seen_count >= 3 && target.interest_score >= 0.55)
            .cloned()
            .collect()
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read attention state: {}", path.display()))?;

        let state = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse attention state: {}", path.display()))?;

        Ok(state)
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
}
