mod market;
mod watchlist;
mod ws;

use axum::{
    Router,
    routing::{delete, get},
};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/quote", get(market::quote))
        .route("/api/profile", get(market::profile))
        .route("/api/market-status", get(market::market_status))
        .route("/api/watchlist", get(watchlist::list).post(watchlist::add))
        .route("/api/watchlist/{symbol}", delete(watchlist::remove))
        .route("/ws", get(ws::ws))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}
