mod market;

use axum::{Router, routing::get};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/quote", get(market::quote))
        .route("/api/profile", get(market::profile))
        .route("/api/market-status", get(market::market_status))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}
