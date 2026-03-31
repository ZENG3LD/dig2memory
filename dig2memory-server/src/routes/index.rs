use axum::{extract::State, routing::post, Json, Router};
use dig2memory_core::index::indexer::Indexer;
use dig2memory_core::{IndexRequest, IndexResult};

use crate::state::{AppError, SharedState};

pub fn router() -> Router<SharedState> {
    Router::new().route("/index", post(post_index))
}

async fn post_index(
    State(state): State<SharedState>,
    Json(req): Json<IndexRequest>,
) -> Result<Json<IndexResult>, AppError> {
    // Clone the Arc so the closure owns it.
    let db = state.db.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut indexer = Indexer::new()?;
        indexer.run(&db, &req)
    })
    .await
    .map_err(|e| AppError::Internal(format!("task join error: {}", e)))??;

    Ok(Json(result))
}
