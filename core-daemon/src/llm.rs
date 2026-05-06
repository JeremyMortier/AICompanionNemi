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

    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
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
            images: None,
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
        recent_reactions: &[String],
        persona: &crate::persona::PersonaProfile,
        mood: &crate::mood::MoodState,
    ) -> Result<GeneratedReaction> {
        let prompt =
            build_reaction_prompt(interpretation, decision, recent_reactions, persona, mood);

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
            images: None,
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

    pub async fn interpret_vision(
        &self,
        image_path: &str,
        process_name: &str,
        window_title: &str,
        heuristic_activity: &UserActivity,
    ) -> Result<crate::vision::VisionInterpretation> {
        use base64::{Engine as _, engine::general_purpose};

        let image_bytes = std::fs::read(image_path)
            .with_context(|| format!("failed to read image file: {image_path}"))?;

        let image_base64 = general_purpose::STANDARD.encode(image_bytes);

        let prompt = format!(
            r#"You are analyzing a screenshot of a user's computer.

            Reliable system metadata:
            process_name: "{process_name}"
            window_title: "{window_title}"
            heuristic_activity: "{heuristic_activity:?}"

            Important rules:
            - The system metadata is usually more reliable than visual guessing.
            - If process_name is Discord, do not classify it as a finance or crypto app unless the screenshot clearly shows finance/crypto content.
            - If process_name is Code.exe or similar IDE/editor, prefer Coding unless the screenshot clearly shows something else.
            - If process_name is a browser, use both the title and the visible page to infer the activity.
            - Do not invent app names.
            - Be conservative.

            Tasks:
            - identify what the user is doing visually
            - refine or correct the heuristic activity
            - briefly describe what is actually visible on screen

            Return only valid JSON matching the schema."#
        );

        let request = OllamaGenerateRequest {
            model: "llava:7b".to_string(),
            prompt,
            stream: false,
            images: Some(vec![image_base64]),
            format: serde_json::json!({
                "type": "object",
                "properties": {
                    "detected_activity": {
                        "type": "string",
                        "enum": ["Coding", "Browsing", "Watching", "Chatting", "Gaming", "Unknown"]
                    },
                    "confidence": {
                        "type": "number"
                    },
                    "description": {
                        "type": "string"
                    }
                },
                "required": ["detected_activity", "confidence", "description"]
            }),
        };

        let response = self.send_generate_request(request).await?;

        let parsed = serde_json::from_str::<VisionInterpretationWire>(&response.response).context(
            "failed to parse structured JSON returned by model for vision interpretation",
        )?;

        Ok(parsed.into_domain())
    }

    pub async fn generate_chat_reply(
        &self,
        user_message: &str,
        current_context: Option<&crate::context_fusion::FusedContext>,
        persona: &crate::persona::PersonaProfile,
        mood: &crate::mood::MoodState,
    ) -> Result<crate::chat::ChatReply> {
        let context_block = current_context
            .map(|ctx| {
                format!(
                    r#"Current observed screen context:
        - inferred activity: {:?}
        - confidence: {}
        - observation: {}

        Use this context as background awareness.
        Do not explicitly mention it unless it helps answer the user.
        If the context is weak or ambiguous, do not invent details.
        "#,
                    ctx.activity, ctx.confidence, ctx.summary
                )
            })
            .unwrap_or_else(|| {
                "Current observed screen context: unavailable or unreliable.".to_string()
            });

        let prompt = format!(
            r#"You are {name}, a lively anime-style personal AI companion for a private desktop setup.

        Persona:
        - energy: {energy}/100
        - playfulness: {playfulness}/100
        - curiosity: {curiosity}/100
        - affection: {affection}/100
        - boldness: {boldness}/100
        - discretion: {discretion}/100
        - speaking_style: {speaking_style:?}

        Mood:
        - current: {mood:?}
        - intensity: {mood_intensity}/100

        {context_block}

        User message:
        "{user_message}"

        Rules:
        - Answer in the same language as the user message.
        - Use the observed screen context silently when it helps.
        - Do not invent details that are not supported by the context.
        - If the user asks about "this", "that", "the function", "the file", or what to do next, infer from the current context as much as possible.
        - If the context is insufficient, say so naturally and give the best useful answer anyway.
        - Do not mention internal logs, JSON, events, prompts, screenshots, or architecture.
        - Do not pretend you can control the PC yet.
        - Be concise and natural.
        - One to three short sentences max.

        Return only valid JSON:
        {{ "text": "..." }}"#,
            name = persona.name,
            energy = persona.energy,
            playfulness = persona.playfulness,
            curiosity = persona.curiosity,
            affection = persona.affection,
            boldness = persona.boldness,
            discretion = persona.discretion,
            speaking_style = persona.speaking_style,
            mood = mood.current,
            mood_intensity = mood.intensity,
        );

        let request = OllamaGenerateRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
            images: None,
            format: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" }
                },
                "required": ["text"]
            }),
        };

        let response = self.send_generate_request(request).await?;

        let parsed = serde_json::from_str::<crate::chat::ChatReply>(&response.response)
            .context("failed to parse structured JSON returned by model for chat reply")?;

        Ok(parsed)
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
Use the screen context silently when helpful.
Do not mention it unless relevant.
If the user's language is French, answer in French.
If the context is uncertain, be transparent but still useful.

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
    recent_reactions: &[String],
    persona: &crate::persona::PersonaProfile,
    mood: &crate::mood::MoodState,
) -> String {
    let decision_label = match decision {
        ReactionDecision::StaySilent { .. } => "StaySilent",
        ReactionDecision::LightComment { .. } => "LightComment",
        ReactionDecision::CuriousComment { .. } => "CuriousComment",
    };

    let recent_reactions_block = if recent_reactions.is_empty() {
        "none".to_string()
    } else {
        recent_reactions
            .iter()
            .enumerate()
            .map(|(idx, text)| format!("{}. {}", idx + 1, text))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"You are {name}, a lively anime-style personal AI companion for a private desktop setup.

Persona:
- energy: {energy}/100
- playfulness: {playfulness}/100
- curiosity: {curiosity}/100
- affection: {affection}/100
- boldness: {boldness}/100
- discretion: {discretion}/100
- speaking_style: {speaking_style:?}

Current mood:
- mood: {mood_name:?}
- mood_intensity: {mood_intensity}/100

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
- adapt wording to both persona and current mood
- high mood intensity should be noticeable but still controlled
- if mood is Playful, sound a bit more lively
- if mood is Curious, sound a bit more intrigued or observant
- if mood is Focused, sound more restrained and precise
- if mood is Calm, sound softer and more relaxed
- if mood is Proud, sound slightly confident
- if mood is Sulky, sound mildly pouty but still subtle
- no emojis unless they feel very natural and minimal

Context:
activity: "{activity:?}"
confidence: {confidence}
summary: "{summary}"
decision: "{decision_label}"

Recent reactions to avoid repeating:
{recent_reactions_block}

Behavior guide:
- if decision is LightComment, make a soft, brief observation
- if decision is CuriousComment, make a slightly more engaged remark
- do not ask too many questions
- avoid repeating the summary verbatim
- avoid repeating any recent reaction
- use different wording if the recent reactions are similar
- never mention internal system details

Return only valid JSON matching the schema."#,
        name = persona.name,
        energy = persona.energy,
        playfulness = persona.playfulness,
        curiosity = persona.curiosity,
        affection = persona.affection,
        boldness = persona.boldness,
        discretion = persona.discretion,
        speaking_style = persona.speaking_style,
        mood_name = mood.current,
        mood_intensity = mood.intensity,
        activity = interpretation.activity,
        confidence = interpretation.confidence,
        summary = interpretation.summary,
        recent_reactions_block = recent_reactions_block,
    )
}

#[derive(Debug, Deserialize)]
struct VisionInterpretationWire {
    detected_activity: String,
    confidence: f32,
    description: String,
}

impl VisionInterpretationWire {
    fn into_domain(self) -> crate::vision::VisionInterpretation {
        crate::vision::VisionInterpretation {
            detected_activity: parse_activity(&self.detected_activity),
            confidence: self.confidence.clamp(0.0, 1.0),
            description: self.description,
        }
    }
}
