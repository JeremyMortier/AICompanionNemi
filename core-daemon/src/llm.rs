use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::activity::UserActivity;
use crate::context::ContextInterpretation;

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    format: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

pub struct LlmClient {
    http: Client,
    base_url: String,
    model: String,
}

impl LlmClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            http: Client::new(),
            base_url,
            model,
        }
    }

    pub async fn interpret_context(
        &self,
        process_name: &str,
        title: &str,
        heuristic_activity: &UserActivity,
        stable_for_ms: u128,
    ) -> Result<ContextInterpretation> {
        let prompt = build_prompt(process_name, title, heuristic_activity, stable_for_ms);

        let request = OllamaGenerateRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
            format: serde_json::json!({
                "type": "object",
                "properties": {
                    "activity": {
                        "type": "string",
                        "enum": ["Coding", "Browsing", "Watching", "Chatting", "Gaming", "Unknown"]
                    },
                    "confidence": {
                        "type": "number"
                    },
                    "summary": {
                        "type": "string"
                    },
                    "should_comment": {
                        "type": "boolean"
                    }
                },
                "required": ["activity", "confidence", "summary", "should_comment"]
            }),
        };

        let url = format!("{}/api/generate", self.base_url);

        let response = self
            .http
            .post(url)
            .json(&request)
            .send()
            .await
            .context("failed to call Ollama")?
            .error_for_status()
            .context("Ollama returned an HTTP error")?
            .json::<OllamaGenerateResponse>()
            .await
            .context("failed to deserialize Ollama response envelope")?;

        let parsed = serde_json::from_str::<ContextInterpretationWire>(&response.response)
            .context("failed to parse structured JSON returned by model")?;

        Ok(parsed.into_domain())
    }
}

#[derive(Debug, Deserialize)]
struct ContextInterpretationWire {
    activity: String,
    confidence: f32,
    summary: String,
    should_comment: bool,
}

impl ContextInterpretationWire {
    fn into_domain(self) -> ContextInterpretation {
        ContextInterpretation {
            activity: parse_activity(&self.activity),
            confidence: self.confidence.clamp(0.0, 1.0),
            summary: self.summary,
            should_comment: self.should_comment,
        }
    }
}

fn parse_activity(value: &str) -> UserActivity {
    match value {
        "Coding" => UserActivity::Coding,
        "Browsing" => UserActivity::Browsing,
        "Watching" => UserActivity::Watching,
        "Chatting" => UserActivity::Chatting,
        "Gaming" => UserActivity::Gaming,
        _ => UserActivity::Unknown,
    }
}

fn build_prompt(
    process_name: &str,
    title: &str,
    heuristic_activity: &UserActivity,
    stable_for_ms: u128,
) -> String {
    format!(
        r#"You are a desktop context interpreter for a personal AI companion.

        Your job:
        - infer what the user is most likely doing
        - use the heuristic activity as a hint, not an absolute truth
        - be conservative and practical
        - do not be verbose

        Return only valid JSON matching the provided schema.

        Input:
        process_name: "{process_name}"
        window_title: "{title}"
        heuristic_activity: "{heuristic_activity:?}"
        stable_for_ms: {stable_for_ms}

        Guidelines:
        - "Coding" if the user is likely programming, debugging, or reading dev docs
        - "Browsing" if generic web navigation or search
        - "Watching" if passive video or streaming consumption
        - "Chatting" if messaging or active communication
        - "Gaming" if likely playing a game
        - "Unknown" if unclear

        For should_comment:
        - false if the user likely needs focus
        - true if a light contextual reaction might be acceptable
        "#,
    )
}
