use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use moka::future::Cache;

use super::{MarketDataSource, MarketError, MarketStatus, Profile, Quote, SymbolInfo};

const QUOTE_TTL: Duration = Duration::from_secs(15);
const PROFILE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const STATUS_TTL: Duration = Duration::from_secs(60);
/// Daftar simbol bursa jarang berubah, dan mengambilnya berat (±7 MB dari Finnhub).
const SYMBOLS_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_ENTRIES: u64 = 1_000;

/// Decorator: implement trait yang sama dengan sumber aslinya, tapi menyimpan hasil sukses
/// sebentar. Handler nggak tahu (dan nggak perlu tahu) ada cache di depan Finnhub.
///
/// Error nggak di-cache, jadi request berikutnya mencoba lagi ke sumber.
pub struct MarketCache {
    source: Arc<dyn MarketDataSource>,
    quotes: Cache<String, Quote>,
    profiles: Cache<String, Profile>,
    status: Cache<(), MarketStatus>,
    symbols: Cache<(), Arc<[SymbolInfo]>>,
}

impl MarketCache {
    pub fn new(source: Arc<dyn MarketDataSource>) -> Self {
        Self {
            source,
            quotes: Cache::builder()
                .max_capacity(MAX_ENTRIES)
                .time_to_live(QUOTE_TTL)
                .build(),
            profiles: Cache::builder()
                .max_capacity(MAX_ENTRIES)
                .time_to_live(PROFILE_TTL)
                .build(),
            status: Cache::builder().time_to_live(STATUS_TTL).build(),
            symbols: Cache::builder().time_to_live(SYMBOLS_TTL).build(),
        }
    }
}

/// `try_get_with` cuma menjalankan future `init` kalau key belum ada. Kalau banyak request
/// nanya key yang sama bersamaan, cuma satu yang manggil sumber; sisanya nunggu hasil itu.
#[async_trait]
impl MarketDataSource for MarketCache {
    async fn quote(&self, symbol: &str) -> Result<Quote, MarketError> {
        self.quotes
            .try_get_with(symbol.to_string(), self.source.quote(symbol))
            .await
            .map_err(Arc::unwrap_or_clone)
    }

    async fn profile(&self, symbol: &str) -> Result<Profile, MarketError> {
        self.profiles
            .try_get_with(symbol.to_string(), self.source.profile(symbol))
            .await
            .map_err(Arc::unwrap_or_clone)
    }

    async fn market_status(&self) -> Result<MarketStatus, MarketError> {
        self.status
            .try_get_with((), self.source.market_status())
            .await
            .map_err(Arc::unwrap_or_clone)
    }

    /// Yang di-cache `Arc`-nya: cache hit cuma menambah reference count.
    async fn us_symbols(&self) -> Result<Arc<[SymbolInfo]>, MarketError> {
        self.symbols
            .try_get_with((), self.source.us_symbols())
            .await
            .map_err(Arc::unwrap_or_clone)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Sumber palsu: menghitung berapa kali dipanggil. Simbol `"BAD"` selalu gagal.
    #[derive(Default)]
    struct CountingSource {
        calls: AtomicUsize,
    }

    impl CountingSource {
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl MarketDataSource for CountingSource {
        async fn quote(&self, symbol: &str) -> Result<Quote, MarketError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // Jeda kecil supaya request paralel di test sempat tumpang tindih.
            tokio::time::sleep(Duration::from_millis(20)).await;
            if symbol == "BAD" {
                return Err(MarketError::UnknownSymbol(symbol.to_string()));
            }
            Ok(Quote {
                price: 1.0,
                open: 1.0,
                high: 1.0,
                low: 1.0,
                prev_close: 1.0,
                timestamp: 0,
            })
        }

        async fn profile(&self, _symbol: &str) -> Result<Profile, MarketError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Profile {
                name: "Test Corp".to_string(),
            })
        }

        async fn market_status(&self) -> Result<MarketStatus, MarketError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(MarketStatus {
                is_open: true,
                session: Some("regular".to_string()),
            })
        }

        async fn us_symbols(&self) -> Result<Arc<[SymbolInfo]>, MarketError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new([]))
        }
    }

    fn setup() -> (Arc<CountingSource>, MarketCache) {
        let source = Arc::new(CountingSource::default());
        let cache = MarketCache::new(source.clone());
        (source, cache)
    }

    #[tokio::test]
    async fn repeated_calls_hit_source_once() {
        let (source, cache) = setup();

        cache.quote("AAPL").await.unwrap();
        cache.quote("AAPL").await.unwrap();
        cache.market_status().await.unwrap();
        cache.market_status().await.unwrap();
        cache.us_symbols().await.unwrap();
        cache.us_symbols().await.unwrap();

        assert_eq!(
            source.calls(),
            3,
            "1x quote AAPL + 1x market status + 1x daftar simbol"
        );
    }

    #[tokio::test]
    async fn symbols_are_cached_separately() {
        let (source, cache) = setup();

        cache.quote("AAPL").await.unwrap();
        cache.quote("NVDA").await.unwrap();
        cache.profile("AAPL").await.unwrap();

        assert_eq!(source.calls(), 3);
    }

    #[tokio::test]
    async fn concurrent_misses_are_coalesced() {
        let (source, cache) = setup();
        let cache = Arc::new(cache);

        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..10 {
            let cache = cache.clone();
            tasks.spawn(async move { cache.quote("AAPL").await });
        }
        while let Some(result) = tasks.join_next().await {
            result.unwrap().unwrap();
        }

        assert_eq!(
            source.calls(),
            1,
            "10 request barengan, sumber cuma dipanggil sekali"
        );
    }

    #[tokio::test]
    async fn errors_are_not_cached() {
        let (source, cache) = setup();

        assert!(matches!(
            cache.quote("BAD").await,
            Err(MarketError::UnknownSymbol(_))
        ));
        assert!(cache.quote("BAD").await.is_err());

        assert_eq!(source.calls(), 2);
    }
}
