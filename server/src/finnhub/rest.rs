use std::{
    num::NonZeroU32,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use reqwest::{StatusCode, header::HeaderValue};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, de::DeserializeOwned};

use crate::market::{
    CompanyNews, MarketDataSource, MarketError, MarketStatus, NewsArticle, Profile, Quote,
    SymbolInfo, SymbolKind, iso_date, normalize_symbol, prepare_news,
};
use crate::metrics::{Outcome, finnhub_request};

const BASE_URL: &str = "https://finnhub.io/api/v1";

/// Free tier: 60 call/menit dan maks 30 call/detik.
/// GCRA: rata-rata 1 call/detik dengan burst 25, jadi nggak pernah tembus 30 dalam 1 detik.
const QUOTA: Quota =
    Quota::per_minute(NonZeroU32::new(60).unwrap()).allow_burst(NonZeroU32::new(25).unwrap());

/// Jatah khusus berita, DI ATAS jatah global: berita paling banyak memakai 20 dari 60 call
/// per menit, jadi quote dan profil selalu kebagian minimal 40, berapa pun pengunjungnya.
const NEWS_QUOTA: Quota =
    Quota::per_minute(NonZeroU32::new(20).unwrap()).allow_burst(NonZeroU32::new(5).unwrap());

/// Batas tunggu jatah berita. Lebih panjang dari jeda isi ulangnya (60 s / 20 = 3 s), supaya
/// request berita berikutnya setelah lonjakan habis dapat slot, bukan 429.
const NEWS_MAX_WAIT: Duration = Duration::from_secs(4);

/// Rentang berita yang diminta ke Finnhub.
const NEWS_DAYS: i64 = 7;

/// Batas antre nunggu jatah limiter sebelum request dibalas 429.
const MAX_WAIT: Duration = Duration::from_secs(2);

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Daftar simbol US ±7 MB; di jaringan biasa butuh ±5 detik.
const SYMBOL_LIST_TIMEOUT: Duration = Duration::from_secs(60);

/// Client REST Finnhub. Satu instance dipakai bareng semua request (lewat `Arc` di `AppState`),
/// jadi limiter-nya berlaku global untuk seluruh server.
pub struct FinnhubRest {
    http: reqwest::Client,
    /// Key dikirim lewat header, bukan query string, supaya nggak ikut ke-log bareng URL.
    token: HeaderValue,
    limiter: DefaultDirectRateLimiter,
    news_limiter: DefaultDirectRateLimiter,
}

impl FinnhubRest {
    pub fn new(api_key: &SecretString) -> anyhow::Result<Self> {
        let mut token = HeaderValue::from_str(api_key.expose_secret())?;
        token.set_sensitive(true);

        let http = reqwest::Client::builder().build()?;

        Ok(Self {
            http,
            token,
            limiter: RateLimiter::direct(QUOTA),
            news_limiter: RateLimiter::direct(NEWS_QUOTA),
        })
    }

    /// Ambil jatah berita dulu; jatah global diambil `get_json` sesudahnya.
    async fn acquire_news(&self) -> Result<(), MarketError> {
        tokio::time::timeout(NEWS_MAX_WAIT, self.news_limiter.until_ready())
            .await
            .map_err(|_| {
                finnhub_request("/company-news", Outcome::LimitedLocally);
                MarketError::RateLimited
            })
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, MarketError> {
        self.get_json_within(path, query, REQUEST_TIMEOUT).await
    }

    /// GET ke Finnhub lalu parse JSON jadi `T`: tunggu jatah limiter, kirim, lalu catat
    /// hasilnya per endpoint (tepat sekali per panggilan). Tipe `T` ditentukan pemanggil.
    async fn get_json_within<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<T, MarketError> {
        if tokio::time::timeout(MAX_WAIT, self.limiter.until_ready())
            .await
            .is_err()
        {
            finnhub_request(path, Outcome::LimitedLocally);
            return Err(MarketError::RateLimited);
        }

        let result = self.send(path, query, timeout).await;
        let outcome = match &result {
            Ok(_) => Outcome::Ok,
            // Di sini cuma bisa datang dari 429 Finnhub: limiter kita sudah lewat di atas.
            Err(MarketError::RateLimited) => Outcome::RateLimited,
            Err(_) => Outcome::Error,
        };
        finnhub_request(path, outcome);
        result
    }

    /// Kirim satu GET dan terjemahkan status HTTP ke `MarketError`.
    async fn send<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<T, MarketError> {
        tracing::debug!(path, ?query, "finnhub request");
        let res = self
            .http
            .get(format!("{BASE_URL}{path}"))
            .query(query)
            .header("X-Finnhub-Token", self.token.clone())
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| MarketError::Upstream(e.to_string()))?;

        match res.status() {
            status if status.is_success() => {}
            // 401 = "Invalid API key"; 403 = "You don't have access to this resource."
            StatusCode::UNAUTHORIZED => return Err(MarketError::InvalidApiKey),
            StatusCode::FORBIDDEN => return Err(MarketError::NotAvailable),
            StatusCode::TOO_MANY_REQUESTS => return Err(MarketError::RateLimited),
            status => return Err(MarketError::Upstream(format!("HTTP {status} dari {path}"))),
        }

        res.json()
            .await
            .map_err(|e| MarketError::Upstream(format!("JSON tidak valid dari {path}: {e}")))
    }
}

#[async_trait]
impl MarketDataSource for FinnhubRest {
    async fn quote(&self, symbol: &str) -> Result<Quote, MarketError> {
        let quote: Quote = self.get_json("/quote", &[("symbol", symbol)]).await?;

        // Finnhub nggak balas 404 untuk ticker ngawur, tapi semua angkanya 0.
        if quote.price == 0.0 && quote.prev_close == 0.0 {
            return Err(MarketError::UnknownSymbol(symbol.to_string()));
        }
        Ok(quote)
    }

    async fn profile(&self, symbol: &str) -> Result<Profile, MarketError> {
        let mut profile: Profile = self
            .get_json("/stock/profile2", &[("symbol", symbol)])
            .await?;

        // Ticker ngawur dibalas `{}` (200), jadi nama kosong.
        if profile.name.is_empty() {
            return Err(MarketError::UnknownSymbol(symbol.to_string()));
        }
        profile.logo = safe_logo(profile.logo);
        Ok(profile)
    }

    async fn market_status(&self) -> Result<MarketStatus, MarketError> {
        self.get_json("/stock/market-status", &[("exchange", "US")])
            .await
    }

    async fn company_news(&self, symbol: &str) -> Result<CompanyNews, MarketError> {
        self.acquire_news().await?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = i64::try_from(now.as_secs()).unwrap_or(i64::MAX);
        let now_ms = i64::try_from(now.as_millis()).unwrap_or(i64::MAX);
        // Tanggal UTC; Finnhub cuma menerima tanggal, bukan jam.
        let from = iso_date(now_secs - NEWS_DAYS * 86_400);
        let to = iso_date(now_secs);
        let articles: Vec<NewsArticle> = self
            .get_json(
                "/company-news",
                &[("symbol", symbol), ("from", &from), ("to", &to)],
            )
            .await?;
        Ok(prepare_news(articles, now_ms))
    }

    async fn us_symbols(&self) -> Result<Arc<[SymbolInfo]>, MarketError> {
        // Finnhub membalas 302 ke file statis di static2.finnhub.io; reqwest mengikutinya
        // sendiri (header token ikut terkirim, tapi host-nya tetap milik Finnhub).
        let raw: Vec<RawSymbol> = self
            .get_json_within("/stock/symbol", &[("exchange", "US")], SYMBOL_LIST_TIMEOUT)
            .await?;
        let total = raw.len();

        let mut symbols: Vec<SymbolInfo> =
            raw.into_iter().filter_map(RawSymbol::into_info).collect();
        symbols.sort_unstable_by(|a, b| a.symbol.cmp(&b.symbol));

        tracing::info!(total, kept = symbols.len(), "daftar simbol US dimuat");
        // Vec → Arc<[T]>: satu alokasi yang dibagi semua request, nggak pernah disalin.
        Ok(symbols.into())
    }
}

/// Satu baris `/stock/symbol`. Field lain (figi, isin, currency, ...) diabaikan serde,
/// jadi nggak ikut dialokasikan.
#[derive(Deserialize)]
struct RawSymbol {
    symbol: String,
    description: String,
    /// Kode bursa (ISO 10383), mis. "XNAS" untuk Nasdaq.
    mic: String,
    #[serde(rename = "type")]
    kind: String,
}

/// Nasdaq, NYSE, NYSE Arca, Cboe BZX, NYSE American. Di luar ini kebanyakan OTC
/// (±17 ribu dari 31 ribu simbol): nggak likuid dan jarang ada di stream trade.
const MAIN_EXCHANGES: &[&str] = &["XNAS", "XNYS", "ARCX", "BATS", "XASE"];

impl RawSymbol {
    /// `None` untuk yang nggak layak masuk watchlist: OTC, warrant, unit, right, dsb.
    fn into_info(self) -> Option<SymbolInfo> {
        if !MAIN_EXCHANGES.contains(&self.mic.as_str()) {
            return None;
        }
        let kind = match self.kind.as_str() {
            "Common Stock" | "ADR" | "REIT" => SymbolKind::Stock,
            "ETP" => SymbolKind::Etf,
            _ => return None,
        };
        // Pastikan simbolnya memang bisa ditambahkan lewat POST /api/watchlist.
        normalize_symbol(&self.symbol).filter(|s| *s == self.symbol)?;
        Some(SymbolInfo {
            symbol: self.symbol,
            name: self.description,
            kind,
        })
    }
}

/// URL logo dipasang browser sebagai `<img src>`, jadi cuma https yang diteruskan.
/// Finnhub membalas `""` untuk perusahaan tanpa logo; itu juga jadi `None`.
fn safe_logo(url: Option<String>) -> Option<String> {
    url.filter(|url| url.starts_with("https://"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Setelah lonjakan 5, jatah berita terisi satu tiap 3 detik. Browser meminta berita satu
    /// per satu, jadi request berikutnya harus sempat menunggu slot itu (bukan 429), tapi
    /// tiga request bersamaan tetap cuma dapat satu slot; sisanya ditolak.
    #[tokio::test]
    async fn news_limiter_waits_for_the_next_slot_after_the_burst() {
        let m = crate::metrics::testing::capture();
        let rest = FinnhubRest::new(&SecretString::from("test")).unwrap();
        for _ in 0..5 {
            rest.acquire_news().await.unwrap();
        }
        let (a, b, c) = tokio::join!(
            rest.acquire_news(),
            rest.acquire_news(),
            rest.acquire_news()
        );
        let granted = [a, b, c].iter().filter(|r| r.is_ok()).count();
        assert_eq!(
            granted, 1,
            "satu slot terisi dalam waktu tunggu, dua ditolak"
        );
        // Yang ditolak limiter kita sendiri tercatat terpisah dari 429 milik Finnhub.
        assert_eq!(
            m.counter(
                "tickstream_finnhub_requests_total",
                &[
                    ("endpoint", "/company-news"),
                    ("outcome", "limited_locally")
                ]
            ),
            2
        );
    }

    #[test]
    fn passes_only_https_logos() {
        let logo = |url: &str| safe_logo(Some(url.into()));
        let aapl = "https://static2.finnhub.io/file/publicdatany/finnhubimage/stock_logo/AAPL.png";
        assert_eq!(logo(aapl).as_deref(), Some(aapl));
        assert_eq!(logo(""), None);
        assert_eq!(logo("http://example.com/a.png"), None);
        assert_eq!(logo("javascript:alert(1)"), None);
        assert_eq!(safe_logo(None), None);
    }

    fn raw(symbol: &str, mic: &str, kind: &str) -> RawSymbol {
        RawSymbol {
            symbol: symbol.into(),
            description: format!("{symbol} CORP"),
            mic: mic.into(),
            kind: kind.into(),
        }
    }

    #[test]
    fn keeps_stocks_and_etfs_on_main_exchanges() {
        let kept = |r: RawSymbol| r.into_info().map(|info| info.kind);
        assert_eq!(
            kept(raw("AAPL", "XNAS", "Common Stock")),
            Some(SymbolKind::Stock)
        );
        assert_eq!(kept(raw("TSM", "XNYS", "ADR")), Some(SymbolKind::Stock));
        assert_eq!(
            kept(raw("BRK.B", "XNYS", "Common Stock")),
            Some(SymbolKind::Stock)
        );
        assert_eq!(kept(raw("SPY", "ARCX", "ETP")), Some(SymbolKind::Etf));
    }

    #[test]
    fn drops_otc_and_odd_instruments() {
        for r in [
            raw("FMOO", "OOTC", "Common Stock"), // OTC
            raw("ABCW", "XNAS", "Equity WRT"),   // warrant
            raw("ABCU", "XNAS", "Unit"),
            raw("ab c", "XNAS", "Common Stock"), // simbol yang ditolak normalize_symbol
        ] {
            let symbol = r.symbol.clone();
            assert!(r.into_info().is_none(), "harusnya dibuang: {symbol}");
        }
    }
}
