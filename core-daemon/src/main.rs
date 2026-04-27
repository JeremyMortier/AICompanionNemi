mod activity;
mod app;
mod config;
mod context;
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

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}
