//! Riwayat harga singkat untuk sparkline: 30 menit terakhir per simbol.
//!
//! Trade dikelompokkan per 10 detik dan tiap kelompok cuma menyimpan harga terakhirnya.
//! Kenapa nggak simpan trade mentah? Saham ramai bisa puluhan trade per detik, saham sepi
//! cuma beberapa per menit. Dengan kelompok waktu, semua baris punya skala waktu yang sama
//! dan memorinya tetap kecil (maksimal 180 titik per simbol), berapa pun ramainya.
//!
//! Kelompok cuma dibuat kalau ada trade. Di luar jam bursa isinya jadi "30 menit terakhir
//! yang ada transaksinya", bukan garis datar.

#![cfg_attr(not(test), allow(dead_code))] // `points` dipakai mulai Task 3 (/ws)

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use sqlx::PgPool;
use tokio::sync::broadcast::{self, error::RecvError};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    hub::{Hub, HubError, Subscription},
    market::{Trade, TradeBatch},
    watchlist,
};

/// Lebar satu kelompok, dalam milidetik.
pub const BUCKET_MS: i64 = 10_000;
/// 180 × 10 detik = 30 menit.
pub const MAX_BUCKETS: usize = 180;

/// `(awal kelompok dalam Unix ms, harga terakhir)`. Tuple di-serialize serde jadi array
/// `[t, harga]`, bentuk yang dikirim ke browser.
pub type Point = (i64, f64);

#[derive(Default)]
pub struct History {
    /// `VecDeque`: buang dari depan dan tambah di belakang sama-sama O(1).
    buckets: Mutex<HashMap<String, VecDeque<Point>>>,
}

impl History {
    pub fn record(&self, trades: &[Trade]) {
        let mut buckets = self.buckets.lock().unwrap();
        for trade in trades {
            // `div_euclid` = pembulatan ke bawah juga untuk angka negatif (`/` membulatkan
            // ke arah nol). Timestamp negatif nggak realistis, tapi aturannya jadi pasti.
            let start = trade.timestamp.div_euclid(BUCKET_MS) * BUCKET_MS;
            let points = buckets.entry(trade.symbol.clone()).or_default();
            match points.back_mut() {
                Some(last) if last.0 == start => last.1 = trade.price,
                // Trade yang datang telat (lebih lama dari kelompok terbaru) nggak boleh
                // menulis ulang atau mengacak urutan garis.
                Some(last) if last.0 > start => {}
                _ => {
                    points.push_back((start, trade.price));
                    if points.len() > MAX_BUCKETS {
                        points.pop_front();
                    }
                }
            }
        }
    }

    /// Urut dari yang paling lama. Simbol tanpa riwayat → kosong.
    pub fn points(&self, symbol: &str) -> Vec<Point> {
        let buckets = self.buckets.lock().unwrap();
        buckets
            .get(symbol)
            .map(|points| points.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn forget(&self, symbol: &str) {
        self.buckets.lock().unwrap().remove(symbol);
    }
}

/// Pencatat riwayat untuk semua simbol watchlist.
///
/// Hub cuma berlangganan ke Finnhub selama ada browser yang menonton. Supaya riwayat tetap
/// terkumpul saat halaman sepi, server memegang `Subscription` miliknya sendiri, persis
/// seperti satu "pengunjung" yang nggak pernah pulang. Slot browser nggak berkurang:
/// batas watchlist sama dengan batas simbol live, dan browser cuma boleh subscribe simbol
/// watchlist.
pub struct Recorder {
    history: History,
    subscription: Mutex<Subscription>,
}

impl Recorder {
    pub fn new(hub: &Arc<Hub>) -> Arc<Self> {
        Arc::new(Self {
            history: History::default(),
            subscription: Mutex::new(hub.subscription()),
        })
    }

    pub fn watch(&self, symbol: &str) -> Result<(), HubError> {
        self.subscription.lock().unwrap().add(symbol)
    }

    /// Simbol yang dihapus dari watchlist ikut kehilangan riwayatnya: kalau ditambah lagi,
    /// garisnya mulai dari kosong, bukan menyambung data lama yang sudah bolong.
    pub fn unwatch(&self, symbol: &str) {
        self.subscription.lock().unwrap().remove(symbol);
        self.history.forget(symbol);
    }

    pub fn points(&self, symbol: &str) -> Vec<Point> {
        self.history.points(symbol)
    }

    pub fn record(&self, trades: &[Trade]) {
        self.history.record(trades);
    }

    /// Dipanggil sekali saat startup. Gagal pantau satu simbol cuma dicatat: server tetap
    /// jalan, simbol itu saja yang sparkline-nya kosong.
    pub async fn watch_watchlist(&self, db: &PgPool) -> sqlx::Result<()> {
        for symbol in watchlist::list(db).await? {
            if let Err(err) = self.watch(&symbol) {
                tracing::warn!(%symbol, error = %err, "recorder: gagal memantau simbol");
            }
        }
        Ok(())
    }

    /// Task background yang membaca broadcast trade, sama seperti satu koneksi `/ws`.
    pub fn spawn(
        self: Arc<Self>,
        mut trades: broadcast::Receiver<TradeBatch>,
        shutdown: CancellationToken,
        tasks: &TaskTracker,
    ) {
        tasks.spawn(async move {
            loop {
                tokio::select! {
                    () = shutdown.cancelled() => break,
                    batch = trades.recv() => match batch {
                        Ok(batch) => self.record(&batch),
                        // Ketinggalan beberapa batch cuma bikin beberapa kelompok lebih
                        // jarang; lanjut dari yang terbaru.
                        Err(RecvError::Lagged(skipped)) => {
                            tracing::warn!(skipped, "recorder tertinggal, tick dilewati");
                        }
                        Err(RecvError::Closed) => break,
                    },
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use tokio::sync::broadcast;
    use tokio_util::{sync::CancellationToken, task::TaskTracker};

    use super::*;
    use crate::{
        hub::Hub,
        market::{TradeBatch, TradeFeed},
    };

    /// Mock `TradeFeed`: mencatat perintah yang sampai ke "upstream".
    #[derive(Default)]
    struct RecordingFeed(Mutex<Vec<String>>);

    impl TradeFeed for RecordingFeed {
        fn subscribe(&self, symbol: &str) {
            self.0.lock().unwrap().push(format!("+{symbol}"));
        }
        fn unsubscribe(&self, symbol: &str) {
            self.0.lock().unwrap().push(format!("-{symbol}"));
        }
    }

    fn log(feed: &RecordingFeed) -> Vec<String> {
        feed.0.lock().unwrap().clone()
    }

    fn recorder() -> (
        Arc<Recorder>,
        Arc<RecordingFeed>,
        broadcast::Sender<TradeBatch>,
    ) {
        let feed = Arc::new(RecordingFeed::default());
        let (trades, _) = broadcast::channel(8);
        let hub = Arc::new(Hub::new(feed.clone(), trades.clone()));
        (Recorder::new(&hub), feed, trades)
    }

    fn trade(symbol: &str, timestamp: i64, price: f64) -> Trade {
        Trade {
            symbol: symbol.to_string(),
            price,
            timestamp,
            volume: 1.0,
        }
    }

    fn record(history: &History, trades: &[(&str, i64, f64)]) {
        let trades: Vec<Trade> = trades.iter().map(|&(s, t, p)| trade(s, t, p)).collect();
        history.record(&trades);
    }

    #[test]
    fn same_bucket_keeps_the_last_price() {
        let history = History::default();
        record(
            &history,
            &[
                ("AAPL", 0, 10.0),
                ("AAPL", 9_999, 11.0),
                ("AAPL", 10_000, 12.0),
            ],
        );
        assert_eq!(history.points("AAPL"), [(0, 11.0), (10_000, 12.0)]);
    }

    #[test]
    fn bucket_start_is_floored() {
        let history = History::default();
        record(&history, &[("AAPL", 1_700_000_019_999, 5.0)]);
        assert_eq!(history.points("AAPL"), [(1_700_000_010_000, 5.0)]);
    }

    #[test]
    fn older_trade_is_ignored() {
        let history = History::default();
        record(&history, &[("AAPL", 20_000, 5.0), ("AAPL", 10_000, 9.0)]);
        assert_eq!(history.points("AAPL"), [(20_000, 5.0)]);
    }

    #[test]
    fn keeps_only_the_newest_180() {
        let history = History::default();
        let trades: Vec<Trade> = (0..=180)
            .map(|i| trade("AAPL", i * BUCKET_MS, i as f64))
            .collect();
        history.record(&trades);

        let points = history.points("AAPL");
        assert_eq!(points.len(), MAX_BUCKETS);
        assert_eq!(points.first(), Some(&(10_000, 1.0)));
        assert_eq!(points.last(), Some(&(1_800_000, 180.0)));
    }

    #[test]
    fn symbols_are_separate_and_unknown_is_empty() {
        let history = History::default();
        record(&history, &[("AAPL", 0, 1.0), ("NVDA", 0, 2.0)]);
        assert_eq!(history.points("AAPL"), [(0, 1.0)]);
        assert_eq!(history.points("NVDA"), [(0, 2.0)]);
        assert!(history.points("MSFT").is_empty());
    }

    #[test]
    fn forget_drops_history() {
        let history = History::default();
        record(&history, &[("AAPL", 0, 1.0)]);
        history.forget("AAPL");
        assert!(history.points("AAPL").is_empty());
    }

    #[test]
    fn watching_subscribes_upstream_once_and_unwatch_releases() {
        let (recorder, feed, _) = recorder();
        recorder.watch("AAPL").unwrap();
        recorder.watch("AAPL").unwrap();
        assert_eq!(log(&feed), ["+AAPL"]);

        recorder.unwatch("AAPL");
        assert_eq!(log(&feed), ["+AAPL", "-AAPL"]);
    }

    #[test]
    fn unwatch_forgets_history() {
        let (recorder, _, _) = recorder();
        recorder.watch("AAPL").unwrap();
        recorder.record(&[trade("AAPL", 0, 1.0)]);
        recorder.unwatch("AAPL");
        recorder.watch("AAPL").unwrap();
        assert!(recorder.points("AAPL").is_empty());
    }

    #[tokio::test]
    async fn spawned_recorder_reads_the_broadcast() {
        let (recorder, _, trades) = recorder();
        let shutdown = CancellationToken::new();
        let tasks = TaskTracker::new();
        Arc::clone(&recorder).spawn(trades.subscribe(), shutdown.clone(), &tasks);

        trades.send(Arc::from(vec![trade("AAPL", 0, 1.0)])).unwrap();
        let mut points = Vec::new();
        for _ in 0..100 {
            points = recorder.points("AAPL");
            if !points.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(points, [(0, 1.0)]);

        shutdown.cancel();
        tasks.close();
        tokio::time::timeout(Duration::from_secs(1), tasks.wait())
            .await
            .expect("recorder berhenti saat shutdown");
    }
}
