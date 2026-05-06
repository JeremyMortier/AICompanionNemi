use crate::activity::UserActivity;
use crate::context::ContextInterpretation;
use crate::vision::VisionInterpretation;

#[derive(Debug, Clone)]
pub struct FusedContext {
    pub activity: UserActivity,
    pub confidence: f32,
    pub summary: String,
    pub source: ContextSource,
}

#[derive(Debug, Clone)]
pub enum ContextSource {
    TextOnly,
    VisionOnly,
    TextAndVision,
    MetadataProtected,
}

pub fn fuse_context(
    process_name: &str,
    window_title: &str,
    text_context: Option<&ContextInterpretation>,
    vision_context: Option<&VisionInterpretation>,
    heuristic_activity: &UserActivity,
    _ocr_text: Option<&str>,
) -> FusedContext {
    let process = process_name.to_lowercase();

    if is_strong_metadata_app(&process) {
        return fuse_with_metadata_protection(
            process_name,
            window_title,
            text_context,
            vision_context,
            heuristic_activity,
        );
    }

    match (text_context, vision_context) {
        (Some(text), Some(vision)) => fuse_text_and_vision(text, vision, heuristic_activity),
        (Some(text), None) => FusedContext {
            activity: text.activity.clone(),
            confidence: text.confidence,
            summary: text.summary.clone(),
            source: ContextSource::TextOnly,
        },
        (None, Some(vision)) => FusedContext {
            activity: vision.detected_activity.clone(),
            confidence: vision.confidence,
            summary: vision.description.clone(),
            source: ContextSource::VisionOnly,
        },
        (None, None) => FusedContext {
            activity: heuristic_activity.clone(),
            confidence: 0.45,
            summary: format!(
                "User is likely using {} with window title: {}",
                process_name, window_title
            ),
            source: ContextSource::TextOnly,
        },
    }
}

fn fuse_text_and_vision(
    text: &ContextInterpretation,
    vision: &VisionInterpretation,
    heuristic_activity: &UserActivity,
) -> FusedContext {
    if text.activity == vision.detected_activity {
        return FusedContext {
            activity: text.activity.clone(),
            confidence: ((text.confidence + vision.confidence) / 2.0 + 0.1).clamp(0.0, 1.0),
            summary: format!(
                "{} Visual confirmation: {}",
                text.summary, vision.description
            ),
            source: ContextSource::TextAndVision,
        };
    }

    if vision.confidence >= 0.85 && text.confidence < 0.65 {
        return FusedContext {
            activity: vision.detected_activity.clone(),
            confidence: vision.confidence,
            summary: vision.description.clone(),
            source: ContextSource::VisionOnly,
        };
    }

    if text.confidence >= 0.75 {
        return FusedContext {
            activity: text.activity.clone(),
            confidence: text.confidence,
            summary: format!("{} Visual note: {}", text.summary, vision.description),
            source: ContextSource::TextAndVision,
        };
    }

    FusedContext {
        activity: heuristic_activity.clone(),
        confidence: 0.55,
        summary: format!(
            "Context is ambiguous. Text says {:?}; vision says {:?}. Visual note: {}",
            text.activity, vision.detected_activity, vision.description
        ),
        source: ContextSource::TextAndVision,
    }
}

fn fuse_with_metadata_protection(
    process_name: &str,
    window_title: &str,
    text_context: Option<&ContextInterpretation>,
    vision_context: Option<&VisionInterpretation>,
    heuristic_activity: &UserActivity,
) -> FusedContext {
    let mut summary = format!(
        "Reliable metadata indicates {:?}. App: {}. Title: {}.",
        heuristic_activity, process_name, window_title
    );

    if let Some(text) = text_context {
        summary.push_str(&format!(" Text interpretation: {}", text.summary));
    }

    if let Some(vision) = vision_context {
        summary.push_str(&format!(" Visual note: {}", vision.description));
    }

    FusedContext {
        activity: heuristic_activity.clone(),
        confidence: 0.85,
        summary,
        source: ContextSource::MetadataProtected,
    }
}

fn is_strong_metadata_app(process: &str) -> bool {
    process.contains("discord")
        || process.contains("code")
        || process.contains("steam")
        || process.contains("slack")
        || process.contains("teams")
}
