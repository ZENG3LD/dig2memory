use axum::{extract::State, routing::get, Json, Router};
use dig2memory_core::HealthStatus;
use dig2memory_core::db::reader;
use crate::state::{SharedState, AppError};

pub fn router() -> Router<SharedState> {
    Router::new().route("/health", get(get_health))
}

async fn get_health(State(state): State<SharedState>) -> Result<Json<HealthStatus>, AppError> {
    let db = state.db.lock().unwrap();
    let workspace_count = reader::list_workspaces(&db).map(|ws| ws.len() as u32).unwrap_or(0);
    let symbol_count = reader::count_symbols(&db, None).unwrap_or(0);
    let file_count = reader::count_files(&db, None).unwrap_or(0);
    let db_path = format!("{}/index.db", state.data_dir);
    let status = HealthStatus {
        ok: true,
        db_path,
        workspace_count,
        symbol_count,
        file_count,
    };
    Ok(Json(status))
}
