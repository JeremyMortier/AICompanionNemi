mod activity;
mod app;
mod config;
mod context;
mod events;
mod llm;
mod observation;
mod state;
mod tick;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}
