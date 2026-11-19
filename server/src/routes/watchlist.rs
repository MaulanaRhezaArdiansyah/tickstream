use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;

use crate::{
    auth::Admin,
    error::AppError,
    extract::{AppJson, AppPath},
    market::normalize_symbol,
    state::AppState,
    watchlist::{self, MAX_ENTRIES},
};

/// `GET /api/watchlist` → `["AAPL", "NVDA", ...]` sesuai urutan ditambahkan.
pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<String>>, AppError> {
    Ok(Json(watchlist::list(&state.db).await?))
}

#[derive(Deserialize)]
pub struct AddBody {
    symbol: String,
}

/// `POST /api/watchlist` `{"symbol":"nvda"}` → `201 "NVDA"`.
pub async fn add(
    _: Admin,
    State(state): State<AppState>,
    AppJson(body): AppJson<AddBody>,
) -> Result<(StatusCode, Json<String>), AppError> {
    let symbol = normalize_symbol(&body.symbol)
        .ok_or_else(|| AppError::BadRequest(format!("simbol tidak valid: {:?}", body.symbol)))?;

    // Cek yang murah (database) dulu, baru yang makan jatah API (quote).
    if watchlist::contains(&state.db, &symbol).await? {
        return Err(AppError::Conflict(format!(
            "{symbol} sudah ada di watchlist"
        )));
    }
    if watchlist::count(&state.db).await? >= MAX_ENTRIES {
        return Err(AppError::Conflict(format!(
            "watchlist penuh (maks {MAX_ENTRIES} simbol)"
        )));
    }
    // Pastikan ticker-nya dikenal sumber data. Ticker ngawur → 404 dari MarketError.
    // Hasilnya masuk cache, jadi kartu baru di browser langsung dapat quote tanpa call lagi.
    state.market.quote(&symbol).await?;

    // Dua request bersamaan untuk simbol yang sama bisa sama-sama lolos cek `contains`;
    // constraint UNIQUE yang jadi penentu akhirnya.
    if !watchlist::add(&state.db, &symbol).await? {
        return Err(AppError::Conflict(format!(
            "{symbol} sudah ada di watchlist"
        )));
    }
    tracing::info!(%symbol, "watchlist: tambah");
    Ok((StatusCode::CREATED, Json(symbol)))
}

/// `DELETE /api/watchlist/NVDA` → `204`.
pub async fn remove(
    _: Admin,
    State(state): State<AppState>,
    AppPath(raw): AppPath<String>,
) -> Result<StatusCode, AppError> {
    let symbol = normalize_symbol(&raw)
        .ok_or_else(|| AppError::BadRequest(format!("simbol tidak valid: {raw:?}")))?;
    if !watchlist::remove(&state.db, &symbol).await? {
        return Err(AppError::NotFound(format!(
            "{symbol} nggak ada di watchlist"
        )));
    }
    tracing::info!(%symbol, "watchlist: hapus");
    Ok(StatusCode::NO_CONTENT)
}
