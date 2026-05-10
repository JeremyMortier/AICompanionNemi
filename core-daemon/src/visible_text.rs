use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisibleTextContext {
    pub raw_preview: String,
    pub detected_files: Vec<String>,
    pub detected_errors: Vec<String>,
    pub detected_keywords: Vec<String>,
}

pub fn analyze_visible_text(text: &str) -> VisibleTextContext {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let detected_files = lines
        .iter()
        .filter_map(|line| extract_file_like_token(line))
        .collect::<Vec<_>>();

    let detected_errors = lines
        .iter()
        .filter(|line| looks_like_error(line))
        .map(|line| line.to_string())
        .take(10)
        .collect::<Vec<_>>();

    let detected_keywords = detect_keywords(&lines);

    VisibleTextContext {
        raw_preview: lines
            .iter()
            .take(30)
            .copied()
            .collect::<Vec<_>>()
            .join("\n"),
        detected_files,
        detected_errors,
        detected_keywords,
    }
}

fn extract_file_like_token(line: &str) -> Option<String> {
    let known_exts = [
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".json", ".toml", ".yaml", ".yml", ".html", ".css",
        ".php", ".py", ".md",
    ];

    line.split_whitespace()
        .map(|token| {
            token.trim_matches(|c: char| {
                matches!(c, ',' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']')
            })
        })
        .find(|token| known_exts.iter().any(|ext| token.ends_with(ext)))
        .map(str::to_string)
}

fn looks_like_error(line: &str) -> bool {
    let lower = line.to_lowercase();

    lower.contains("error")
        || lower.contains("warning")
        || lower.contains("failed")
        || lower.contains("panic")
        || lower.contains("exception")
        || lower.contains("mismatched")
        || lower.contains("cannot")
        || lower.contains("borrow")
        || lower.contains("undefined")
}

fn detect_keywords(lines: &[&str]) -> Vec<String> {
    let joined = lines.join(" ").to_lowercase();

    let candidates = [
        "rust",
        "cargo",
        "clippy",
        "tesseract",
        "ocr",
        "visual studio code",
        "discord",
        "steam",
        "github",
        "json",
        "typescript",
        "javascript",
        "python",
    ];

    candidates
        .iter()
        .filter(|keyword| joined.contains(**keyword))
        .map(|keyword| keyword.to_string())
        .collect()
}
