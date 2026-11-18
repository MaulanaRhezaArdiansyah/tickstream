use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::{
    error::AppError,
    market::{MarketStatus, Profile, Quote, normalize_symbol},
    state::AppState,
};

#[derive(Deserialize)]
pub struct SymbolQuery {
    symbol: String,
}

/// `GET /api/quote?symbol=NVDA`
pub async fn quote(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Result<Json<Quote>, AppError> {
    let symbol = parse_symbol(&params.symbol)?;
    Ok(Json(state.market.quote(&symbol).await?))
}

/// `GET /api/profile?symbol=NVDA`
pub async fn profile(
    State(state): State<AppState>,
    Query(params): Query<SymbolQuery>,
) -> Result<Json<Profile>, AppError> {
    let symbol = parse_symbol(&params.symbol)?;
    Ok(Json(state.market.profile(&symbol).await?))
}

/// `GET /api/market-status` (bursa US)
pub async fn market_status(State(state): State<AppState>) -> Result<Json<MarketStatus>, AppError> {
    Ok(Json(state.market.market_status().await?))
}

fn parse_symbol(raw: &str) -> Result<String, AppError> {
    normalize_symbol(raw)
        .ok_or_else(|| AppError::BadRequest(format!("simbol tidak valid: {raw:?}")))
}
