mod market;
mod watchlist;
mod webhook;
mod ws;

#[cfg(test)]
mod tests;

use std::path::Path;

use axum::{
    Router, middleware,
    routing::{any, delete, get, post},
};
use tower_http::{
    compression::CompressionLayer,
    services::{ServeDir, ServeFile},
};

use crate::{error::AppError, state::AppState};

/// `web_dir`: folder hasil build React yang ikut disajikan (production). `None` saat dev,
/// karena halaman disajikan Vite yang meneruskan /api dan /ws ke sini.
pub fn router(state: AppState, web_dir: Option<&Path>) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(crate::metrics::render))
        .route("/api/quote", get(market::quote))
        .route("/api/news", get(market::news))
        .route("/api/candles", get(market::candles))
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
        .route("/webhook/finnhub", post(webhook::finnhub));

    let app = match web_dir {
        // Path selain API dicari di folder build; yang nggak ada dapat index.html.
        // Path /api yang salah ketik tetap dibalas JSON 404, bukan halaman HTML.
        Some(dir) => api
            .route("/api/{*rest}", any(not_found))
            .fallback_service(ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")))),
        // Route yang nggak ada juga dibalas JSON, bukan body kosong.
        None => api.fallback(not_found),
    };
    // Dipasang paling akhir supaya fallback (file statis, 404) ikut terukur.
    app.layer(middleware::from_fn(crate::metrics::track_http))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn not_found() -> AppError {
    AppError::NotFound("endpoint tidak ada".into())
}
