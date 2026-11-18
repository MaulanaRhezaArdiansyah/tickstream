//! Kontrak sumber data pasar. Route (dan nanti cache) cuma kenal trait ini, bukan Finnhub,
//! jadi sumber data bisa diganti mock di test.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Quote harian satu simbol. Field Finnhub (`c`, `o`, `h`, ...) di-rename waktu deserialize;
/// ke browser dikirim dengan nama camelCase yang terbaca.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Quote {
    #[serde(rename(deserialize = "c"))]
    pub price: f64,
    #[serde(rename(deserialize = "o"))]
    pub open: f64,
    #[serde(rename(deserialize = "h"))]
    pub high: f64,
    #[serde(rename(deserialize = "l"))]
    pub low: f64,
    #[serde(rename(deserialize = "pc"))]
    pub prev_close: f64,
    /// Unix timestamp (detik) quote terakhir.
    #[serde(rename(deserialize = "t"))]
    pub timestamp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum MarketError {
    #[error("simbol {0} tidak ditemukan")]
    UnknownSymbol(String),
    #[error("rate limit sumber data kena, coba lagi sebentar")]
    RateLimited,
    /// Key ditolak. Masalah konfigurasi server, bukan salah user.
    #[error("API key sumber data ditolak")]
    InvalidApiKey,
    /// Key valid, tapi simbol/endpoint ini nggak boleh diakses di free tier.
    #[error("data tidak tersedia di paket free sumber data")]
    NotAvailable,
    /// Detail teknis (timeout, HTTP 5xx, JSON rusak). Cuma untuk log, nggak dikirim ke client.
    #[error("gagal menghubungi sumber data: {0}")]
    Upstream(String),
}

/// `async fn` di trait belum bisa dipakai lewat `dyn` di Rust stable,
/// makanya pakai macro `#[async_trait]`.
#[async_trait]
pub trait MarketDataSource: Send + Sync {
    async fn quote(&self, symbol: &str) -> Result<Quote, MarketError>;
}
