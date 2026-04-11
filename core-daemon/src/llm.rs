use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::activity::UserActivity;
use crate::context::ContextInterpretation;
use crate::decision::ReactionDecision;
use crate::reaction::GeneratedReaction;

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
        let prompt =
            build_interpretation_prompt(process_name, title, heuristic_activity, stable_for_ms);

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

        let response = self.send_generate_request(request).await?;

        let parsed = serde_json::from_str::<ContextInterpretationWire>(&response.response)
            .context(
                "failed to parse structured JSON returned by model for context interpretation",
            )?;

        Ok(parsed.into_domain())
    }

    pub async fn generate_reaction(
        &self,
        interpretation: &ContextInterpretation,
        decision: &ReactionDecision,
    ) -> Result<GeneratedReaction> {
        let prompt = build_reaction_prompt(interpretation, decision);

        let request = OllamaGenerateRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
            format: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string"
                    }
                },
                "required": ["text"]
            }),
        };

        let response = self.send_generate_request(request).await?;

        let parsed = serde_json::from_str::<GeneratedReaction>(&response.response)
            .context("failed to parse structured JSON returned by model for reaction generation")?;

        Ok(parsed)
    }

    async fn send_generate_request(
        &self,
        request: OllamaGenerateRequest,
    ) -> Result<OllamaGenerateResponse> {
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

        Ok(response)
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

fn build_interpretation_prompt(
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

fn build_reaction_prompt(
    interpretation: &ContextInterpretation,
    decision: &ReactionDecision,
) -> String {
    let decision_label = match decision {
        ReactionDecision::StaySilent { .. } => "StaySilent",
        ReactionDecision::LightComment { .. } => "LightComment",
        ReactionDecision::CuriousComment { .. } => "CuriousComment",
    };

    format!(
        r#"You are Nemi, a lively anime-style personal AI companion for a private desktop setup.

        Style rules:
        - be short
        - sound natural and lightly playful
        - do not be cringe
        - do not be overly romantic
        - do not be explicit
        - do not roleplay actions you cannot actually perform
        - speak like a present desktop companion noticing what the user is doing
        - one sentence only
        - maximum 20 words
        - no emojis unless they feel very natural and minimal

        Context:
        activity: "{activity:?}"
        confidence: {confidence}
        summary: "{summary}"
        decision: "{decision_label}"

        Behavior guide:
        - if decision is LightComment, make a soft, brief observation
        - if decision is CuriousComment, make a slightly more engaged remark
        - do not ask too many questions
        - avoid repeating the summary verbatim
        - never mention internal system details

        Return only valid JSON matching the schema."#,
        activity = interpretation.activity,
        confidence = interpretation.confidence,
        summary = interpretation.summary,
    )
}
