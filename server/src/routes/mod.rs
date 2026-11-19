mod market;
mod watchlist;
mod webhook;
mod ws;

#[cfg(test)]
mod tests;

use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::{error::AppError, state::AppState};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/quote", get(market::quote))
        .route("/api/profile", get(market::profile))
        .route("/api/market-status", get(market::market_status))
        .route("/api/watchlist", get(watchlist::list).post(watchlist::add))
        .route("/api/watchlist/{symbol}", delete(watchlist::remove))
        .route("/ws", get(ws::ws))
        .route("/webhook/finnhub", post(webhook::finnhub))
        // Route yang nggak ada juga dibalas JSON, bukan body kosong.
        .fallback(not_found)
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn not_found() -> AppError {
    AppError::NotFound("endpoint tidak ada".into())
}
