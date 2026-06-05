mod action_plan;
mod actions;
mod activity;
mod app;
mod assessment;
mod attention;
mod chat;
mod chat_context;
mod config;
mod context;
mod context_fusion;
mod decision;
mod events;
mod intent;
mod llm;
mod memory;
mod mood;
mod observation;
mod ocr;
mod persona;
mod reaction;
mod screen;
mod server;
mod snapshot;
mod state;
mod tick;
mod visible_text;
mod vision;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}
