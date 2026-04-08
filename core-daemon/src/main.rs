mod app;
mod config;
mod events;
mod observation;
mod state;
mod tick;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    app::run().await
}
