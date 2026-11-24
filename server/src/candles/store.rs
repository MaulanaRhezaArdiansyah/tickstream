use sqlx::PgPool;

use super::PendingCandle;

/// Tulis sekumpulan candle dalam satu statement `UNNEST`.
///
/// Kalau menit yang sama sudah ada (mis. flush berikutnya setelah ada trade baru), `open`
/// tetap yang pertama, `high`/`low` melebar, `volume` dijumlahkan, dan `close` cuma diganti
/// kalau `last_trade_ms` pendatang minimal sama barunya.
pub async fn upsert(db: &PgPool, candles: &[PendingCandle]) -> sqlx::Result<()> {
    if candles.is_empty() {
        return Ok(());
    }

    let mut symbols = Vec::with_capacity(candles.len());
    let mut minutes = Vec::with_capacity(candles.len());
    let mut opens = Vec::with_capacity(candles.len());
    let mut highs = Vec::with_capacity(candles.len());
    let mut lows = Vec::with_capacity(candles.len());
    let mut closes = Vec::with_capacity(candles.len());
    let mut volumes = Vec::with_capacity(candles.len());
    let mut last_trades = Vec::with_capacity(candles.len());

    for c in candles {
        symbols.push(c.symbol.clone());
        minutes.push(c.minute_s);
        opens.push(c.open);
        highs.push(c.high);
        lows.push(c.low);
        closes.push(c.close);
        volumes.push(c.volume);
        last_trades.push(c.last_trade_ms);
    }

    sqlx::query!(
        "INSERT INTO candles (symbol, minute, open, high, low, close, volume, last_trade_ms)
         SELECT s, to_timestamp(m::float8), o, h, l, c, v, t
         FROM UNNEST($1::text[], $2::bigint[], $3::float8[], $4::float8[],
                     $5::float8[], $6::float8[], $7::float8[], $8::bigint[])
              AS input(s, m, o, h, l, c, v, t)
         ON CONFLICT (symbol, minute) DO UPDATE SET
             high = GREATEST(candles.high, EXCLUDED.high),
             low = LEAST(candles.low, EXCLUDED.low),
             volume = candles.volume + EXCLUDED.volume,
             close = CASE WHEN EXCLUDED.last_trade_ms >= candles.last_trade_ms
                          THEN EXCLUDED.close ELSE candles.close END,
             last_trade_ms = GREATEST(candles.last_trade_ms, EXCLUDED.last_trade_ms)",
        &symbols,
        &minutes,
        &opens,
        &highs,
        &lows,
        &closes,
        &volumes,
        &last_trades,
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn purge_older_than_days(db: &PgPool, days: i32) -> sqlx::Result<u64> {
    let result = sqlx::query!(
        "DELETE FROM candles WHERE minute < now() - ($1::int * interval '1 day')",
        days
    )
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        candles::{CandleBuffer, flush},
        market::Trade,
    };

    const MINUTE_S: i64 = 1_700_000_040;

    fn candle(
        symbol: &str,
        minute_s: i64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        last_trade_ms: i64,
    ) -> PendingCandle {
        PendingCandle {
            symbol: symbol.to_string(),
            minute_s,
            open,
            high,
            low,
            close,
            volume,
            last_trade_ms,
        }
    }

    async fn fetch_candle(
        db: &PgPool,
        symbol: &str,
        minute_s: i64,
    ) -> (f64, f64, f64, f64, f64, i64) {
        sqlx::query_as(
            "SELECT open, high, low, close, volume, last_trade_ms
             FROM candles
             WHERE symbol = $1 AND minute = to_timestamp($2::float8)",
        )
        .bind(symbol)
        .bind(minute_s as f64)
        .fetch_one(db)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn upsert_inserts_a_candle(db: PgPool) {
        let c = candle("NVDA", MINUTE_S, 10.0, 12.0, 9.0, 11.0, 5.0, 2000);
        upsert(&db, &[c]).await.unwrap();

        let row = fetch_candle(&db, "NVDA", MINUTE_S).await;
        assert_eq!(row, (10.0, 12.0, 9.0, 11.0, 5.0, 2000));
    }

    #[sqlx::test]
    async fn second_flush_merges(db: PgPool) {
        let first = candle("NVDA", MINUTE_S, 10.0, 12.0, 9.0, 11.0, 5.0, 2000);
        upsert(&db, &[first]).await.unwrap();

        let second = candle("NVDA", MINUTE_S, 50.0, 13.0, 8.0, 14.0, 1.0, 3000);
        upsert(&db, &[second]).await.unwrap();

        let row = fetch_candle(&db, "NVDA", MINUTE_S).await;
        assert_eq!(row, (10.0, 13.0, 8.0, 14.0, 6.0, 3000));
    }

    #[sqlx::test]
    async fn late_trade_keeps_close(db: PgPool) {
        let first = candle("NVDA", MINUTE_S, 10.0, 12.0, 9.0, 11.0, 5.0, 3000);
        upsert(&db, &[first]).await.unwrap();

        let late = candle("NVDA", MINUTE_S, 50.0, 99.0, 10.0, 99.0, 1.0, 1000);
        upsert(&db, &[late]).await.unwrap();

        let row = fetch_candle(&db, "NVDA", MINUTE_S).await;
        assert_eq!(row, (10.0, 99.0, 9.0, 11.0, 6.0, 3000));
    }

    #[sqlx::test]
    async fn purge_removes_only_old_rows(db: PgPool) {
        sqlx::query(
            "INSERT INTO candles (symbol, minute, open, high, low, close, volume, last_trade_ms)
             VALUES
               ('NVDA', now() - interval '15 days', 10, 10, 10, 10, 1, 1),
               ('NVDA', now() - interval '13 days', 20, 20, 20, 20, 1, 2)",
        )
        .execute(&db)
        .await
        .unwrap();

        let deleted = purge_older_than_days(&db, 14).await.unwrap();
        assert_eq!(deleted, 1);

        let remaining: Vec<f64> =
            sqlx::query_scalar("SELECT close FROM candles WHERE symbol = 'NVDA'")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(remaining, vec![20.0]);
    }

    #[sqlx::test]
    async fn failed_flush_keeps_candles(db: PgPool) {
        let buf = CandleBuffer::default();
        let now_ms = MINUTE_S * 1000 + 1000;
        buf.record(
            &[Trade {
                symbol: "NVDA".to_string(),
                price: 100.0,
                timestamp: now_ms,
                volume: 1.0,
            }],
            now_ms,
        );
        assert_eq!(buf.len(), 1);

        db.close().await;
        assert!(flush(&buf, &db).await.is_err());
        assert_eq!(buf.len(), 1);
    }

    #[sqlx::test]
    async fn flush_of_empty_buffer_counts_nothing(db: PgPool) {
        let m = crate::metrics::testing::capture();
        let buf = CandleBuffer::default();

        flush(&buf, &db).await.unwrap();
        assert_eq!(
            m.counter("tickstream_candle_flushes_total", &[("outcome", "ok")]),
            0
        );
        assert_eq!(
            m.counter("tickstream_candle_flushes_total", &[("outcome", "error")]),
            0
        );
    }

    #[sqlx::test]
    async fn flush_counts_outcome(db: PgPool) {
        let m = crate::metrics::testing::capture();
        let buf = CandleBuffer::default();
        let now_ms = MINUTE_S * 1000 + 1000;
        buf.record(
            &[Trade {
                symbol: "NVDA".to_string(),
                price: 100.0,
                timestamp: now_ms,
                volume: 1.0,
            }],
            now_ms,
        );

        flush(&buf, &db).await.unwrap();
        assert_eq!(
            m.counter("tickstream_candle_flushes_total", &[("outcome", "ok")]),
            1
        );
    }
}
