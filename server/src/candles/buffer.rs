use std::{
    collections::{BTreeMap, btree_map::Entry},
    sync::Mutex,
};

use crate::market::Trade;

/// Batas entri di memori supaya gangguan Postgres yang lama nggak bikin RAM habis.
pub const MAX_PENDING: usize = 3_000;

const MINUTE_MS: i64 = 60_000;
const MAX_AGE_MS: i64 = 3_600_000;
const MAX_FUTURE_MS: i64 = 60_000;

#[derive(Debug, Clone, PartialEq)]
pub struct PendingCandle {
    pub symbol: String,
    /// Awal menit, Unix detik.
    pub minute_s: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub last_trade_ms: i64,
}

/// Penampung candle satu menit di memori sebelum ditulis ke Postgres tiap 5 detik.
///
/// Key `(minute_s, symbol)` membuat `BTreeMap` urut dari menit paling lama, jadi `pop_first`
/// langsung membuang menit tertua saat kapasitas `MAX_PENDING` terlampaui.
#[derive(Default)]
pub struct CandleBuffer {
    entries: Mutex<BTreeMap<(i64, String), PendingCandle>>,
}

impl CandleBuffer {
    pub fn record<'a>(&self, trades: impl IntoIterator<Item = &'a Trade>, now_ms: i64) {
        let min_ts = now_ms.saturating_sub(MAX_AGE_MS);
        let max_ts = now_ms.saturating_add(MAX_FUTURE_MS);

        let mut entries = self.entries.lock().unwrap();
        for trade in trades {
            if !trade.price.is_finite() || trade.price <= 0.0 {
                continue;
            }
            if trade.timestamp < min_ts || trade.timestamp > max_ts {
                continue;
            }

            let minute_s = trade.timestamp.div_euclid(MINUTE_MS) * 60;
            let incoming = PendingCandle {
                symbol: trade.symbol.clone(),
                minute_s,
                open: trade.price,
                high: trade.price,
                low: trade.price,
                close: trade.price,
                volume: trade.volume,
                last_trade_ms: trade.timestamp,
            };

            match entries.entry((minute_s, trade.symbol.clone())) {
                Entry::Vacant(slot) => {
                    slot.insert(incoming);
                }
                Entry::Occupied(mut slot) => {
                    let earlier = slot.get().clone();
                    slot.insert(merge(earlier, incoming));
                }
            }
        }
        enforce_cap(&mut entries);
    }

    /// Ambil semua isi buffer dan kosongkan.
    pub fn take(&self) -> Vec<PendingCandle> {
        std::mem::take(&mut *self.entries.lock().unwrap())
            .into_values()
            .collect()
    }

    /// Kembalikan hasil `take` yang gagal ditulis. Yang dikembalikan dianggap LEBIH DULU.
    pub fn restore(&self, candles: Vec<PendingCandle>) {
        if candles.is_empty() {
            return;
        }
        let mut entries = self.entries.lock().unwrap();
        for earlier in candles {
            match entries.entry((earlier.minute_s, earlier.symbol.clone())) {
                Entry::Vacant(slot) => {
                    slot.insert(earlier);
                }
                Entry::Occupied(mut slot) => {
                    let later = slot.get().clone();
                    slot.insert(merge(earlier, later));
                }
            }
        }
        enforce_cap(&mut entries);
    }

    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Gabungkan dua candle pada `(minute_s, symbol)` yang sama. `earlier` menjaga `open`,
/// sementara `close` diambil dari trade dengan timestamp paling baru (seri: yang datang belakangan).
fn merge(earlier: PendingCandle, later: PendingCandle) -> PendingCandle {
    let (close, last_trade_ms) = if later.last_trade_ms >= earlier.last_trade_ms {
        (later.close, later.last_trade_ms)
    } else {
        (earlier.close, earlier.last_trade_ms)
    };
    PendingCandle {
        symbol: earlier.symbol,
        minute_s: earlier.minute_s,
        open: earlier.open,
        high: earlier.high.max(later.high),
        low: earlier.low.min(later.low),
        close,
        volume: earlier.volume + later.volume,
        last_trade_ms,
    }
}

fn enforce_cap(entries: &mut BTreeMap<(i64, String), PendingCandle>) {
    let mut dropped = 0usize;
    while entries.len() > MAX_PENDING {
        entries.pop_first();
        dropped += 1;
    }
    if dropped > 0 {
        tracing::warn!(
            dropped,
            cap = MAX_PENDING,
            "buffer candle penuh, menit terlama dibuang"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: i64 = 1_699_999_980_000;
    const NOW: i64 = M + 30_000;

    fn trade(symbol: &str, timestamp: i64, price: f64, volume: f64) -> Trade {
        Trade {
            symbol: symbol.to_string(),
            price,
            timestamp,
            volume,
        }
    }

    #[test]
    fn builds_ohlc_within_a_minute() {
        let buf = CandleBuffer::default();
        let trades = [
            trade("NVDA", M, 10.0, 1.0),
            trade("NVDA", M + 1_000, 12.0, 2.0),
            trade("NVDA", M + 2_000, 9.0, 3.0),
            trade("NVDA", M + 3_000, 11.0, 4.0),
        ];
        buf.record(&trades, NOW);

        let candles = buf.take();
        assert_eq!(candles.len(), 1);
        assert_eq!(
            candles[0],
            PendingCandle {
                symbol: "NVDA".to_string(),
                minute_s: M / 1000,
                open: 10.0,
                high: 12.0,
                low: 9.0,
                close: 11.0,
                volume: 10.0,
                last_trade_ms: M + 3_000,
            }
        );
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn next_minute_starts_a_new_candle() {
        let buf = CandleBuffer::default();
        let trades = [
            trade("NVDA", M + 60_000, 15.0, 1.0),
            trade("NVDA", M, 10.0, 1.0),
        ];
        buf.record(&trades, NOW + 60_000);

        let candles = buf.take();
        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].minute_s, M / 1000);
        assert_eq!(candles[1].minute_s, (M + 60_000) / 1000);
    }

    #[test]
    fn out_of_order_trade_does_not_change_close() {
        let buf = CandleBuffer::default();
        let trades = [
            trade("NVDA", M + 5_000, 20.0, 1.0),
            trade("NVDA", M + 1_000, 30.0, 2.0),
        ];
        buf.record(&trades, NOW);

        let candles = buf.take();
        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].open, 20.0);
        assert_eq!(candles[0].high, 30.0);
        assert_eq!(candles[0].low, 20.0);
        assert_eq!(candles[0].close, 20.0);
        assert_eq!(candles[0].volume, 3.0);
        assert_eq!(candles[0].last_trade_ms, M + 5_000);
    }

    #[test]
    fn rejects_bad_trades() {
        let buf = CandleBuffer::default();
        let trades = [
            trade("NVDA", NOW, 0.0, 1.0),
            trade("NVDA", NOW, -1.0, 1.0),
            trade("NVDA", NOW, f64::NAN, 1.0),
            trade("NVDA", NOW, f64::INFINITY, 1.0),
            trade("NVDA", NOW - 3_600_001, 10.0, 1.0),
            trade("NVDA", NOW + 60_001, 10.0, 1.0),
        ];
        buf.record(&trades, NOW);
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn accepts_zero_volume_and_boundary_times() {
        let buf = CandleBuffer::default();
        let trades = [
            trade("NVDA", NOW - 3_600_000, 10.0, 0.0),
            trade("NVDA", NOW + 60_000, 11.0, 0.0),
        ];
        buf.record(&trades, NOW);
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn restore_merges_with_newer_trades() {
        let buf = CandleBuffer::default();
        buf.record(&[trade("NVDA", M, 10.0, 1.0)], NOW);
        let taken = buf.take();
        buf.record(&[trade("NVDA", M + 1_000, 20.0, 1.0)], NOW);
        buf.restore(taken);

        let candles = buf.take();
        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].open, 10.0);
        assert_eq!(candles[0].high, 20.0);
        assert_eq!(candles[0].low, 10.0);
        assert_eq!(candles[0].close, 20.0);
        assert_eq!(candles[0].volume, 2.0);
    }

    #[test]
    fn cap_drops_the_oldest_minutes() {
        let buf = CandleBuffer::default();
        let candles: Vec<PendingCandle> = (0..=MAX_PENDING)
            .map(|i| PendingCandle {
                symbol: "NVDA".to_string(),
                minute_s: (M / 1000) + (i as i64) * 60,
                open: 10.0,
                high: 10.0,
                low: 10.0,
                close: 10.0,
                volume: 1.0,
                last_trade_ms: M + (i as i64) * 60_000,
            })
            .collect();
        buf.restore(candles);

        assert_eq!(buf.len(), MAX_PENDING);
        let kept = buf.take();
        assert_eq!(kept.first().unwrap().minute_s, (M / 1000) + 60);
    }
}
