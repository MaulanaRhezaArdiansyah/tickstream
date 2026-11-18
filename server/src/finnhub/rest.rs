use std::{num::NonZeroU32, time::Duration};

use async_trait::async_trait;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use reqwest::{StatusCode, header::HeaderValue};
use secrecy::{ExposeSecret, SecretString};

use crate::market::{MarketDataSource, MarketError, Quote};

const BASE_URL: &str = "https://finnhub.io/api/v1";

/// Free tier: 60 call/menit dan maks 30 call/detik.
/// GCRA: rata-rata 1 call/detik dengan burst 25, jadi nggak pernah tembus 30 dalam 1 detik.
const QUOTA: Quota =
    Quota::per_minute(NonZeroU32::new(60).unwrap()).allow_burst(NonZeroU32::new(25).unwrap());

/// Batas antre nunggu jatah limiter sebelum request dibalas 429.
const MAX_WAIT: Duration = Duration::from_secs(2);

/// Client REST Finnhub. Satu instance dipakai bareng semua request (lewat `Arc` di `AppState`),
/// jadi limiter-nya berlaku global untuk seluruh server.
pub struct FinnhubRest {
    http: reqwest::Client,
    /// Key dikirim lewat header, bukan query string, supaya nggak ikut ke-log bareng URL.
    token: HeaderValue,
    limiter: DefaultDirectRateLimiter,
}

impl FinnhubRest {
    pub fn new(api_key: &SecretString) -> anyhow::Result<Self> {
        let mut token = HeaderValue::from_str(api_key.expose_secret())?;
        token.set_sensitive(true);

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self {
            http,
            token,
            limiter: RateLimiter::direct(QUOTA),
        })
    }

    /// GET ke Finnhub: tunggu jatah limiter, kirim, lalu terjemahkan status HTTP ke `MarketError`.
    async fn get(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<reqwest::Response, MarketError> {
        tokio::time::timeout(MAX_WAIT, self.limiter.until_ready())
            .await
            .map_err(|_| MarketError::RateLimited)?;

        let res = self
            .http
            .get(format!("{BASE_URL}{path}"))
            .query(query)
            .header("X-Finnhub-Token", self.token.clone())
            .send()
            .await
            .map_err(|e| MarketError::Upstream(e.to_string()))?;

        match res.status() {
            status if status.is_success() => Ok(res),
            StatusCode::TOO_MANY_REQUESTS => Err(MarketError::RateLimited),
            // 401 = "Invalid API key"; 403 = "You don't have access to this resource."
            StatusCode::UNAUTHORIZED => Err(MarketError::InvalidApiKey),
            StatusCode::FORBIDDEN => Err(MarketError::NotAvailable),
            status => Err(MarketError::Upstream(format!("HTTP {status} dari {path}"))),
        }
    }
}

#[async_trait]
impl MarketDataSource for FinnhubRest {
    async fn quote(&self, symbol: &str) -> Result<Quote, MarketError> {
        let quote: Quote = self
            .get("/quote", &[("symbol", symbol)])
            .await?
            .json()
            .await
            .map_err(|e| MarketError::Upstream(e.to_string()))?;

        // Finnhub nggak balas 404 untuk ticker ngawur, tapi semua angkanya 0.
        if quote.price == 0.0 && quote.prev_close == 0.0 {
            return Err(MarketError::UnknownSymbol(symbol.to_string()));
        }
        Ok(quote)
    }
}
