use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::market::MarketError;

/// Error yang bisa keluar dari handler HTTP. Axum mengubahnya jadi response lewat `IntoResponse`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    /// Permintaan valid tapi bentrok dengan keadaan sekarang (duplikat, kuota penuh).
    #[error("{0}")]
    Conflict(String),
    /// `#[from]` bikin `?` otomatis mengubah `MarketError` jadi `AppError::Market`.
    #[error(transparent)]
    Market(#[from] MarketError),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Market(MarketError::UnknownSymbol(_)) => StatusCode::NOT_FOUND,
            AppError::Market(MarketError::NotAvailable) => StatusCode::FORBIDDEN,
            AppError::Market(MarketError::RateLimited) => StatusCode::TOO_MANY_REQUESTS,
            AppError::Market(MarketError::InvalidApiKey | MarketError::Upstream(_)) => {
                StatusCode::BAD_GATEWAY
            }
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        if status.is_server_error() {
            tracing::warn!(error = %self, "request gagal");
        }

        let message = match &self {
            // Detail teknis cukup di log; client dapat pesan umum.
            AppError::Market(MarketError::Upstream(_)) => "gagal menghubungi sumber data".into(),
            AppError::Database(_) => "kesalahan server".into(),
            other => other.to_string(),
        };

        (status, Json(ErrorBody { error: message })).into_response()
    }
}
