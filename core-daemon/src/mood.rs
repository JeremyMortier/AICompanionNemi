use serde::{Deserialize, Serialize};

use crate::activity::UserActivity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Mood {
    Calm,
    Curious,
    Playful,
    Focused,
    Proud,
    Sulky,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoodState {
    pub current: Mood,
    pub intensity: u8,
}

impl MoodState {
    pub fn new() -> Self {
        Self {
            current: Mood::Calm,
            intensity: 40,
        }
    }

    pub fn update_from_activity(&mut self, activity: &UserActivity, stable_for_ms: u128) {
        match activity {
            UserActivity::Coding => {
                if stable_for_ms >= 10_000 {
                    self.current = Mood::Focused;
                    self.intensity = 70;
                } else {
                    self.current = Mood::Curious;
                    self.intensity = 50;
                }
            }
            UserActivity::Browsing => {
                self.current = Mood::Curious;
                self.intensity = 65;
            }
            UserActivity::Watching => {
                self.current = Mood::Calm;
                self.intensity = 55;
            }
            UserActivity::Chatting => {
                self.current = Mood::Playful;
                self.intensity = 50;
            }
            UserActivity::Gaming => {
                self.current = Mood::Playful;
                self.intensity = 75;
            }
            UserActivity::Unknown => {
                self.current = Mood::Curious;
                self.intensity = 45;
            }
        }
    }
}
