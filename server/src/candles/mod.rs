pub mod buffer;
pub mod store;

#[allow(unused_imports)]
pub use buffer::{CandleBuffer, MAX_PENDING, PendingCandle};

use std::{sync::Arc, time::Duration};

use sqlx::PgPool;
use tokio::time::MissedTickBehavior;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const PURGE_INTERVAL: Duration = Duration::from_secs(60 * 60);
const RETENTION_DAYS: i32 = 14;
/// Harus muat di dalam `SHUTDOWN_GRACE` (5 detik) di `main.rs`.
const FINAL_FLUSH_TIMEOUT: Duration = Duration::from_secs(3);

/// Ambil isi buffer dan tulis ke Postgres. Buffer kosong nggak dicatat ke metrik.
/// Kalau penulisan gagal, candle dikembalikan ke buffer supaya nggak hilang.
pub async fn flush(buffer: &CandleBuffer, db: &PgPool) -> sqlx::Result<()> {
    let batch = buffer.take();
    if batch.is_empty() {
        return Ok(());
    }
    match store::upsert(db, &batch).await {
        Ok(()) => {
            crate::metrics::candle_flush(true);
            Ok(())
        }
        Err(err) => {
            buffer.restore(batch);
            crate::metrics::candle_flush(false);
            Err(err)
        }
    }
}

/// Jalankan task background untuk flush tiap 5 detik dan bersih-bersih candle > 14 hari tiap jam.
pub fn spawn_flusher(
    buffer: Arc<CandleBuffer>,
    db: PgPool,
    shutdown: CancellationToken,
    tasks: &TaskTracker,
) {
    tasks.spawn(async move {
        let mut flush_tick = tokio::time::interval(FLUSH_INTERVAL);
        flush_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        // Tick pertama langsung jalan saat startup supaya data lama cepat bersih.
        let mut purge_tick = tokio::time::interval(PURGE_INTERVAL);
        purge_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                () = shutdown.cancelled() => {
                    match tokio::time::timeout(FINAL_FLUSH_TIMEOUT, flush(&buffer, &db)).await {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => {
                            tracing::warn!(error = %err, "gagal flush candle saat shutdown");
                        }
                        Err(_) => {
                            tracing::warn!("timeout flush candle saat shutdown");
                        }
                    }
                    break;
                }
                _ = flush_tick.tick() => {
                    if let Err(err) = flush(&buffer, &db).await {
                        tracing::warn!(error = %err, "gagal menulis candle ke Postgres");
                    }
                }
                _ = purge_tick.tick() => {
                    if let Err(err) = store::purge_older_than_days(&db, RETENTION_DAYS).await {
                        tracing::warn!(error = %err, "gagal menghapus candle lama");
                    }
                }
            }
        }
    });
}
