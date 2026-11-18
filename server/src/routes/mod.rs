mod market;

use axum::{Router, routing::get};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/quote", get(market::quote))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}
