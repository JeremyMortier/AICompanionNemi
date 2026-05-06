use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

pub fn extract_text_from_image(
    tesseract_path: &str,
    image_path: impl AsRef<Path>,
) -> Result<String> {
    let image_path = image_path.as_ref();

    let output = Command::new(tesseract_path)
        .arg(image_path)
        .arg("stdout")
        .arg("-l")
        .arg("eng+fra+jpn")
        .arg("--psm")
        .arg("11")
        .output()
        .with_context(|| format!("failed to run tesseract on {}", image_path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tesseract failed: {}", stderr);
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();

    Ok(clean_ocr_text(&text))
}

fn clean_ocr_text(input: &str) -> String {
    input
        .lines()
        .map(str::trim)
        .filter(|line| is_useful_ocr_line(line))
        .take(80)
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_useful_ocr_line(line: &str) -> bool {
    if line.len() < 2 {
        return false;
    }

    let alpha_numeric_count = line.chars().filter(|c| c.is_alphanumeric()).count();

    if alpha_numeric_count < 2 {
        return false;
    }

    let noise_chars = line
        .chars()
        .filter(|c| {
            !c.is_alphanumeric()
                && !c.is_whitespace()
                && !matches!( c, '.' | ',' | ':' | ';' | '-' | '_' | '/' | '\\' | '(' | ')' | '[' | ']' | '{' | '}' | '#' | '+' | '=' | '"' | '\'' | '<' | '>' | '?' | '!' )
        })
        .count();

    let total_chars = line.chars().count();

    if total_chars == 0 {
        return false;
    }

    let noise_ratio = noise_chars as f32 / total_chars as f32;

    noise_ratio < 0.35
}
