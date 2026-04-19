use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaProfile {
    pub name: String,

    pub energy: u8,
    pub playfulness: u8,
    pub curiosity: u8,
    pub affection: u8,
    pub boldness: u8,
    pub discretion: u8,

    pub speaking_style: SpeakingStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpeakingStyle {
    Soft,
    Teasing,
    Cheerful,
    Calm,
}

impl PersonaProfile {
    pub fn nemi_default() -> Self {
        Self {
            name: "Nemi".to_string(),
            energy: 70,
            playfulness: 72,
            curiosity: 78,
            affection: 58,
            boldness: 52,
            discretion: 68,
            speaking_style: SpeakingStyle::Teasing,
        }
    }
}
