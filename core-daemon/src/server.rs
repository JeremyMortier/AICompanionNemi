use std::{net::SocketAddr, sync::Arc};

use axum::{Json, Router, extract::State, routing::get};
use tokio::sync::RwLock;

use crate::snapshot::AppSnapshot;

pub type SharedSnapshot = Arc<RwLock<AppSnapshot>>;

pub async fn run_server(shared_snapshot: SharedSnapshot) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/state", get(get_state))
        .with_state(shared_snapshot);

    let addr = SocketAddr::from(([127, 0, 0, 1], 7878));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn get_state(State(shared_snapshot): State<SharedSnapshot>) -> Json<AppSnapshot> {
    let snapshot = shared_snapshot.read().await.clone();
    Json(snapshot)
}
