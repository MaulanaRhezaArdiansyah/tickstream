use std::sync::Arc;

use axum::{Json, extract::State, http::header, response::IntoResponse};
use serde::Deserialize;

use crate::{
    error::AppError,
    extract::AppQuery,
    market::{CompanyNews, MarketStatus, Profile, Quote, SymbolInfo, normalize_symbol},
    state::AppState,
    watchlist,
};

#[derive(Deserialize)]
pub struct SymbolQuery {
    symbol: String,
}

/// `GET /api/quote?symbol=NVDA`
pub async fn quote(
    State(state): State<AppState>,
    AppQuery(params): AppQuery<SymbolQuery>,
) -> Result<Json<Quote>, AppError> {
    let symbol = watched_symbol(&state, &params.symbol).await?;
    Ok(Json(state.market.quote(&symbol).await?))
}

/// `GET /api/news?symbol=AMD`: berita 7 hari terakhir dan jumlah berita 24 jam.
pub async fn news(
    State(state): State<AppState>,
    AppQuery(params): AppQuery<SymbolQuery>,
) -> Result<Json<CompanyNews>, AppError> {
    let symbol = watched_symbol(&state, &params.symbol).await?;
    Ok(Json(state.market.company_news(&symbol).await?))
}

/// `GET /api/profile?symbol=NVDA`
pub async fn profile(
    State(state): State<AppState>,
    AppQuery(params): AppQuery<SymbolQuery>,
) -> Result<Json<Profile>, AppError> {
    let symbol = watched_symbol(&state, &params.symbol).await?;
    Ok(Json(state.market.profile(&symbol).await?))
}

/// `GET /api/market-status` (bursa US)
pub async fn market_status(State(state): State<AppState>) -> Result<Json<MarketStatus>, AppError> {
    Ok(Json(state.market.market_status().await?))
}

/// `GET /api/symbols`: saham & ETF bursa utama US, untuk saran di form tambah.
/// Daftarnya paling cepat berubah harian, jadi browser boleh menyimpannya sejam.
pub async fn symbols(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let symbols: Arc<[SymbolInfo]> = state.market.us_symbols().await?;
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=3600")],
        Json(symbols),
    ))
}

/// Quote dan profil cuma dilayani untuk simbol di watchlist. Pengunjung bebas membuka
/// halaman, tapi nggak bisa memakai server ini untuk menghabiskan kuota Finnhub dengan
/// simbol sembarang. Browser memang cuma meminta simbol watchlist.
async fn watched_symbol(state: &AppState, raw: &str) -> Result<String, AppError> {
    let symbol = normalize_symbol(raw)
        .ok_or_else(|| AppError::BadRequest(format!("simbol tidak valid: {raw:?}")))?;
    if !watchlist::contains(&state.db, &symbol).await? {
        return Err(AppError::NotFound(format!(
            "{symbol} nggak ada di watchlist"
        )));
    }
    Ok(symbol)
}
