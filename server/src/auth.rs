//! Pemilik vs pengunjung. Semua orang boleh melihat watchlist; yang mengubahnya cuma
//! pemilik, yang membuktikan diri dengan header `Authorization: Bearer <ADMIN_TOKEN>`.

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
};
use secrecy::ExposeSecret;
use subtle::ConstantTimeEq;

use crate::{error::AppError, state::AppState};

/// Bukti bahwa request datang dari pemilik. Handler yang butuh izin cukup menerima
/// parameter `_: Admin`: axum menjalankan pengecekan ini sebelum handler dipanggil,
/// dan kalau gagal, handler-nya nggak pernah jalan sama sekali.
pub struct Admin;

impl FromRequestParts<AppState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        // Token belum diatur: nggak ada yang bisa jadi pemilik, watchlist read-only.
        let Some(expected) = &state.admin_token else {
            return Err(AppError::Unauthorized);
        };
        let given = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or_default();
        // Constant-time, sama seperti secret webhook: token nggak bisa ditebak lewat
        // mengukur lama respons.
        if bool::from(given.as_bytes().ct_eq(expected.expose_secret().as_bytes())) {
            Ok(Admin)
        } else {
            Err(AppError::Unauthorized)
        }
    }
}

/// `GET /api/admin`: 204 kalau token-nya benar. Dipakai browser untuk mengecek token
/// sebelum menyimpannya.
pub async fn check(_: Admin) -> StatusCode {
    StatusCode::NO_CONTENT
}
