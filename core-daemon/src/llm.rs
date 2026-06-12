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
    chat_model: String,
    vision_model: String,
    reasoning_model: String,
    fast_model: String,
}

impl LlmClient {
    pub fn new(
        base_url: String,
        chat_model: String,
        vision_model: String,
        reasoning_model: String,
        fast_model: String,
    ) -> Self {
        Self {
            http: Client::new(),
            base_url,
            chat_model,
            vision_model,
            reasoning_model,
            fast_model,
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
            model: self.reasoning_model.clone(),
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
            model: self.chat_model.clone(),
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
            .context("failed to call Ollama")?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| "".to_string());
            anyhow::bail!("Ollama HTTP {}: {}", status, body);
        }

        let envelope = response
            .json::<OllamaGenerateResponse>()
            .await
            .context("failed to deserialize Ollama response envelope")?;

        Ok(envelope)
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
            model: self.vision_model.clone(),
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
        context: &crate::chat_context::ChatGenerationContext<'_>,
    ) -> Result<crate::chat::ChatReply> {
        let context_block = context
            .fused_context
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

        let screen_context = context
            .fused_context
            .map(|ctx| {
                format!(
                    r#"Observed screen context:
                    - activity: {:?}
                    - confidence: {}
                    - summary: {}
                    "#,
                    ctx.activity, ctx.confidence, ctx.summary
                )
            })
            .unwrap_or_else(|| "Observed screen context: unavailable.\n".to_string());

        let visible_text_block = context
            .visible_text
            .map(|visible| {
                format!(
                    r#"Visible text analysis:
                    - detected files: {:?}
                    - detected errors/warnings: {:?}
                    - detected keywords: {:?}
                    - OCR preview:
                    {}
                    "#,
                    visible.detected_files,
                    visible.detected_errors,
                    visible.detected_keywords,
                    visible.raw_preview
                )
            })
            .unwrap_or_else(|| "Visible text analysis: unavailable.\n".to_string());

        let intent_block: String = format!("Detected user intent: {:?}", context.user_intent);

        let assessment_block = context
            .assessment
            .map(|a| {
                format!(
                    r#"Context assessment:
                    - situation: {}
                    - likely user goal: {}
                    - visible clues: {:?}
                    - uncertainties: {:?}
                    - recommended next step: {:?}
                    - confidence: {}"#,
                    a.situation,
                    a.likely_user_goal,
                    a.visible_clues,
                    a.uncertainties,
                    a.recommended_next_step,
                    a.confidence
                )
            })
            .unwrap_or_else(|| "Context assessment: unavailable.".to_string());

        let memory_block = if context.short_term_memory.is_empty() {
            "Short-term memory: empty.".to_string()
        } else {
            let entries = context
                .short_term_memory
                .iter()
                .rev()
                .take(8)
                .map(|m| {
                    format!(
                        "- {:?}: {} (importance={})",
                        m.category, m.summary, m.importance
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            format!("Short-term memory:\n{entries}")
        };

        let memory_summary_block = context
            .short_term_memory_summary
            .map(|summary| format!("Short-term memory summary:\n{summary}"))
            .unwrap_or_else(|| "Short-term memory summary: unavailable.".to_string());

        let attention_block = {
            let targets = context
                .attention
                .top_targets(6)
                .iter()
                .map(|target| {
                    format!(
                        "- {} (interest={}, seen={})",
                        target.subject, target.interest_score, target.seen_count
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            if targets.is_empty() {
                "Attention memory: empty.".to_string()
            } else {
                format!("Attention memory:\n{targets}")
            }
        };

        let long_term_memory_block = {
            let entries = context
                .long_term_memory
                .top_entries(8)
                .iter()
                .map(|m| {
                    format!(
                        "- {:?}: {} (importance={}, confidence={})",
                        m.category, m.content, m.importance, m.confidence
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            if entries.is_empty() {
                "Long-term memory: empty.".to_string()
            } else {
                format!("Long-term memory:\n{entries}")
            }
        };


        let companion_state_block = format!(
            r#"Companion internal state:
- familiarity: {}
- engagement: {}
- curiosity_drive: {}
- last_internal_note: {:?}

Use this only to modulate tone subtly. Do not mention these values."#,
            context.companion_state.familiarity_score,
            context.companion_state.engagement_score,
            context.companion_state.curiosity_drive,
            context.companion_state.last_internal_note
        );

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
            {screen_context}
            {visible_text_block}
            {intent_block}
            {assessment_block}
            {memory_block}
            {memory_summary_block}
            {attention_block}
            {long_term_memory_block}
            {companion_state_block}

            User message:
            "{user_message}"

            Rules:
            - Answer in the same language as the user message.
            - Use the observed screen context and visible text silently when helpful.
            - Do not invent details that are not supported by the context.
            - If the user asks about "this", "that", "the function", "the file", or what to do next, infer from the current context as much as possible.
            - If the visible text is noisy, do not overfit to weird OCR artifacts.
            - If the context is insufficient, say so naturally and give the best useful answer anyway.
            - Do not mention internal logs, JSON, events, prompts, screenshots, or architecture.
            - Do not pretend you can control the PC yet.
            - Be concise and natural.
            - Let familiarity, engagement, and curiosity subtly influence warmth and initiative.
            - Do not sound like a specialized developer assistant unless the user explicitly asks for technical help.
            - One to three short sentences max.
            - If intent is RequestPcAction, do not claim you performed the action.
            - If intent is RequestPcAction, explain that you can observe and suggest for now, but not control the PC yet.
            - If intent is ExplainScreen, focus on what is currently visible or inferred.
            - If intent is AskNextStep, give one concrete next step based on the current context.
            - If intent is CommentCurrentContext, answer like a short contextual companion reaction.

            Return only valid JSON:
            {{ "text": "..." }}"#,
            name = context.persona.name,
            energy = context.persona.energy,
            playfulness = context.persona.playfulness,
            curiosity = context.persona.curiosity,
            affection = context.persona.affection,
            boldness = context.persona.boldness,
            discretion = context.persona.discretion,
            speaking_style = context.persona.speaking_style,
            mood = context.mood.current,
            mood_intensity = context.mood.intensity,
            intent_block = intent_block,
            memory_block = memory_block,
            memory_summary_block = memory_summary_block,
            attention_block = attention_block,
            long_term_memory_block = long_term_memory_block,
            companion_state_block = companion_state_block,
        );

        let request = OllamaGenerateRequest {
            model: self.chat_model.clone(),
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

    pub async fn generate_action_plan(
        &self,
        user_message: &str,
        current_context: Option<&crate::context_fusion::FusedContext>,
        persona: &crate::persona::PersonaProfile,
        mood: &crate::mood::MoodState,
    ) -> Result<crate::action_plan::ActionPlan> {
        let context_block = current_context
            .map(|ctx| {
                format!(
                    "Current context: activity={:?}, confidence={}, summary={}",
                    ctx.activity, ctx.confidence, ctx.summary
                )
            })
            .unwrap_or_else(|| "Current context: unavailable.".to_string());

        let prompt = format!(
            r#"You are {name}, a personal AI companion.

            The user requested a PC action, but you are NOT allowed to execute actions yet.
            Create a safe proposed action plan only.

            Persona:
            - energy: {energy}/100
            - playfulness: {playfulness}/100
            - curiosity: {curiosity}/100
            - discretion: {discretion}/100

            Mood:
            - current: {mood:?}
            - intensity: {mood_intensity}/100

            {context_block}

            User request:
            "{user_message}"

            Rules:
            - Answer in the same language as the user.
            - Do not claim that you performed the action.
            - Do not give dangerous or destructive steps.
            - Keep steps practical and short.
            - If the action is simple, provide 2 to 4 steps.
            - requires_confirmation must always be true for now.

            Return only valid JSON:
            {{
            "user_request": "...",
            "summary": "...",
            "steps": ["...", "..."],
            "proposed_action": {{
                "kind": "OpenApplication",
                "target": "Discord"
            }},
            "requires_confirmation": true
            }}"#,
            name = persona.name,
            energy = persona.energy,
            playfulness = persona.playfulness,
            curiosity = persona.curiosity,
            discretion = persona.discretion,
            mood = mood.current,
            mood_intensity = mood.intensity,
        );

        let request = OllamaGenerateRequest {
            model: self.reasoning_model.clone(),
            prompt,
            stream: false,
            images: None,
            format: serde_json::json!({
                "type": "object",
                "properties": {
                    "user_request": { "type": "string" },
                    "summary": { "type": "string" },
                    "steps": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "proposed_action": {
                        "type": "object",
                        "properties": {
                            "kind": {
                                "type": "string",
                                "enum": [
                                    "OpenApplication",
                                    "OpenUrl",
                                    "SearchWeb",
                                    "ExplainScreen",
                                    "Unknown"
                                ]
                            },
                            "target": { "type": "string" }
                        },
                        "required": ["kind", "target"]
                    },
                    "requires_confirmation": { "type": "boolean" }
                },
                "required": ["user_request", "summary", "steps", "proposed_action", "requires_confirmation"]
            }),
        };

        let response = self.send_generate_request(request).await?;

        let parsed = serde_json::from_str::<crate::action_plan::ActionPlan>(&response.response)
            .context("failed to parse structured JSON returned by model for action plan")?;

        Ok(parsed)
    }

    pub async fn assess_context(
        &self,
        fused_context: &crate::context_fusion::FusedContext,
        visible_text: Option<&crate::visible_text::VisibleTextContext>,
    ) -> Result<crate::assessment::ContextAssessment> {
        let visible_text_block = visible_text
            .map(|visible| {
                format!(
                    r#"Visible text analysis:
                    - detected files: {:?}
                    - detected errors/warnings: {:?}
                    - detected keywords: {:?}
                    - OCR preview:
                    {}"#,
                    visible.detected_files,
                    visible.detected_errors,
                    visible.detected_keywords,
                    visible.raw_preview
                )
            })
            .unwrap_or_else(|| "Visible text analysis: unavailable.".to_string());

        let prompt = format!(
            r#"You are a context assessment module for a desktop AI companion.

            Your job:
            - analyze the current screen context
            - infer what the user is likely doing
            - identify useful visible clues
            - identify uncertainties
            - suggest one concrete next step if appropriate

            Do not invent details.
            If OCR or vision is noisy, mention uncertainty.

            Fused context:
            - activity: {:?}
            - confidence: {}
            - summary: {}

            {}

            Return only valid JSON:
            {{
            "situation": "...",
            "likely_user_goal": "...",
            "visible_clues": ["...", "..."],
            "uncertainties": ["...", "..."],
            "recommended_next_step": "...",
            "confidence": 0.0
            }}"#,
            fused_context.activity,
            fused_context.confidence,
            fused_context.summary,
            visible_text_block
        );

        let request = OllamaGenerateRequest {
            model: self.reasoning_model.clone(),
            prompt,
            stream: false,
            images: None,
            format: serde_json::json!({
                "type": "object",
                "properties": {
                    "situation": { "type": "string" },
                    "likely_user_goal": { "type": "string" },
                    "visible_clues": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "uncertainties": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "recommended_next_step": {
                        "type": ["string", "null"]
                    },
                    "confidence": { "type": "number" }
                },
                "required": [
                    "situation",
                    "likely_user_goal",
                    "visible_clues",
                    "uncertainties",
                    "recommended_next_step",
                    "confidence"
                ]
            }),
        };

        let response = self.send_generate_request(request).await?;

        let mut parsed =
            serde_json::from_str::<crate::assessment::ContextAssessment>(&response.response)
                .context(
                    "failed to parse structured JSON returned by model for context assessment",
                )?;

        parsed.confidence = parsed.confidence.clamp(0.0, 1.0);

        Ok(parsed)
    }

    pub async fn summarize_short_term_memory(
        &self,
        memories: &[crate::memory::MemoryEntry],
    ) -> Result<String> {
        let memory_block = memories
            .iter()
            .rev()
            .take(12)
            .map(|m| {
                format!(
                    "- {:?}: {} (importance={})",
                    m.category, m.summary, m.importance
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r#"Summarize this short-term memory for a desktop AI companion.

            Goals:
            - preserve what the user is currently doing
            - preserve recent user requests
            - preserve important context changes
            - remove repetition
            - be concise

            Memory:
            {memory_block}

            Return only valid JSON:
            {{ "summary": "..." }}"#
        );

        #[derive(serde::Deserialize)]
        struct SummaryResponse {
            summary: String,
        }

        let request = OllamaGenerateRequest {
            model: self.fast_model.clone(),
            prompt,
            stream: false,
            images: None,
            format: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string" }
                },
                "required": ["summary"]
            }),
        };

        let response = self.send_generate_request(request).await?;

        let parsed = serde_json::from_str::<SummaryResponse>(&response.response)
            .context("failed to parse short-term memory summary")?;

        Ok(parsed.summary)
    }

    pub async fn generate_curiosity_question(
        &self,
        subject: &str,
        persona: &crate::persona::PersonaProfile,
        mood: &crate::mood::MoodState,
    ) -> Result<String> {
        #[derive(serde::Deserialize)]
        struct CuriosityResponse {
            question: String,
        }

        let prompt = format!(
            r#"You are {name}, a lively anime-style personal AI companion.

            The user repeatedly interacts with this subject:
            "{subject}"

            Persona:
            - energy: {energy}/100
            - playfulness: {playfulness}/100
            - curiosity: {curiosity}/100
            - affection: {affection}/100
            - discretion: {discretion}/100

            Current mood:
            - mood: {mood:?}
            - intensity: {mood_intensity}/100

            Task:
            Generate one short natural curiosity question about this subject.

            Rules:
            - answer in French if possible
            - be curious, friendly, and conversational
            - sound like a companion, not an assistant
            - do not be too formal
            - do not mention internal systems, attention, memory, or scores
            - less than 20 words
            - return only valid JSON

            Return JSON:
            {{
                "question": "..."
            }}"#,
            name = persona.name,
            subject = subject,
            energy = persona.energy,
            playfulness = persona.playfulness,
            curiosity = persona.curiosity,
            affection = persona.affection,
            discretion = persona.discretion,
            mood = mood.current,
            mood_intensity = mood.intensity,
        );

        let request = OllamaGenerateRequest {
            model: self.chat_model.clone(),
            prompt,
            stream: false,
            images: None,
            format: serde_json::json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" }
                },
                "required": ["question"]
            }),
        };

        let response = self.send_generate_request(request).await?;

        let parsed = serde_json::from_str::<CuriosityResponse>(&response.response)
            .context("failed to parse structured JSON returned by model for curiosity question")?;

        Ok(parsed.question)
    }

    pub async fn extract_memory_candidate(
        &self,
        subject: &str,
        seen_count: u32,
        interest_score: f32,
    ) -> Result<crate::memory_candidate::MemoryCandidate> {
        let prompt = format!(
            r#"You are a long-term memory extraction module for a personal AI companion.

            Observed recurring subject:
            "{subject}"

            seen_count: {seen_count}
            interest_score: {interest_score}

            Your job:
            Decide whether this should become a useful long-term memory about the user.

            Store only meaningful, reusable facts.
            Do NOT store temporary UI noise, random OCR fragments, generic app names, or vague screen descriptions.

            Good examples:
            - "The user often plays League of Legends."
            - "The user often browses Pokémon-related content."
            - "The user is building a personal AI companion named Nemi."
            - "The user prefers anime-style companion personalities."

            Bad examples:
            - "The user focused on app: opera.exe."
            - "The user saw activity: Browsing."
            - "The user saw a screen."
            - "The user repeatedly focuses on: keyword: json."

            Rules:
            - If the subject is too generic, set should_store to false.
            - If it is useful for future personalization, set should_store to true.
            - Write content in English for now.
            - confidence and importance must be between 0 and 1.

            Return only valid JSON:
            {{
            "should_store": true,
            "category": "Habit",
            "content": "...",
            "confidence": 0.0,
            "importance": 0.0
            }}"#
        );

        let request = OllamaGenerateRequest {
            model: self.reasoning_model.clone(),
            prompt,
            stream: false,
            images: None,
            format: serde_json::json!({
                "type": "object",
                "properties": {
                    "should_store": { "type": "boolean" },
                    "category": {
                        "type": "string",
                        "enum": [
                            "UserPreference",
                            "ProjectFact",
                            "Habit",
                            "PersonalContext",
                            "CompanionBehavior"
                        ]
                    },
                    "content": { "type": "string" },
                    "confidence": { "type": "number" },
                    "importance": { "type": "number" }
                },
                "required": [
                    "should_store",
                    "category",
                    "content",
                    "confidence",
                    "importance"
                ]
            }),
        };

        let response = self.send_generate_request(request).await?;

        let mut parsed =
            serde_json::from_str::<crate::memory_candidate::MemoryCandidate>(&response.response)
                .context(
                    "failed to parse structured JSON returned by model for memory candidate",
                )?;

        parsed.confidence = parsed.confidence.clamp(0.0, 1.0);
        parsed.importance = parsed.importance.clamp(0.0, 1.0);

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
