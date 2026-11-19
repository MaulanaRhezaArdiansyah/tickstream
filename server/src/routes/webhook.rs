//! `POST /webhook/finnhub`: event dari Finnhub.
//!
//! Finnhub minta webhook dibalas 2xx secepatnya, SEBELUM logika apa pun; kalau sering
//! lambat/gagal, webhook-nya bisa dimatikan. Jadi handler cuma memeriksa (secret + JSON
//! valid), lalu menyerahkan penyimpanan ke task background dan langsung membalas 200.

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use secrecy::ExposeSecret;
use serde::de::IgnoredAny;
use subtle::ConstantTimeEq;

use crate::{error::AppError, state::AppState, webhook};

const SECRET_HEADER: &str = "x-finnhub-secret";

pub async fn finnhub(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    // Secret belum diatur: anggap endpoint ini nggak ada.
    let Some(expected) = &state.webhook_secret else {
        return Err(AppError::NotFound("endpoint tidak ada".into()));
    };

    let given = headers
        .get(SECRET_HEADER)
        .map(|value| value.as_bytes())
        .unwrap_or_default();
    // Perbandingan constant-time: lama waktunya nggak bergantung di byte ke berapa
    // perbedaan pertama muncul, jadi secret nggak bisa ditebak byte demi byte lewat
    // pengukuran waktu respons. `==` biasa berhenti di byte pertama yang beda.
    if !bool::from(given.ct_eq(expected.expose_secret().as_bytes())) {
        tracing::warn!("webhook: secret salah atau tidak ada");
        return Err(AppError::Unauthorized);
    }

    // Validasi saja (IgnoredAny = parse tanpa membangun apa pun); teksnya diteruskan ke Postgres.
    let payload = String::from_utf8(body.to_vec())
        .ok()
        .filter(|text| serde_json::from_str::<IgnoredAny>(text).is_ok())
        .ok_or_else(|| AppError::BadRequest("body harus JSON".into()))?;

    // Dilacak TaskTracker: saat shutdown, event yang sedang disimpan ditunggu dulu.
    let db = state.db.clone();
    state.tasks.spawn(async move {
        match webhook::store(&db, &payload).await {
            Ok(id) => tracing::info!(id, bytes = payload.len(), "webhook: event disimpan"),
            // Sudah dibalas 200, jadi Finnhub nggak akan kirim ulang. Minimal tercatat di log.
            Err(err) => tracing::error!(error = %err, "webhook: gagal menyimpan event"),
        }
    });

    Ok(StatusCode::OK)
}
