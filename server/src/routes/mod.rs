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
use tower_http::compression::CompressionLayer;

use crate::{error::AppError, state::AppState};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/quote", get(market::quote))
        .route("/api/profile", get(market::profile))
        .route("/api/market-status", get(market::market_status))
        // ±11 ribu baris (±800 KB JSON); gzip memangkasnya jadi sekitar seperlimanya.
        // Layer dipasang di route ini saja: response lain kecil, nggak perlu dikompres.
        .route(
            "/api/symbols",
            get(market::symbols).layer(CompressionLayer::new()),
        )
        .route("/api/admin", get(crate::auth::check))
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
