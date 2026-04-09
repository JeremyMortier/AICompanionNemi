use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UserActivity {
    Coding,
    Browsing,
    Watching,
    Chatting,
    Gaming,
    Unknown,
}

pub fn classify_activity(process_name: &str, title: &str) -> UserActivity {
    let process = process_name.to_lowercase();
    let title = title.to_lowercase();

    if process.contains("code")
        || process.contains("idea")
        || process.contains("pycharm")
        || process.contains("webstorm")
    {
        return UserActivity::Coding;
    }

    if process.contains("chrome")
        || process.contains("firefox")
        || process.contains("edge")
        || process.contains("opera")
    {
        if title.contains("youtube") || title.contains("netflix") || title.contains("twitch") {
            return UserActivity::Watching;
        }

        return UserActivity::Browsing;
    }

    if process.contains("discord") || process.contains("slack") || process.contains("teams") {
        return UserActivity::Chatting;
    }

    if process.contains("game") || process.contains("steam") {
        return UserActivity::Gaming;
    }

    UserActivity::Unknown
}
