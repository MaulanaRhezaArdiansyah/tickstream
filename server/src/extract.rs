//! Extractor bawaan axum (`Json`, `Query`, `Path`) membalas input rusak dengan teks polos.
//! Versi di sini sama persis, cuma penolakannya diubah jadi `AppError`, jadi semua error
//! API berbentuk `{"error": "..."}`.
//!
//! `derive(FromRequest)` membuat wrapper yang meminjam extractor asli lewat `via(...)` dan
//! mengubah rejection-nya lewat `From<...Rejection> for AppError` (lihat error.rs).

use axum::extract::{FromRequest, FromRequestParts};

use crate::error::AppError;

/// Body JSON. Pakai di handler: `AppJson(body): AppJson<AddBody>`.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct AppJson<T>(pub T);

/// Query string, mis. `?symbol=NVDA`.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
pub struct AppQuery<T>(pub T);

/// Segmen path, mis. `/api/watchlist/{symbol}`.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(AppError))]
pub struct AppPath<T>(pub T);
