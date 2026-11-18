//! Kontrak sumber data pasar. Route dan cache cuma kenal trait ini, bukan Finnhub,
//! jadi sumber data bisa diganti mock di test.

mod cache;

pub use cache::MarketCache;

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

/// Profil perusahaan. Dashboard baru butuh nama; field lain ditambah kalau dipakai.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Profile {
    /// Finnhub membalas `{}` untuk ticker yang nggak dikenal, jadi default ke string kosong.
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatus {
    pub is_open: bool,
    /// `"pre-market"`, `"regular"`, `"post-market"`, atau `null` saat tutup.
    pub session: Option<String>,
}

/// Satu transaksi dari stream real-time. Seperti `Quote`: nama pendek Finnhub waktu
/// deserialize, nama terbaca waktu dikirim ke browser.
#[derive(Debug, Deserialize, Serialize)]
pub struct Trade {
    #[serde(rename(deserialize = "s"))]
    pub symbol: String,
    #[serde(rename(deserialize = "p"))]
    pub price: f64,
    /// Unix timestamp dalam milidetik.
    #[serde(rename(deserialize = "t"))]
    pub timestamp: i64,
    #[serde(rename(deserialize = "v"))]
    pub volume: f64,
}

/// Satu pesan trade dari upstream (bisa berisi banyak trade). `Arc` supaya broadcast ke
/// N client cuma menyalin pointer, bukan isi datanya.
pub type TradeBatch = std::sync::Arc<[Trade]>;

/// Sumber trade real-time. Cukup subscribe/unsubscribe; trade-nya sendiri datang lewat
/// broadcast channel. Method-nya sinkron (cuma menitip perintah), jadi nggak perlu async_trait.
pub trait TradeFeed: Send + Sync {
    fn subscribe(&self, symbol: &str);
    fn unsubscribe(&self, symbol: &str);
}

/// `" nvda "` → `Some("NVDA")`. Huruf/angka plus `.` `-` `:`, maks 20 karakter:
/// cukup untuk `BRK.B` dan simbol berprefiks bursa seperti `BINANCE:BTCUSDT`.
pub fn normalize_symbol(raw: &str) -> Option<String> {
    let symbol = raw.trim().to_ascii_uppercase();
    let valid = (1..=20).contains(&symbol.len())
        && symbol
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'));
    valid.then_some(symbol)
}

/// `Clone` dibutuhkan cache: satu error dibagi ke semua request yang nunggu hasil yang sama.
#[derive(Debug, Clone, thiserror::Error)]
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
    async fn profile(&self, symbol: &str) -> Result<Profile, MarketError>;
    async fn market_status(&self) -> Result<MarketStatus, MarketError>;
}

#[cfg(test)]
mod tests {
    use super::normalize_symbol;

    #[test]
    fn normalizes_case_and_whitespace() {
        assert_eq!(normalize_symbol(" nvda ").as_deref(), Some("NVDA"));
        assert_eq!(normalize_symbol("brk.b").as_deref(), Some("BRK.B"));
        assert_eq!(
            normalize_symbol("binance:btcusdt").as_deref(),
            Some("BINANCE:BTCUSDT")
        );
    }

    #[test]
    fn rejects_invalid_symbols() {
        let too_long = "A".repeat(21);
        for raw in ["", "   ", "AAPL;DROP", "NVDA&token=x", "A B", &too_long] {
            assert!(normalize_symbol(raw).is_none(), "harusnya ditolak: {raw:?}");
        }
    }
}
