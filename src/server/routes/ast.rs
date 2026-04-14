use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use crate::db::reader;
use crate::search::trigram::fuzzy_search;
use crate::server::state::{SharedState, AppError};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/ast/search", get(search_symbols))
        .route("/ast/symbols", get(symbols_in_file))
        .route("/ast/callers", get(callers_of_symbol))
        .route("/ast/deps", get(file_deps))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    workspace: Option<String>,
    limit: Option<u32>,
}

#[derive(Deserialize)]
struct FileQuery {
    file: String,
    workspace: Option<String>,
}

#[derive(Deserialize)]
struct SymQuery {
    sym: String,
    workspace: Option<String>,
}

async fn search_symbols(
    State(state): State<SharedState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::Internal("db lock poisoned".into()))?;
    let limit = params.limit.unwrap_or(50);
    let results = fuzzy_search(&db, params.workspace.as_deref(), &params.q, limit, 0.1)?;
    Ok(Json(serde_json::json!(results)))
}

async fn symbols_in_file(
    State(state): State<SharedState>,
    Query(params): Query<FileQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::Internal("db lock poisoned".into()))?;
    let workspace = params.workspace.as_deref().unwrap_or("");
    let results = reader::get_symbols_in_file(&db, workspace, &params.file)?;
    Ok(Json(serde_json::json!(results)))
}

async fn callers_of_symbol(
    State(state): State<SharedState>,
    Query(params): Query<SymQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::Internal("db lock poisoned".into()))?;
    let results = reader::get_callers_of(&db, &params.sym, params.workspace.as_deref())?;
    Ok(Json(serde_json::json!(results)))
}

async fn file_deps(
    State(state): State<SharedState>,
    Query(params): Query<FileQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::Internal("db lock poisoned".into()))?;
    let workspace = params.workspace.as_deref().unwrap_or("");
    let results = reader::get_deps_of_file(&db, workspace, &params.file)?;
    Ok(Json(serde_json::json!(results)))
}
