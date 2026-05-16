use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserIntent {
    AskQuestion,
    ExplainScreen,
    AskNextStep,
    RequestPcAction,
    CommentCurrentContext,
    Unknown,
}

pub fn classify_user_intent(message: &str) -> UserIntent {
    let m = message.to_lowercase();

    if contains_any(
        &m,
        &[
            "explique ce que tu vois",
            "qu'est-ce que tu vois",
            "que vois-tu",
            "résume l'écran",
            "résume ce que tu vois",
            "describe screen",
            "what do you see",
        ],
    ) {
        return UserIntent::ExplainScreen;
    }

    if contains_any(
        &m,
        &[
            "que faire ensuite",
            "prochaine étape",
            "étape suivante",
            "je fais quoi maintenant",
            "what next",
            "next step",
        ],
    ) {
        return UserIntent::AskNextStep;
    }

    if contains_any(
        &m,
        &[
            "commente",
            "réagis",
            "donne ton avis",
            "comment this",
            "react to this",
        ],
    ) {
        return UserIntent::CommentCurrentContext;
    }

    if contains_any(
        &m,
        &[
            "ouvre ",
            "lance ",
            "ferme ",
            "clique",
            "écris ",
            "tape ",
            "supprime ",
            "déplace ",
            "copie ",
            "colle ",
            "open ",
            "launch ",
            "close ",
            "click",
            "type ",
            "delete ",
            "move ",
            "copy ",
            "paste ",
        ],
    ) {
        return UserIntent::RequestPcAction;
    }

    if m.trim().ends_with('?')
        || contains_any(
            &m,
            &[
                "comment", "pourquoi", "quoi", "où", "quand", "how", "why", "what",
            ],
        )
    {
        return UserIntent::AskQuestion;
    }

    UserIntent::Unknown
}

fn contains_any(input: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| input.contains(pattern))
}
