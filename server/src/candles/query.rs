use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use moka::future::Cache;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CandleRange {
    #[serde(rename = "1d")]
    OneDay,
    #[serde(rename = "5d")]
    FiveDays,
}

impl CandleRange {
    fn session_dates(self) -> i64 {
        match self {
            Self::OneDay => 1,
            Self::FiveDays => 5,
        }
    }

    fn bucket_seconds(self) -> i32 {
        match self {
            Self::OneDay => 60,
            Self::FiveDays => 300,
        }
    }

    fn rolling_hours(self) -> i64 {
        match self {
            Self::OneDay => 24,
            Self::FiveDays => 120,
        }
    }
}

/// `[unixSeconds, open, high, low, close]` setelah di-serialize.
pub type CandleRow = (i64, f64, f64, f64, f64);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandleSeries {
    pub symbol: String,
    pub range: CandleRange,
    pub bucket_seconds: i32,
    pub candles: Vec<CandleRow>,
}

/// Baca candle tersimpan untuk `symbol` dan `range`.
///
/// Saham memakai N tanggal sesi terakhir (Senin–Jumat dengan minimal satu candle di jam
/// reguler NY `09:30..16:00`), lalu mengambil semua candle mulai tengah malam NY tanggal
/// tersebut sampai data paling baru — jadi pre-market hari ini nggak langsung menghapus sesi
/// tadi malam. Simbol berkode bursa (berisi `:`, mis. crypto 24 jam) memakai jendela geser.
pub async fn load(db: &PgPool, symbol: &str, range: CandleRange) -> sqlx::Result<CandleSeries> {
    let start_s = if symbol.contains(':') {
        let now_s = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        now_s - range.rolling_hours() * 3_600
    } else {
        sqlx::query_scalar!(
            r#"SELECT extract(epoch FROM (d::timestamp AT TIME ZONE 'America/New_York'))::bigint AS "start!"
               FROM (
                   SELECT DISTINCT (minute AT TIME ZONE 'America/New_York')::date AS d
                   FROM candles
                   WHERE symbol = $1
                     AND extract(isodow FROM minute AT TIME ZONE 'America/New_York') < 6
                     AND (minute AT TIME ZONE 'America/New_York')::time >= TIME '09:30'
                     AND (minute AT TIME ZONE 'America/New_York')::time <  TIME '16:00'
               ) dates
               ORDER BY d DESC
               OFFSET $2 LIMIT 1"#,
            symbol,
            range.session_dates() - 1,
        )
        .fetch_optional(db)
        .await?
        .unwrap_or(0)
    };

    let bucket_seconds = range.bucket_seconds();
    let rows = sqlx::query!(
        r#"SELECT extract(epoch FROM date_bin($3::int * interval '1 second', minute,
                                           TIMESTAMPTZ '2000-01-01'))::bigint AS "t!",
                  (array_agg(open ORDER BY minute))[1] AS "open!",
                  max(high) AS "high!",
                  min(low) AS "low!",
                  (array_agg(close ORDER BY minute DESC))[1] AS "close!"
           FROM candles
           WHERE symbol = $1 AND minute >= to_timestamp($2::float8)
           GROUP BY 1
           ORDER BY 1"#,
        symbol,
        start_s as f64,
        bucket_seconds,
    )
    .fetch_all(db)
    .await?;

    let candles = rows
        .into_iter()
        .map(|r| (r.t, r.open, r.high, r.low, r.close))
        .collect();

    Ok(CandleSeries {
        symbol: symbol.to_string(),
        range,
        bucket_seconds,
        candles,
    })
}

#[derive(Clone)]
pub struct CandleCache {
    cache: Cache<(String, CandleRange), Arc<CandleSeries>>,
}

impl Default for CandleCache {
    fn default() -> Self {
        Self::new()
    }
}

impl CandleCache {
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(100)
                .time_to_live(Duration::from_secs(5))
                .build(),
        }
    }

    /// Ambil seri candle lewat cache 5 detik supaya endpoint publik nggak membanjiri Postgres.
    /// Error nggak di-cache.
    pub async fn get(
        &self,
        db: &PgPool,
        symbol: &str,
        range: CandleRange,
    ) -> sqlx::Result<Arc<CandleSeries>> {
        self.cache
            .try_get_with((symbol.to_string(), range), async {
                load(db, symbol, range).await.map(Arc::new)
            })
            .await
            .map_err(|err| sqlx::Error::Protocol(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn insert_ny(db: &PgPool, symbol: &str, ny_ts: &str, o: f64, h: f64, l: f64, c: f64) {
        sqlx::query(
            "INSERT INTO candles (symbol, minute, open, high, low, close, volume, last_trade_ms)
             VALUES ($1, $2::timestamp AT TIME ZONE 'America/New_York', $3, $4, $5, $6, 1.0, 1)",
        )
        .bind(symbol)
        .bind(ny_ts)
        .bind(o)
        .bind(h)
        .bind(l)
        .bind(c)
        .execute(db)
        .await
        .unwrap();
    }

    async fn insert_ny_1(db: &PgPool, symbol: &str, ny_ts: &str) {
        insert_ny(db, symbol, ny_ts, 100.0, 101.0, 99.0, 100.5).await;
    }

    async fn ny(db: &PgPool, ny_ts: &str) -> i64 {
        sqlx::query_scalar(
            "SELECT extract(epoch FROM ($1::timestamp AT TIME ZONE 'America/New_York'))::bigint",
        )
        .bind(ny_ts)
        .fetch_one(db)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn one_day_is_the_latest_session(db: PgPool) {
        insert_ny_1(&db, "NVDA", "2026-11-20 10:00").await;
        insert_ny_1(&db, "NVDA", "2026-11-23 04:30").await;
        insert_ny_1(&db, "NVDA", "2026-11-23 10:00").await;
        insert_ny_1(&db, "NVDA", "2026-11-23 17:00").await;

        let res = load(&db, "NVDA", CandleRange::OneDay).await.unwrap();
        let times: Vec<i64> = res.candles.iter().map(|c| c.0).collect();
        assert_eq!(
            times,
            vec![
                ny(&db, "2026-11-23 04:30").await,
                ny(&db, "2026-11-23 10:00").await,
                ny(&db, "2026-11-23 17:00").await,
            ]
        );
    }

    #[sqlx::test]
    async fn premarket_keeps_last_session(db: PgPool) {
        insert_ny_1(&db, "NVDA", "2026-11-20 10:00").await;
        insert_ny_1(&db, "NVDA", "2026-11-20 17:00").await;
        insert_ny_1(&db, "NVDA", "2026-11-23 04:30").await;

        let res = load(&db, "NVDA", CandleRange::OneDay).await.unwrap();
        let times: Vec<i64> = res.candles.iter().map(|c| c.0).collect();
        assert_eq!(
            times,
            vec![
                ny(&db, "2026-11-20 10:00").await,
                ny(&db, "2026-11-20 17:00").await,
                ny(&db, "2026-11-23 04:30").await,
            ]
        );
    }

    #[sqlx::test]
    async fn first_regular_candle_moves_to_today(db: PgPool) {
        insert_ny_1(&db, "NVDA", "2026-11-20 10:00").await;
        insert_ny_1(&db, "NVDA", "2026-11-20 17:00").await;
        insert_ny_1(&db, "NVDA", "2026-11-23 04:30").await;
        insert_ny_1(&db, "NVDA", "2026-11-23 09:30").await;

        let res = load(&db, "NVDA", CandleRange::OneDay).await.unwrap();
        let times: Vec<i64> = res.candles.iter().map(|c| c.0).collect();
        assert_eq!(
            times,
            vec![
                ny(&db, "2026-11-23 04:30").await,
                ny(&db, "2026-11-23 09:30").await,
            ]
        );
    }

    #[sqlx::test]
    async fn close_at_1600_is_not_regular_hours(db: PgPool) {
        insert_ny_1(&db, "NVDA", "2026-11-20 10:00").await;
        insert_ny_1(&db, "NVDA", "2026-11-23 16:00").await;

        let res = load(&db, "NVDA", CandleRange::OneDay).await.unwrap();
        let times: Vec<i64> = res.candles.iter().map(|c| c.0).collect();
        assert_eq!(
            times,
            vec![
                ny(&db, "2026-11-20 10:00").await,
                ny(&db, "2026-11-23 16:00").await,
            ]
        );
    }

    #[sqlx::test]
    async fn weekend_print_is_kept_but_not_a_session(db: PgPool) {
        insert_ny_1(&db, "NVDA", "2026-11-20 10:00").await;
        insert_ny_1(&db, "NVDA", "2026-11-21 10:00").await;

        let res = load(&db, "NVDA", CandleRange::OneDay).await.unwrap();
        let times: Vec<i64> = res.candles.iter().map(|c| c.0).collect();
        assert_eq!(
            times,
            vec![
                ny(&db, "2026-11-20 10:00").await,
                ny(&db, "2026-11-21 10:00").await,
            ]
        );
    }

    #[sqlx::test]
    async fn five_days_skips_the_weekend(db: PgPool) {
        for day in [
            "2026-11-16 10:00",
            "2026-11-17 10:00",
            "2026-11-18 10:00",
            "2026-11-19 10:00",
            "2026-11-20 10:00",
            "2026-11-23 10:00",
        ] {
            insert_ny_1(&db, "NVDA", day).await;
        }

        let res = load(&db, "NVDA", CandleRange::FiveDays).await.unwrap();
        assert_eq!(res.candles.len(), 5);
        assert_eq!(res.candles[0].0, ny(&db, "2026-11-17 10:00").await);
    }

    #[sqlx::test]
    async fn fewer_sessions_return_everything(db: PgPool) {
        insert_ny_1(&db, "NVDA", "2026-11-20 10:00").await;
        insert_ny_1(&db, "NVDA", "2026-11-23 10:00").await;

        let res = load(&db, "NVDA", CandleRange::FiveDays).await.unwrap();
        assert_eq!(res.candles.len(), 2);
    }

    #[sqlx::test]
    async fn no_session_returns_everything(db: PgPool) {
        insert_ny_1(&db, "NVDA", "2026-11-23 05:00").await;

        let res = load(&db, "NVDA", CandleRange::OneDay).await.unwrap();
        assert_eq!(res.candles.len(), 1);
    }

    #[sqlx::test]
    async fn window_start_is_correct_across_dst(db: PgPool) {
        insert_ny_1(&db, "NVDA", "2026-10-30 10:00").await;
        insert_ny_1(&db, "NVDA", "2026-11-02 00:30").await;
        insert_ny_1(&db, "NVDA", "2026-11-02 10:00").await;

        let res = load(&db, "NVDA", CandleRange::OneDay).await.unwrap();
        assert_eq!(res.candles.len(), 2);
        assert_eq!(res.candles[0].0, ny(&db, "2026-11-02 00:30").await);
    }

    #[sqlx::test]
    async fn crypto_uses_a_rolling_window(db: PgPool) {
        sqlx::query(
            "INSERT INTO candles (symbol, minute, open, high, low, close, volume, last_trade_ms)
             VALUES
               ('BINANCE:BTCUSDT', date_trunc('minute', now() - interval '25 hours'), 100, 100, 100, 100, 1, 1),
               ('BINANCE:BTCUSDT', date_trunc('minute', now() - interval '1 hour'),   200, 200, 200, 200, 1, 2)",
        )
        .execute(&db)
        .await
        .unwrap();

        let one = load(&db, "BINANCE:BTCUSDT", CandleRange::OneDay)
            .await
            .unwrap();
        assert_eq!(one.candles.len(), 1);
        assert_eq!(one.candles[0].4, 200.0);

        let five = load(&db, "BINANCE:BTCUSDT", CandleRange::FiveDays)
            .await
            .unwrap();
        assert_eq!(five.candles.len(), 2);
    }

    #[sqlx::test]
    async fn five_day_rollup_merges_minutes(db: PgPool) {
        insert_ny(&db, "NVDA", "2026-11-23 10:00", 1.0, 5.0, 1.0, 2.0).await;
        insert_ny(&db, "NVDA", "2026-11-23 10:01", 2.0, 9.0, 0.0, 3.0).await;
        insert_ny(&db, "NVDA", "2026-11-23 10:05", 3.0, 3.0, 3.0, 3.0).await;

        let res = load(&db, "NVDA", CandleRange::FiveDays).await.unwrap();
        assert_eq!(res.bucket_seconds, 300);
        assert_eq!(res.candles.len(), 2);
        assert_eq!(
            res.candles[0],
            (ny(&db, "2026-11-23 10:00").await, 1.0, 9.0, 0.0, 3.0)
        );
    }
}
