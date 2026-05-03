mod activity;
mod app;
mod chat;
mod config;
mod context;
mod context_fusion;
mod decision;
mod events;
mod llm;
mod memory;
mod mood;
mod observation;
mod persona;
mod reaction;
mod screen;
mod server;
mod snapshot;
mod state;
mod tick;
mod vision;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}
