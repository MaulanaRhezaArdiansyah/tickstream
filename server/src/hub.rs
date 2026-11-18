//! Pembagi stream ke banyak browser.
//!
//! Upstream cuma di-subscribe ke gabungan simbol semua client. Tiap simbol punya hitungan
//! pemakai: subscribe ke upstream saat 0→1, unsubscribe saat 1→0.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;

use crate::market::{TradeBatch, TradeFeed};

/// Batas simbol berbeda yang boleh di-subscribe ke Finnhub (free tier).
pub const MAX_SYMBOLS: usize = 50;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum HubError {
    #[error("batas {MAX_SYMBOLS} simbol live sudah penuh")]
    Full,
}

pub struct Hub {
    feed: Arc<dyn TradeFeed>,
    trades: broadcast::Sender<TradeBatch>,
    /// `std::sync::Mutex`, bukan versi tokio: lock-nya cuma dipegang sebentar dan nggak
    /// pernah melewati `.await`, jadi mutex biasa lebih sederhana dan lebih cepat.
    refs: Mutex<HashMap<String, usize>>,
}

impl Hub {
    pub fn new(feed: Arc<dyn TradeFeed>, trades: broadcast::Sender<TradeBatch>) -> Self {
        Self {
            feed,
            trades,
            refs: Mutex::default(),
        }
    }

    /// Penerima trade baru untuk satu client. Isinya semua trade; client yang menyaring.
    pub fn trades(&self) -> broadcast::Receiver<TradeBatch> {
        self.trades.subscribe()
    }

    /// Daftar simbol kosong milik satu client. Semua simbolnya dilepas otomatis saat di-drop.
    pub fn subscription(self: &Arc<Self>) -> Subscription {
        Subscription {
            hub: Arc::clone(self),
            symbols: HashSet::new(),
        }
    }

    fn acquire(&self, symbol: &str) -> Result<(), HubError> {
        // `unwrap` di lock: lock cuma "poisoned" kalau thread lain panic saat memegangnya,
        // dan kode di dalam lock ini nggak bisa panic.
        let mut refs = self.refs.lock().unwrap();
        if let Some(count) = refs.get_mut(symbol) {
            *count += 1;
            return Ok(());
        }
        if refs.len() >= MAX_SYMBOLS {
            return Err(HubError::Full);
        }
        refs.insert(symbol.to_string(), 1);
        // Dipanggil di dalam lock supaya urutan subscribe/unsubscribe ke upstream sama
        // dengan urutan perubahan hitungan. Aman: `subscribe` cuma menitip ke channel.
        self.feed.subscribe(symbol);
        Ok(())
    }

    fn release(&self, symbol: &str) {
        let mut refs = self.refs.lock().unwrap();
        let Some(count) = refs.get_mut(symbol) else {
            return;
        };
        *count -= 1;
        if *count == 0 {
            refs.remove(symbol);
            self.feed.unsubscribe(symbol);
        }
    }
}

/// Simbol yang sedang dipantau satu client.
///
/// Pola RAII: pemegang guard ini nggak perlu ingat "bersih-bersih" waktu koneksi selesai.
/// Mau keluar normal, error, atau task-nya dibatalkan, `Drop` tetap jalan.
pub struct Subscription {
    hub: Arc<Hub>,
    symbols: HashSet<String>,
}

impl Subscription {
    /// Idempoten: simbol yang sudah ada nggak dihitung dua kali.
    pub fn add(&mut self, symbol: &str) -> Result<(), HubError> {
        if self.symbols.contains(symbol) {
            return Ok(());
        }
        self.hub.acquire(symbol)?;
        self.symbols.insert(symbol.to_string());
        Ok(())
    }

    pub fn remove(&mut self, symbol: &str) {
        if self.symbols.remove(symbol) {
            self.hub.release(symbol);
        }
    }

    pub fn contains(&self, symbol: &str) -> bool {
        self.symbols.contains(symbol)
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        for symbol in &self.symbols {
            self.hub.release(symbol);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn setup() -> (Arc<Hub>, Arc<RecordingFeed>) {
        let feed = Arc::new(RecordingFeed::default());
        let (trades, _) = broadcast::channel(8);
        (Arc::new(Hub::new(feed.clone(), trades)), feed)
    }

    fn log(feed: &RecordingFeed) -> Vec<String> {
        feed.0.lock().unwrap().clone()
    }

    #[test]
    fn shared_symbol_is_subscribed_once_and_released_by_last_client() {
        let (hub, feed) = setup();
        let mut a = hub.subscription();
        let mut b = hub.subscription();
        a.add("AAPL").unwrap();
        b.add("AAPL").unwrap();
        assert_eq!(log(&feed), ["+AAPL"]);

        drop(a);
        assert_eq!(log(&feed), ["+AAPL"], "b masih pakai AAPL");

        drop(b);
        assert_eq!(log(&feed), ["+AAPL", "-AAPL"]);
    }

    #[test]
    fn adding_twice_counts_once() {
        let (hub, feed) = setup();
        let mut a = hub.subscription();
        a.add("NVDA").unwrap();
        a.add("NVDA").unwrap();
        a.remove("NVDA");
        assert_eq!(log(&feed), ["+NVDA", "-NVDA"]);
        a.remove("NVDA"); // nggak ada efek, nggak bikin hitungan minus
        assert_eq!(log(&feed).len(), 2);
    }

    #[test]
    fn cap_limits_distinct_symbols_only() {
        let (hub, _feed) = setup();
        let mut a = hub.subscription();
        for i in 0..MAX_SYMBOLS {
            a.add(&format!("S{i}")).unwrap();
        }
        assert_eq!(a.add("EXTRA"), Err(HubError::Full));

        // Simbol yang sudah di-subscribe tetap boleh dipakai client lain.
        let mut b = hub.subscription();
        assert_eq!(b.add("S0"), Ok(()));

        // Setelah ada yang dilepas, slot kosong lagi.
        a.remove("S1");
        assert_eq!(b.add("EXTRA"), Ok(()));
    }
}
