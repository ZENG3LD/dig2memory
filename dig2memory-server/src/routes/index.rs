use axum::{extract::State, routing::post, Json, Router};
use dig2memory_core::{IndexRequest, IndexResult};
use dig2memory_core::index::indexer::Indexer;
use crate::state::{SharedState, AppError};

pub fn router() -> Router<SharedState> {
    Router::new().route("/index", post(post_index))
}

async fn post_index(
    State(state): State<SharedState>,
    Json(req): Json<IndexRequest>,
) -> Result<Json<IndexResult>, AppError> {
    let state2 = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let db = state2.db.lock().unwrap();
        let mut indexer = Indexer::new(&db)?;
        indexer.run(&req)
    })
    .await
    .map_err(|e| AppError::Internal(format!("task join error: {}", e)))??;

    Ok(Json(result))
}
