use std::{
    hash::Hash,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use moka::future::Cache;

use super::{CompanyNews, MarketDataSource, MarketError, MarketStatus, Profile, Quote, SymbolInfo};

const QUOTE_TTL: Duration = Duration::from_secs(15);
const PROFILE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const STATUS_TTL: Duration = Duration::from_secs(60);
/// Berita dibagi ke semua pengunjung; Finnhub cuma ditanya sekali per simbol per 15 menit.
const NEWS_TTL: Duration = Duration::from_secs(15 * 60);
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
    news: Cache<String, CompanyNews>,
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
            news: Cache::builder()
                .max_capacity(MAX_ENTRIES)
                .time_to_live(NEWS_TTL)
                .build(),
            symbols: Cache::builder().time_to_live(SYMBOLS_TTL).build(),
        }
    }
}

/// Ambil `key` dari `cache`, memanggil `load` kalau belum ada, dan catat hit/miss-nya.
///
/// Miss = panggilan ini yang menjalankan `load`, atau hasilnya error (termasuk ikut menunggu
/// panggilan orang lain yang gagal): dua-duanya berarti cache nggak menyelamatkan request.
async fn cached<K, V>(
    cache: &Cache<K, V>,
    name: &'static str,
    key: K,
    load: impl Future<Output = Result<V, MarketError>> + Send,
) -> Result<V, MarketError>
where
    K: Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    let loaded = AtomicBool::new(false);
    let result = cache
        .try_get_with(key, async {
            loaded.store(true, Ordering::Relaxed);
            load.await
        })
        .await
        .map_err(Arc::unwrap_or_clone);
    crate::metrics::cache_lookup(name, result.is_ok() && !loaded.load(Ordering::Relaxed));
    result
}

/// `try_get_with` cuma menjalankan future `init` kalau key belum ada. Kalau banyak request
/// nanya key yang sama bersamaan, cuma satu yang manggil sumber; sisanya nunggu hasil itu.
#[async_trait]
impl MarketDataSource for MarketCache {
    async fn quote(&self, symbol: &str) -> Result<Quote, MarketError> {
        cached(
            &self.quotes,
            "quote",
            symbol.to_string(),
            self.source.quote(symbol),
        )
        .await
    }

    async fn profile(&self, symbol: &str) -> Result<Profile, MarketError> {
        cached(
            &self.profiles,
            "profile",
            symbol.to_string(),
            self.source.profile(symbol),
        )
        .await
    }

    async fn market_status(&self) -> Result<MarketStatus, MarketError> {
        cached(
            &self.status,
            "market_status",
            (),
            self.source.market_status(),
        )
        .await
    }

    async fn company_news(&self, symbol: &str) -> Result<CompanyNews, MarketError> {
        cached(
            &self.news,
            "news",
            symbol.to_string(),
            self.source.company_news(symbol),
        )
        .await
    }

    /// Yang di-cache `Arc`-nya: cache hit cuma menambah reference count.
    async fn us_symbols(&self) -> Result<Arc<[SymbolInfo]>, MarketError> {
        cached(&self.symbols, "symbols", (), self.source.us_symbols()).await
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
                logo: None,
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

        async fn company_news(&self, _symbol: &str) -> Result<CompanyNews, MarketError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(CompanyNews {
                count_24h: 0,
                items: Vec::new(),
            })
        }
    }

    fn setup() -> (Arc<CountingSource>, MarketCache) {
        let source = Arc::new(CountingSource::default());
        let cache = MarketCache::new(source.clone());
        (source, cache)
    }

    const LOOKUPS: &str = "tickstream_cache_requests_total";

    #[tokio::test]
    async fn cache_counts_hits_and_misses() {
        let m = crate::metrics::testing::capture();
        let (_, cache) = setup();

        cache.quote("AAPL").await.unwrap();
        cache.quote("AAPL").await.unwrap();

        assert_eq!(
            m.counter(LOOKUPS, &[("cache", "quote"), ("result", "miss")]),
            1
        );
        assert_eq!(
            m.counter(LOOKUPS, &[("cache", "quote"), ("result", "hit")]),
            1
        );
    }

    /// Dua pencarian bersamaan berbagi satu panggilan ke sumber. Kalau panggilan itu gagal,
    /// yang ikut menunggu juga nggak dapat data, jadi bukan hit.
    #[tokio::test]
    async fn failed_lookup_is_a_miss() {
        let m = crate::metrics::testing::capture();
        let (source, cache) = setup();

        let (a, b) = tokio::join!(cache.quote("BAD"), cache.quote("BAD"));
        assert!(a.is_err() && b.is_err());
        assert_eq!(source.calls(), 1, "satu panggilan dibagi berdua");

        assert_eq!(
            m.counter(LOOKUPS, &[("cache", "quote"), ("result", "miss")]),
            2
        );
        assert_eq!(
            m.counter(LOOKUPS, &[("cache", "quote"), ("result", "hit")]),
            0
        );
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
    async fn news_is_cached_per_symbol() {
        let (source, cache) = setup();

        let first = cache.company_news("AAPL").await.unwrap();
        let second = cache.company_news("AAPL").await.unwrap();
        cache.company_news("NVDA").await.unwrap();

        assert_eq!(source.calls(), 2, "1x AAPL + 1x NVDA");
        assert_eq!(first, second);
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
