use axum::Router;
use crate::server::state::SharedState;

mod ast;
mod graph;
mod index;
mod health;

pub fn api_router(state: SharedState) -> Router {
    Router::new()
        .merge(health::router())
        .merge(index::router())
        .merge(ast::router())
        .merge(graph::router())
        .with_state(state)
}
