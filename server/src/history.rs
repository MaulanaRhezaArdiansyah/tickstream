//! Riwayat harga singkat untuk sparkline: 30 menit terakhir per simbol.
//!
//! Trade dikelompokkan per 10 detik dan tiap kelompok cuma menyimpan harga terakhirnya.
//! Kenapa nggak simpan trade mentah? Saham ramai bisa puluhan trade per detik, saham sepi
//! cuma beberapa per menit. Dengan kelompok waktu, semua baris punya skala waktu yang sama
//! dan memorinya tetap kecil (maksimal 180 titik per simbol), berapa pun ramainya.
//!
//! Kelompok cuma dibuat kalau ada trade. Di luar jam bursa isinya jadi "30 menit terakhir
//! yang ada transaksinya", bukan garis datar.

#![cfg_attr(not(test), allow(dead_code))] // dipakai mulai Task 2 (Recorder)

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
};

use crate::market::Trade;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
