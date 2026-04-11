mod activity;
mod app;
mod config;
mod context;
mod decision;
mod events;
mod llm;
mod observation;
mod reaction;
mod state;
mod tick;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}
