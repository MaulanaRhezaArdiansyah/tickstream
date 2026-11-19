use std::sync::Arc;

use axum::{Json, extract::State, http::header, response::IntoResponse};
use serde::Deserialize;

use crate::{
    error::AppError,
    extract::AppQuery,
    market::{MarketStatus, Profile, Quote, SymbolInfo, normalize_symbol},
    state::AppState,
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
    let symbol = parse_symbol(&params.symbol)?;
    Ok(Json(state.market.quote(&symbol).await?))
}

/// `GET /api/profile?symbol=NVDA`
pub async fn profile(
    State(state): State<AppState>,
    AppQuery(params): AppQuery<SymbolQuery>,
) -> Result<Json<Profile>, AppError> {
    let symbol = parse_symbol(&params.symbol)?;
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

fn parse_symbol(raw: &str) -> Result<String, AppError> {
    normalize_symbol(raw)
        .ok_or_else(|| AppError::BadRequest(format!("simbol tidak valid: {raw:?}")))
}
