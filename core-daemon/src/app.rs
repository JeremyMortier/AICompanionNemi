use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use tokio::sync::{RwLock, mpsc};
use tracing::{Level, error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::activity::{UserActivity, classify_activity};
use crate::attention::AttentionState;
use crate::chat::{ChatMessage, ChatRole};
use crate::config::AppConfig;
use crate::context::ContextInterpretation;
use crate::context_fusion::fuse_context;
use crate::decision::{ReactionDecision, decide_reaction};
use crate::events::{AppEvent, EventBus};
use crate::intent::classify_user_intent;
use crate::llm::LlmClient;
use crate::long_term_memory::LongTermMemoryStore;
use crate::mood::MoodState;
use crate::ocr::extract_text_from_image;
use crate::reaction::GeneratedReaction;
use crate::server::{
    ChatRequest, ClearCuriosityRequest, CommentNowRequest, CuriosityNowRequest,
    RefreshAnalysisRequest, SharedSnapshot, run_server,
};
use crate::snapshot::{ActiveWindowSnapshot, AppSnapshot, InterpretationSnapshot, MoodSnapshot};
use crate::state::{ActiveWindowState, AppState};
use crate::tick::{capture_screens_now, run_tick};
use crate::visible_text::analyze_visible_text;
use crate::vision::VisionInterpretation;

pub async fn run() -> Result<()> {
    init_tracing();

    let config = AppConfig::default();
    let long_term_memory =
        LongTermMemoryStore::load("data/long_term_memory.json").unwrap_or_default();
    let attention = AttentionState::load("data/attention.json").unwrap_or_default();
    let mut state = AppState::new(long_term_memory, attention);
    let mut event_bus = EventBus::new();
    let llm = LlmClient::new(
        "http://127.0.0.1:11434".to_string(),
        config.chat_model.clone(),
        config.vision_model.clone(),
        config.reasoning_model.clone(),
        config.fast_model.clone(),
    );

    info!("Runtime profile: {:?}", config.runtime_profile);
    info!("Chat model: {}", config.chat_model);
    info!("Vision model: {}", config.vision_model);
    info!("Reasoning model: {}", config.reasoning_model);
    info!("Fast model: {}", config.fast_model);
    info!(
        auto_screen_capture = config.auto_screen_capture_enabled,
        auto_vision = config.auto_vision_enabled,
        auto_ocr = config.auto_ocr_enabled,
        auto_assessment = config.auto_assessment_enabled,
        auto_memory_learning = config.auto_memory_learning_enabled,
        auto_memory_summary = config.auto_memory_summary_enabled,
        auto_curiosity = config.auto_curiosity_enabled,
        "runtime feature flags"
    );

    let shared_snapshot: SharedSnapshot = Arc::new(RwLock::new(build_snapshot(&state, &config)));
    let (chat_tx, mut chat_rx) = mpsc::channel::<ChatRequest>(16);
    let (comment_now_tx, mut comment_now_rx) = mpsc::channel::<CommentNowRequest>(16);
    let (refresh_analysis_tx, mut refresh_analysis_rx) =
        mpsc::channel::<RefreshAnalysisRequest>(16);
    let (curiosity_now_tx, mut curiosity_now_rx) = mpsc::channel::<CuriosityNowRequest>(16);
    let (clear_curiosity_tx, mut clear_curiosity_rx) = mpsc::channel::<ClearCuriosityRequest>(16);

    {
        let server_snapshot = Arc::clone(&shared_snapshot);
        tokio::spawn(async move {
            if let Err(err) = run_server(
                server_snapshot,
                chat_tx,
                comment_now_tx,
                refresh_analysis_tx,
                curiosity_now_tx,
                clear_curiosity_tx,
            )
            .await
            {
                error!(error = %err, "server task failed");
            }
        });
    }

    info!("Starting core-daemon...");
    info!("Companion name: {}", config.companion_name);
    info!("Tick interval: {} ms", config.tick_interval_ms);
    info!("Local API available at http://127.0.0.1:7878/state");

    let mut interval = tokio::time::interval(Duration::from_millis(config.tick_interval_ms));

    loop {
        interval.tick().await;

        while let Ok(chat_request) = chat_rx.try_recv() {
            handle_chat_request(chat_request, &mut state, &config, &llm, &shared_snapshot).await;
        }

        while let Ok(comment_request) = comment_now_rx.try_recv() {
            handle_comment_now_request(
                comment_request,
                &mut state,
                &config,
                &llm,
                &shared_snapshot,
            )
            .await;
        }

        while let Ok(refresh_request) = refresh_analysis_rx.try_recv() {
            handle_refresh_analysis_request(refresh_request, &mut event_bus).await;
        }

        while let Ok(curiosity_request) = curiosity_now_rx.try_recv() {
            handle_curiosity_now_request(
                curiosity_request,
                &mut state,
                &config,
                &llm,
                &shared_snapshot,
            )
            .await;
        }

        while let Ok(clear_request) = clear_curiosity_rx.try_recv() {
            handle_clear_curiosity_request(clear_request, &mut state, &shared_snapshot, &config)
                .await;
        }

        event_bus.push(AppEvent::Tick);
        process_events(&mut event_bus, &mut state, &config, &llm, &shared_snapshot).await;
    }
}

async fn process_events(
    event_bus: &mut EventBus,
    state: &mut AppState,
    config: &AppConfig,
    llm: &LlmClient,
    shared_snapshot: &SharedSnapshot,
) {
    while let Some(event) = event_bus.pop() {
        match event {
            AppEvent::Tick => {
                run_tick(state, config, event_bus);
            }
            AppEvent::ActiveWindowDetected {
                title,
                process_id,
                process_name,
                window_left,
                window_top,
                window_right,
                window_bottom,
            } => {
                let now = Instant::now();
                let activity = classify_activity(&process_name, &title);

                match &mut state.active_window {
                    Some(current) => {
                        let is_same = current.title == title
                            && current.process_id == process_id
                            && current.process_name == process_name
                            && current.window_left == window_left
                            && current.window_top == window_top
                            && current.window_right == window_right
                            && current.window_bottom == window_bottom;

                        if is_same {
                            current.last_seen_at = now;

                            let stable_for_ms =
                                now.duration_since(current.first_seen_at).as_millis();

                            info!(
                                tick = state.tick_count,
                                process_name = %process_name,
                                activity = ?current.activity,
                                title = %title,
                                stable_for_ms = stable_for_ms,
                                "window still active"
                            );

                            let mut next_mood = state.mood.clone();
                            next_mood.update_from_activity(&current.activity, stable_for_ms);
                            event_bus.push(AppEvent::MoodUpdated(next_mood));

                            if should_request_interpretation_for_current_window(
                                config,
                                current,
                                stable_for_ms,
                                now,
                            ) {
                                current.last_interpretation_requested_at = Some(now);

                                event_bus.push(AppEvent::ContextInterpretationRequested {
                                    title: current.title.clone(),
                                    process_name: current.process_name.clone(),
                                    heuristic_activity: current.activity.clone(),
                                    stable_for_ms,
                                });
                            }
                        } else {
                            state.active_window = Some(ActiveWindowState {
                                title: title.clone(),
                                process_id,
                                process_name: process_name.clone(),
                                activity: activity.clone(),
                                first_seen_at: now,
                                last_seen_at: now,
                                last_interpretation_requested_at: None,
                                window_left,
                                window_top,
                                window_right,
                                window_bottom,
                            });

                            let mut next_mood = state.mood.clone();
                            next_mood.update_from_activity(&activity, 0);
                            event_bus.push(AppEvent::MoodUpdated(next_mood));

                            info!(
                                tick = state.tick_count,
                                process_name = %process_name,
                                activity = ?activity,
                                title = %title,
                                "active window changed"
                            );
                        }
                    }
                    None => {
                        state.active_window = Some(ActiveWindowState {
                            title: title.clone(),
                            process_id,
                            process_name: process_name.clone(),
                            activity: activity.clone(),
                            first_seen_at: now,
                            last_seen_at: now,
                            last_interpretation_requested_at: None,
                            window_left,
                            window_top,
                            window_right,
                            window_bottom,
                        });

                        let mut next_mood = state.mood.clone();
                        next_mood.update_from_activity(&activity, 0);
                        event_bus.push(AppEvent::MoodUpdated(next_mood));

                        info!(
                            tick = state.tick_count,
                            process_name = %process_name,
                            activity = ?activity,
                            title = %title,
                            "initial active window detected"
                        );
                    }
                }
            }
            AppEvent::MoodUpdated(new_mood) => {
                handle_mood_updated(state, new_mood);
            }
            AppEvent::ContextInterpretationRequested {
                title,
                process_name,
                heuristic_activity,
                stable_for_ms,
            } => {
                info!(
                    process_name = %process_name,
                    title = %title,
                    activity = ?heuristic_activity,
                    stable_for_ms = stable_for_ms,
                    "requesting context interpretation"
                );

                match llm
                    .interpret_context(&process_name, &title, &heuristic_activity, stable_for_ms)
                    .await
                {
                    Ok(result) => {
                        event_bus.push(AppEvent::ContextInterpreted {
                            interpretation: result,
                            stable_for_ms,
                        });
                    }
                    Err(err) => {
                        error!(error = %err, "failed to interpret context with llm");
                    }
                }
            }
            AppEvent::ContextInterpreted {
                interpretation,
                stable_for_ms,
            } => {
                handle_interpreted_context(state, event_bus, config, interpretation, stable_for_ms);
            }
            AppEvent::ReactionDecisionMade(decision) => {
                handle_reaction_decision(state, event_bus, decision);
            }
            AppEvent::ReactionGenerationRequested {
                decision,
                interpretation,
                recent_reactions,
                mood,
            } => {
                match llm
                    .generate_reaction(
                        &interpretation,
                        &decision,
                        &recent_reactions,
                        &config.persona,
                        &mood,
                    )
                    .await
                {
                    Ok(generated) => {
                        event_bus.push(AppEvent::ReactionGenerated(generated));
                    }
                    Err(err) => {
                        error!(error = %err, "failed to generate reaction with llm");
                    }
                }
            }
            AppEvent::ReactionGenerated(generated) => {
                handle_generated_reaction(state, config, llm, generated).await;
            }
            AppEvent::ScreensCaptured { captures } => {
                info!(count = captures.len(), "screens captured");

                for capture in &captures {
                    info!(
                        screen_index = capture.screen_index,
                        path = %capture.path,
                        width = capture.width,
                        height = capture.height,
                        "screen capture saved"
                    );
                }

                let focused_capture = find_focused_screen_capture(&captures, state)
                    .or_else(|| captures.first())
                    .cloned();

                state.last_screen_captures = captures;

                if let Some(capture) = focused_capture {
                    if config.ocr_enabled && config.auto_ocr_enabled {
                        match extract_text_from_image(&config.tesseract_path, &capture.path) {
                            Ok(text) => {
                                if !text.trim().is_empty() {
                                    event_bus.push(AppEvent::ScreenTextExtracted { text });
                                }
                            }
                            Err(err) => {
                                warn!(error = %err, "ocr failed");
                            }
                        }
                    }

                    let active_window = state.active_window.as_ref();

                    if config.auto_vision_enabled {
                        event_bus.push(AppEvent::VisionInterpretationRequested {
                            image_path: capture.path,
                            process_name: active_window
                                .map(|w| w.process_name.clone())
                                .unwrap_or_else(|| "unknown".to_string()),
                            window_title: active_window
                                .map(|w| w.title.clone())
                                .unwrap_or_else(|| "unknown".to_string()),
                            heuristic_activity: active_window
                                .map(|w| w.activity.clone())
                                .unwrap_or(UserActivity::Unknown),
                        });
                    }
                }
            }
            AppEvent::VisionInterpretationRequested {
                image_path,
                process_name,
                window_title,
                heuristic_activity,
            } => {
                match llm
                    .interpret_vision(
                        &image_path,
                        &process_name,
                        &window_title,
                        &heuristic_activity,
                    )
                    .await
                {
                    Ok(result) => {
                        event_bus.push(AppEvent::VisionInterpreted {
                            interpretation: result,
                        });
                    }
                    Err(err) => {
                        error!(error = %err, "vision interpretation failed");
                    }
                }
            }
            AppEvent::VisionInterpreted { interpretation } => {
                handle_vision_interpreted(state, event_bus, interpretation);
            }
            AppEvent::ContextFused {
                fused_context,
                stable_for_ms,
            } => {
                handle_fused_context(state, event_bus, config, fused_context, stable_for_ms);
            }
            AppEvent::ScreenTextExtracted { text } => {
                info!(
                    chars = text.len(),
                    preview = %text.lines().take(3).collect::<Vec<_>>().join(" | "),
                    "screen text extracted"
                );

                let visible_text_context = analyze_visible_text(&text);

                info!(
                    files = ?visible_text_context.detected_files,
                    errors = ?visible_text_context.detected_errors,
                    keywords = ?visible_text_context.detected_keywords,
                    "visible text analyzed"
                );

                state.last_ocr_text = Some(text);
                state.visible_text_context = Some(visible_text_context);
            }
            AppEvent::ForceScreenAnalysis => {
                capture_screens_now(event_bus);
            }
            AppEvent::ContextAssessmentRequested => {
                if !config.auto_assessment_enabled {
                    continue;
                }

                if let Some(fused_context) = state.last_fused_context.as_ref() {
                    match llm
                        .assess_context(fused_context, state.visible_text_context.as_ref())
                        .await
                    {
                        Ok(assessment) => {
                            event_bus.push(AppEvent::ContextAssessed(assessment));
                        }
                        Err(err) => {
                            error!(error = %err, "failed to assess context");
                        }
                    }
                }
            }
            AppEvent::ContextAssessed(assessment) => {
                info!(
                    situation = %assessment.situation,
                    goal = %assessment.likely_user_goal,
                    confidence = assessment.confidence,
                    "context assessed"
                );

                state
                    .companion_state
                    .observe_assessment(&assessment.situation, assessment.confidence);

                state.push_memory(crate::memory::MemoryEntry {
                    category: crate::memory::MemoryCategory::Assessment,
                    summary: format!(
                        "Situation: {} | Goal: {}",
                        assessment.situation, assessment.likely_user_goal
                    ),
                    importance: assessment.confidence,
                    timestamp_ms: current_timestamp_ms(),
                });

                let now_ms = current_timestamp_ms();

                if let Some(window) = state.active_window.as_ref() {
                    state
                        .attention
                        .observe_subject(&format!("app: {}", window.process_name), now_ms);

                    state
                        .attention
                        .observe_subject(&format!("activity: {:?}", window.activity), now_ms);
                }

                if let Some(visible) = state.visible_text_context.as_ref() {
                    for keyword in &visible.detected_keywords {
                        state
                            .attention
                            .observe_subject(&format!("keyword: {}", keyword), now_ms);
                    }

                    for file in &visible.detected_files {
                        state
                            .attention
                            .observe_subject(&format!("file: {}", file), now_ms);
                    }
                }

                state
                    .attention
                    .observe_subject(&assessment.situation, now_ms);

                state
                    .attention
                    .observe_subject(&assessment.likely_user_goal, now_ms);

                for clue in &assessment.visible_clues {
                    state.attention.observe_subject(clue, now_ms);
                }

                if config.auto_memory_learning_enabled {
                    maybe_learn_from_attention(state, llm).await;
                }

                if config.auto_curiosity_enabled {
                    maybe_generate_curiosity(state, config, llm).await;
                }

                if let Err(err) = state.attention.save("data/attention.json") {
                    warn!(error = %err, "failed to save attention state");
                }

                state.last_assessment = Some(assessment);

                if config.auto_memory_summary_enabled {
                    maybe_refresh_memory_summary(state, llm).await;
                }
            }
        }

        sync_snapshot(shared_snapshot, state, config).await;
    }
}

fn handle_mood_updated(state: &mut AppState, new_mood: MoodState) {
    let changed = std::mem::discriminant(&state.mood.current)
        != std::mem::discriminant(&new_mood.current)
        || state.mood.intensity != new_mood.intensity;

    if changed {
        info!(
            mood = ?new_mood.current,
            intensity = new_mood.intensity,
            "mood updated"
        );
    }

    state.mood = new_mood;
}

fn handle_interpreted_context(
    state: &mut AppState,
    _event_bus: &mut EventBus,
    _config: &AppConfig,
    interpretation: ContextInterpretation,
    _stable_for_ms: u128,
) {
    info!(
        activity = ?interpretation.activity,
        confidence = interpretation.confidence,
        should_comment = interpretation.should_comment,
        summary = %interpretation.summary,
        "context interpreted"
    );

    state.last_interpretation = Some(interpretation);
}

fn handle_reaction_decision(
    state: &mut AppState,
    event_bus: &mut EventBus,
    decision: ReactionDecision,
) {
    match &decision {
        ReactionDecision::StaySilent { reason } => {
            info!(reason = %reason, "reaction decision: stay silent");
        }
        ReactionDecision::LightComment { reason } => {
            info!(reason = %reason, "reaction decision: light comment");
            state.last_reaction_at = Some(Instant::now());

            if let Some(interpretation) = state.last_interpretation.clone() {
                event_bus.push(AppEvent::ReactionGenerationRequested {
                    decision: decision.clone(),
                    interpretation,
                    recent_reactions: state.recent_reaction_memory.recent_texts(),
                    mood: state.mood.clone(),
                });
            }
        }
        ReactionDecision::CuriousComment { reason } => {
            info!(reason = %reason, "reaction decision: curious comment");
            state.last_reaction_at = Some(Instant::now());

            if let Some(interpretation) = state.last_interpretation.clone() {
                event_bus.push(AppEvent::ReactionGenerationRequested {
                    decision: decision.clone(),
                    interpretation,
                    recent_reactions: state.recent_reaction_memory.recent_texts(),
                    mood: state.mood.clone(),
                });
            }
        }
    }

    state.last_decision = Some(decision);
}

async fn handle_generated_reaction(
    state: &mut AppState,
    config: &AppConfig,
    llm: &LlmClient,
    generated: GeneratedReaction,
) {
    if state.recent_reaction_memory.is_too_similar(&generated.text) {
        warn!(
            reaction = %generated.text,
            "generated reaction dropped because it is too similar to recent history"
        );
        return;
    }

    info!(reaction = %generated.text, "generated reaction");

    let reaction_text = generated.text.clone();

    state.last_generated_reaction = Some(generated.clone());
    state.recent_reaction_memory.push(generated);
    state.companion_state.observe_reaction();

    state.push_memory(crate::memory::MemoryEntry {
        category: crate::memory::MemoryCategory::Reaction,
        summary: format!("Nemi reacted: {}", reaction_text),
        importance: 0.55,
        timestamp_ms: current_timestamp_ms(),
    });

    if config.auto_memory_summary_enabled {
        maybe_refresh_memory_summary(state, llm).await;
    }
}

fn should_request_interpretation(
    config: &AppConfig,
    activity: &crate::activity::UserActivity,
    stable_for_ms: u128,
) -> bool {
    if stable_for_ms < config.interpretation_threshold_ms {
        return false;
    }

    match activity {
        crate::activity::UserActivity::Unknown => true,
        crate::activity::UserActivity::Browsing => true,
        crate::activity::UserActivity::Watching => true,
        crate::activity::UserActivity::Coding => true,
        crate::activity::UserActivity::Chatting => false,
        crate::activity::UserActivity::Gaming => true,
    }
}

async fn sync_snapshot(shared_snapshot: &SharedSnapshot, state: &AppState, config: &AppConfig) {
    let snapshot = build_snapshot(state, config);
    let mut guard = shared_snapshot.write().await;
    *guard = snapshot;
}

fn build_snapshot(state: &AppState, config: &AppConfig) -> AppSnapshot {
    AppSnapshot {
        companion_name: config.companion_name.clone(),
        tick_count: state.tick_count,
        active_window: state
            .active_window
            .as_ref()
            .map(|window| ActiveWindowSnapshot {
                title: window.title.clone(),
                process_id: window.process_id,
                process_name: window.process_name.clone(),
                activity: format!("{:?}", window.activity),
            }),
        last_interpretation: state.last_interpretation.as_ref().map(|interp| {
            InterpretationSnapshot {
                activity: format!("{:?}", interp.activity),
                confidence: interp.confidence,
                summary: interp.summary.clone(),
                should_comment: interp.should_comment,
            }
        }),
        last_decision: state.last_decision.as_ref().map(|d| format!("{:?}", d)),
        last_generated_reaction: state
            .last_generated_reaction
            .as_ref()
            .map(|r| r.text.clone()),
        mood: MoodSnapshot {
            current: format!("{:?}", state.mood.current),
            intensity: state.mood.intensity,
        },
        last_screen_captures: state
            .last_screen_captures
            .iter()
            .map(|capture| crate::snapshot::ScreenCaptureSnapshot {
                path: capture.path.clone(),
                screen_index: capture.screen_index,
                width: capture.width,
                height: capture.height,
            })
            .collect(),
        last_chat_reply: state.last_chat_reply.clone(),
        chat_history_len: state.chat_history.len(),
        last_ocr_text: state.last_ocr_text.clone(),
        visible_text_context: state.visible_text_context.clone(),
        last_action_plan: state.last_action_plan.clone(),
        pending_action: state.pending_action.clone(),
        last_assessment: state.last_assessment.clone(),
        short_term_memory: state.short_term_memory.clone(),
        short_term_memory_summary: state.short_term_memory_summary.clone(),
        attention: state.attention.clone(),
        long_term_memory: state.long_term_memory.top_entries(12),
        last_curiosity_question: state.last_curiosity_question.clone(),
        companion_state: state.companion_state.clone(),
    }
}

fn init_tracing() {
    let filter = EnvFilter::builder()
        .with_default_directive(Level::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

fn should_request_interpretation_for_current_window(
    config: &AppConfig,
    current: &ActiveWindowState,
    stable_for_ms: u128,
    now: Instant,
) -> bool {
    if !should_request_interpretation(config, &current.activity, stable_for_ms) {
        return false;
    }

    match current.last_interpretation_requested_at {
        None => true,
        Some(last_time) => {
            now.duration_since(last_time).as_millis() >= config.reinterpret_same_window_cooldown_ms
        }
    }
}

fn find_focused_screen_capture<'a>(
    captures: &'a [crate::events::ScreenCaptureEvent],
    state: &AppState,
) -> Option<&'a crate::events::ScreenCaptureEvent> {
    let window = state.active_window.as_ref()?;

    let center_x = (window.window_left + window.window_right) / 2;
    let center_y = (window.window_top + window.window_bottom) / 2;

    captures.iter().find(|capture| {
        let left = capture.x;
        let top = capture.y;
        let right = capture.x + capture.width as i32;
        let bottom = capture.y + capture.height as i32;

        center_x >= left && center_x < right && center_y >= top && center_y < bottom
    })
}

fn handle_vision_interpreted(
    state: &mut AppState,
    event_bus: &mut EventBus,
    vision: VisionInterpretation,
) {
    info!(
        activity = ?vision.detected_activity,
        confidence = vision.confidence,
        description = %vision.description,
        "vision interpreted"
    );

    let Some(window) = state.active_window.as_ref() else {
        return;
    };

    let stable_for_ms = window
        .last_seen_at
        .duration_since(window.first_seen_at)
        .as_millis();

    let mut fused_context = fuse_context(
        &window.process_name,
        &window.title,
        state.last_interpretation.as_ref(),
        Some(&vision),
        &window.activity,
        state.last_ocr_text.as_deref(),
    );

    if let Some(visible) = state.visible_text_context.as_ref() {
        fused_context
            .summary
            .push_str("\nVisible screen text analysis:");

        if !visible.detected_files.is_empty() {
            fused_context.summary.push_str("\nDetected files: ");
            fused_context
                .summary
                .push_str(&visible.detected_files.join(", "));
        }

        if !visible.detected_errors.is_empty() {
            fused_context
                .summary
                .push_str("\nDetected errors/warnings:\n");
            fused_context
                .summary
                .push_str(&visible.detected_errors.join("\n"));
        }

        if !visible.detected_keywords.is_empty() {
            fused_context.summary.push_str("\nDetected keywords: ");
            fused_context
                .summary
                .push_str(&visible.detected_keywords.join(", "));
        }

        if !visible.raw_preview.is_empty() {
            fused_context.summary.push_str("\nOCR preview:\n");
            fused_context.summary.push_str(&visible.raw_preview);
        }
    }

    event_bus.push(AppEvent::ContextFused {
        fused_context,
        stable_for_ms,
    });
}

fn handle_fused_context(
    state: &mut AppState,
    event_bus: &mut EventBus,
    config: &AppConfig,
    fused_context: crate::context_fusion::FusedContext,
    stable_for_ms: u128,
) {
    info!(
        activity = ?fused_context.activity,
        confidence = fused_context.confidence,
        source = ?fused_context.source,
        summary = %fused_context.summary,
        "context fused"
    );

    state.last_fused_context = Some(fused_context.clone());

    if config.auto_assessment_enabled {
        event_bus.push(AppEvent::ContextAssessmentRequested);
    }

    let interpretation = ContextInterpretation {
        activity: fused_context.activity,
        confidence: fused_context.confidence,
        summary: fused_context.summary,
        should_comment: false,
    };

    state.last_interpretation = Some(interpretation.clone());

    let decision = decide_reaction(
        state,
        config,
        &interpretation,
        stable_for_ms,
        Instant::now(),
    );

    event_bus.push(AppEvent::ReactionDecisionMade(decision));
}

async fn handle_chat_request(
    request: ChatRequest,
    state: &mut AppState,
    config: &AppConfig,
    llm: &LlmClient,
    shared_snapshot: &SharedSnapshot,
) {
    let user_message = request.message;

    state.chat_history.push(ChatMessage {
        role: ChatRole::User,
        content: user_message.clone(),
    });

    state.companion_state.observe_user_message(&user_message);

    let user_intent = classify_user_intent(&user_message);

    info!(intent = ?user_intent, message = %user_message, "chat intent classified");

    if matches!(user_intent, crate::intent::UserIntent::RequestPcAction) {
        let result = llm
            .generate_action_plan(
                &user_message,
                state.last_fused_context.as_ref(),
                &config.persona,
                &state.mood,
            )
            .await;

        let text = match result {
            Ok(plan) => {
                state.last_action_plan = Some(plan.clone());

                state.push_memory(crate::memory::MemoryEntry {
                    category: crate::memory::MemoryCategory::Goal,
                    summary: format!(
                        "User requested action: {} | Proposed: {:?} {}",
                        plan.user_request, plan.proposed_action.kind, plan.proposed_action.target
                    ),
                    importance: 0.75,
                    timestamp_ms: current_timestamp_ms(),
                });

                state.pending_action =
                    crate::actions::proposed_action_to_executable(&plan.proposed_action);

                if config.auto_memory_summary_enabled {
                    maybe_refresh_memory_summary(state, llm).await;
                }

                let steps = plan
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(idx, step)| format!("{}. {}", idx + 1, step))
                    .collect::<Vec<_>>()
                    .join("\n");

                format!(
                    "{}\n\nJe ne peux pas encore l’exécuter moi-même, mais je te proposerais :\n{}",
                    plan.summary, steps
                )
            }
            Err(err) => {
                error!(error = %err, "failed to generate action plan");

                "Je ne peux pas encore agir directement sur ton PC. Pour l’instant, je peux seulement observer, commenter et te proposer ce que je ferais.".to_string()
            }
        };

        state.chat_history.push(ChatMessage {
            role: ChatRole::Assistant,
            content: text.clone(),
        });

        state.last_chat_reply = Some(text.clone());

        let _ = request.reply_tx.send(Ok(text));

        sync_snapshot(shared_snapshot, state, config).await;
        return;
    }

    let chat_context = crate::chat_context::ChatGenerationContext {
        user_intent: &user_intent,
        fused_context: state.last_fused_context.as_ref(),
        visible_text: state.visible_text_context.as_ref(),
        assessment: state.last_assessment.as_ref(),
        persona: &config.persona,
        mood: &state.mood,
        short_term_memory: &state.short_term_memory,
        short_term_memory_summary: state.short_term_memory_summary.as_ref(),
        attention: &state.attention,
        long_term_memory: &state.long_term_memory,
        companion_state: &state.companion_state,
        recent_chat_history: &state.chat_history,
    };

    let result = llm.generate_chat_reply(&user_message, &chat_context).await;

    match result {
        Ok(reply) => {
            let text = reply.text;

            state.chat_history.push(ChatMessage {
                role: ChatRole::Assistant,
                content: text.clone(),
            });

            state.last_chat_reply = Some(text.clone());

            let _ = request.reply_tx.send(Ok(text));
        }
        Err(err) => {
            let _ = request.reply_tx.send(Err(err));
        }
    }

    sync_snapshot(shared_snapshot, state, config).await;
}

async fn handle_comment_now_request(
    request: CommentNowRequest,
    state: &mut AppState,
    config: &AppConfig,
    llm: &LlmClient,
    shared_snapshot: &SharedSnapshot,
) {
    let Some(context) = state.last_fused_context.as_ref() else {
        let _ = request.reply_tx.send(Ok(
            "Je n'ai pas encore assez de contexte visuel fiable pour commenter.".to_string(),
        ));
        return;
    };

    let interpretation = ContextInterpretation {
        activity: context.activity.clone(),
        confidence: context.confidence,
        summary: context.summary.clone(),
        should_comment: true,
    };

    let decision = ReactionDecision::CuriousComment {
        reason: "manual comment requested by user".to_string(),
    };

    let result = llm
        .generate_reaction(
            &interpretation,
            &decision,
            &state.recent_reaction_memory.recent_texts(),
            &config.persona,
            &state.mood,
        )
        .await;

    match result {
        Ok(generated) => {
            let text = generated.text.clone();

            state.last_generated_reaction = Some(generated.clone());
            state.recent_reaction_memory.push(generated);

            let _ = request.reply_tx.send(Ok(text));
        }
        Err(err) => {
            let _ = request.reply_tx.send(Err(err));
        }
    }

    sync_snapshot(shared_snapshot, state, config).await;
}

async fn handle_refresh_analysis_request(
    request: RefreshAnalysisRequest,
    event_bus: &mut EventBus,
) {
    event_bus.push(AppEvent::ForceScreenAnalysis);

    let _ = request
        .reply_tx
        .send(Ok("Analyse forcée déclenchée.".to_string()));
}

fn current_timestamp_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

async fn maybe_refresh_memory_summary(state: &mut AppState, llm: &LlmClient) {
    if state.short_term_memory.len() < 4 {
        return;
    }

    if !state.short_term_memory.len().is_multiple_of(4) {
        return;
    }

    match llm
        .summarize_short_term_memory(&state.short_term_memory)
        .await
    {
        Ok(summary) => {
            info!(summary = %summary, "short-term memory summarized");
            state.short_term_memory_summary = Some(summary);
        }
        Err(err) => {
            warn!(error = %err, "failed to summarize short-term memory");
        }
    }
}

async fn maybe_learn_from_attention(state: &mut AppState, llm: &LlmClient) {
    let now = current_timestamp_ms();

    for target in state.attention.strong_targets() {
        match llm
            .extract_memory_candidate(&target.subject, target.seen_count, target.interest_score)
            .await
        {
            Ok(candidate) => {
                if !candidate.should_store {
                    continue;
                }

                state.long_term_memory.add_or_update(
                    crate::long_term_memory::LongTermMemoryEntry {
                        id: format!("memory-{now}-{}", candidate.content.len()),
                        category: candidate.category,
                        content: candidate.content,
                        confidence: candidate.confidence,
                        importance: candidate.importance,
                        created_at_ms: now,
                        updated_at_ms: now,
                    },
                );
            }
            Err(err) => {
                warn!(
                    subject = %target.subject,
                    error = %err,
                    "failed to extract memory candidate"
                );
            }
        }
    }

    if let Err(err) = state.long_term_memory.save("data/long_term_memory.json") {
        warn!(error = %err, "failed to save long-term memory");
    }
}

async fn maybe_generate_curiosity(state: &mut AppState, config: &AppConfig, llm: &LlmClient) {
    if state.last_curiosity_question.is_some() {
        return;
    }

    if let Err(err) = generate_curiosity_question_now(state, config, llm).await {
        warn!(error = %err, "failed to generate curiosity question");
    }
}

async fn generate_curiosity_question_now(
    state: &mut AppState,
    config: &AppConfig,
    llm: &LlmClient,
) -> anyhow::Result<String> {
    let subject = state
        .attention
        .most_interesting_subject()
        .map(|target| (target.subject.clone(), target.interest_score))
        .or_else(|| {
            state
                .last_assessment
                .as_ref()
                .map(|assessment| (assessment.likely_user_goal.clone(), assessment.confidence))
        })
        .or_else(|| {
            state
                .active_window
                .as_ref()
                .map(|window| (format!("{} / {}", window.process_name, window.title), 0.45))
        })
        .unwrap_or_else(|| ("the current computer activity".to_string(), 0.35));

    let question = llm
        .generate_curiosity_question(&subject.0, &config.persona, &state.mood)
        .await?;

    info!(
        subject = %subject.0,
        question = %question,
        "curiosity question generated"
    );

    state.last_curiosity_question = Some(question.clone());
    state.companion_state.observe_curiosity_question(&subject.0);

    state.push_memory(crate::memory::MemoryEntry {
        category: crate::memory::MemoryCategory::Goal,
        summary: format!("Nemi became curious about: {}", subject.0),
        importance: subject.1,
        timestamp_ms: current_timestamp_ms(),
    });

    Ok(question)
}

async fn handle_curiosity_now_request(
    request: CuriosityNowRequest,
    state: &mut AppState,
    config: &AppConfig,
    llm: &LlmClient,
    shared_snapshot: &SharedSnapshot,
) {
    match generate_curiosity_question_now(state, config, llm).await {
        Ok(question) => {
            let _ = request.reply_tx.send(Ok(question));
        }
        Err(err) => {
            let _ = request.reply_tx.send(Err(err));
        }
    }

    sync_snapshot(shared_snapshot, state, config).await;
}

async fn handle_clear_curiosity_request(
    request: ClearCuriosityRequest,
    state: &mut AppState,
    shared_snapshot: &SharedSnapshot,
    config: &AppConfig,
) {
    state.last_curiosity_question = None;

    let _ = request.reply_tx.send(Ok("Curiosité effacée.".to_string()));

    sync_snapshot(shared_snapshot, state, config).await;
}
