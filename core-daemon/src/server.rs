use std::{net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc};

use crate::snapshot::AppSnapshot;

pub type SharedSnapshot = Arc<RwLock<AppSnapshot>>;

#[derive(Clone)]
pub struct ServerState {
    pub snapshot: SharedSnapshot,
    pub chat_tx: mpsc::Sender<ChatRequest>,
}

#[derive(Debug)]
pub struct ChatRequest {
    pub message: String,
    pub reply_tx: tokio::sync::oneshot::Sender<anyhow::Result<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequestBody {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponseBody {
    pub reply: String,
}

pub async fn run_server(
    shared_snapshot: SharedSnapshot,
    chat_tx: mpsc::Sender<ChatRequest>,
) -> anyhow::Result<()> {
    let state = ServerState {
        snapshot: shared_snapshot,
        chat_tx,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/state", get(get_state))
        .route("/chat", post(chat))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 7878));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn get_state(State(state): State<ServerState>) -> Json<AppSnapshot> {
    let snapshot = state.snapshot.read().await.clone();
    Json(snapshot)
}

async fn chat(
    State(state): State<ServerState>,
    Json(body): Json<ChatRequestBody>,
) -> Json<ChatResponseBody> {
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

    let request = ChatRequest {
        message: body.message,
        reply_tx,
    };

    if state.chat_tx.send(request).await.is_err() {
        return Json(ChatResponseBody {
            reply: "Nemi n'est pas disponible pour répondre pour l'instant.".to_string(),
        });
    }

    match reply_rx.await {
        Ok(Ok(reply)) => Json(ChatResponseBody { reply }),
        _ => Json(ChatResponseBody {
            reply: "Je n'ai pas réussi à formuler une réponse cette fois.".to_string(),
        }),
    }
}
