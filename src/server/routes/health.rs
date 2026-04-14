use axum::{extract::State, routing::get, Json, Router};
use crate::HealthStatus;
use crate::db::reader;
use crate::server::state::{SharedState, AppError};

pub fn router() -> Router<SharedState> {
    Router::new().route("/health", get(get_health))
}

async fn get_health(State(state): State<SharedState>) -> Result<Json<HealthStatus>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::Internal("db lock poisoned".into()))?;
    let workspaces = reader::list_workspaces(&db);
    let symbol_count = reader::count_symbols(&db, None);
    let file_count = reader::count_files(&db, None);
    let ok = workspaces.is_ok() && symbol_count.is_ok() && file_count.is_ok();
    let db_path = format!("{}/index.db", state.data_dir);
    let status = HealthStatus {
        ok,
        db_path,
        workspace_count: workspaces.map(|ws| ws.len() as u32).unwrap_or(0),
        symbol_count: symbol_count.unwrap_or(0),
        file_count: file_count.unwrap_or(0),
    };
    Ok(Json(status))
}
