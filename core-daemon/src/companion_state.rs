use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanionState {
    pub familiarity_score: f32,
    pub engagement_score: f32,
    pub curiosity_drive: f32,
    pub last_internal_note: Option<String>,
}

impl Default for CompanionState {
    fn default() -> Self {
        Self {
            familiarity_score: 0.15,
            engagement_score: 0.25,
            curiosity_drive: 0.35,
            last_internal_note: None,
        }
    }
}

impl CompanionState {
    pub fn observe_user_message(&mut self, message: &str) {
        self.familiarity_score = (self.familiarity_score + 0.015).min(1.0);
        self.engagement_score = (self.engagement_score + 0.04).min(1.0);

        let lower = message.to_lowercase();
        if lower.contains("pourquoi")
            || lower.contains("comment")
            || lower.contains("tu vois")
            || lower.contains("what")
            || lower.contains("why")
            || lower.contains("how")
        {
            self.curiosity_drive = (self.curiosity_drive + 0.03).min(1.0);
        }

        self.last_internal_note = Some("The user interacted directly with Nemi.".to_string());
    }

    pub fn observe_assessment(&mut self, situation: &str, confidence: f32) {
        if confidence >= 0.75 {
            self.engagement_score = (self.engagement_score + 0.015).min(1.0);
        }

        if situation.len() > 20 {
            self.curiosity_drive = (self.curiosity_drive + 0.01).min(1.0);
        }

        self.last_internal_note = Some(format!("Recently noticed: {situation}"));
    }

    pub fn observe_reaction(&mut self) {
        self.engagement_score = (self.engagement_score + 0.02).min(1.0);
        self.curiosity_drive = (self.curiosity_drive * 0.98).max(0.0);
        self.last_internal_note = Some("Nemi reacted to the current context.".to_string());
    }

    pub fn observe_curiosity_question(&mut self, subject: &str) {
        self.curiosity_drive = (self.curiosity_drive * 0.75).max(0.0);
        self.engagement_score = (self.engagement_score + 0.03).min(1.0);
        self.last_internal_note = Some(format!("Nemi became curious about: {subject}"));
    }
}
