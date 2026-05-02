use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use screenshots::Screen;

const MAX_SCREENSHOTS_TO_KEEP: usize = 12;

#[derive(Debug, Clone)]
pub struct ScreenCapture {
    pub path: PathBuf,
    pub screen_index: usize,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

pub fn capture_all_screens(output_dir: impl AsRef<Path>) -> Result<Vec<ScreenCapture>> {
    let output_dir = output_dir.as_ref();

    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create screenshot dir: {}", output_dir.display()))?;

    cleanup_old_screenshots(output_dir)?;

    let screens = Screen::all().context("failed to enumerate screens")?;

    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?
        .as_millis();

    let mut captures = Vec::new();

    for (index, screen) in screens.iter().enumerate() {
        let image = screen
            .capture()
            .with_context(|| format!("failed to capture screen #{index}"))?;

        let path = output_dir.join(format!("screen-{timestamp_ms}-{index}.png"));

        image
            .save(&path)
            .with_context(|| format!("failed to save screenshot: {}", path.display()))?;

        captures.push(ScreenCapture {
            path,
            screen_index: index,
            width: image.width(),
            height: image.height(),
            x: screen.display_info.x,
            y: screen.display_info.y,
        });
    }

    Ok(captures)
}

fn cleanup_old_screenshots(output_dir: &Path) -> Result<()> {
    if !output_dir.exists() {
        return Ok(());
    }

    let mut entries = fs::read_dir(output_dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();

            let is_png = path.extension().and_then(|ext| ext.to_str()) == Some("png");

            if !is_png {
                return None;
            }

            let modified = entry.metadata().ok()?.modified().ok()?;

            Some((path, modified))
        })
        .collect::<Vec<_>>();

    entries.sort_by_key(|(_, modified)| *modified);

    let excess = entries.len().saturating_sub(MAX_SCREENSHOTS_TO_KEEP);

    for (path, _) in entries.into_iter().take(excess) {
        let _ = fs::remove_file(path);
    }

    Ok(())
}
