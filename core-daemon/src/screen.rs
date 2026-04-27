use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use screenshots::Screen;

#[derive(Debug, Clone)]
pub struct ScreenCapture {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

pub fn capture_primary_screen(output_dir: impl AsRef<Path>) -> Result<ScreenCapture> {
    let output_dir = output_dir.as_ref();

    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create screenshot dir: {}", output_dir.display()))?;

    let screens = Screen::all().context("failed to enumerate screens")?;

    let screen = screens.first().context("no screen found")?;

    let image = screen.capture().context("failed to capture screen")?;

    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?
        .as_millis();

    let path = output_dir.join(format!("screen-{timestamp_ms}.png"));

    image
        .save(&path)
        .with_context(|| format!("failed to save screenshot: {}", path.display()))?;

    Ok(ScreenCapture {
        path,
        width: image.width(),
        height: image.height(),
    })
}
