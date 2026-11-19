mod auth;
mod config;
mod error;
mod extract;
mod finnhub;
mod hub;
mod market;
mod routes;
mod state;
mod watchlist;
mod webhook;

use std::{sync::Arc, time::Duration};

use anyhow::Context;
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing_subscriber::EnvFilter;

use crate::{
    config::Config,
    finnhub::{FinnhubRest, FinnhubStream},
    hub::Hub,
    market::MarketCache,
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // .env opsional: di production variabel biasanya di-set langsung oleh environment.
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env()?;

    // Pool = beberapa koneksi Postgres yang dipakai bergantian oleh semua request.
    let db = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(config.database_url.expose_secret())
        .await
        .context("gagal konek ke Postgres (sudah `docker compose up -d`?)")?;
    // Migration ikut di-embed ke binary saat compile, lalu dijalankan tiap startup.
    // Yang sudah pernah jalan dilewati (dicatat di tabel _sqlx_migrations).
    sqlx::migrate!().run(&db).await.context("migration gagal")?;

    let finnhub = Arc::new(FinnhubRest::new(&config.finnhub_api_key)?);

    // Satu token untuk seluruh server: di-cancel sekali, semua yang memegang salinannya
    // (stream upstream, tiap koneksi /ws) melihatnya dan beres-beres sendiri.
    let shutdown = CancellationToken::new();
    let tasks = TaskTracker::new();

    // Kapasitas = berapa batch yang boleh numpuk untuk client yang lambat sebelum ia
    // dianggap tertinggal. Receiver pertama dibuang: tiap client bikin miliknya sendiri.
    let (trades, _) = broadcast::channel(1024);
    let stream = FinnhubStream::spawn(
        &config.finnhub_api_key,
        trades.clone(),
        shutdown.clone(),
        &tasks,
    );

    if config.admin_token.is_none() {
        tracing::info!("ADMIN_TOKEN kosong: watchlist cuma bisa dibaca");
    }
    if config.webhook_secret.is_none() {
        tracing::info!("FINNHUB_WEBHOOK_SECRET kosong: POST /webhook/finnhub dimatikan");
    }
    let state = AppState {
        market: Arc::new(MarketCache::new(finnhub)),
        hub: Arc::new(Hub::new(Arc::new(stream), trades)),
        db: db.clone(),
        shutdown: shutdown.clone(),
        tasks: tasks.clone(),
        webhook_secret: config.webhook_secret.map(Arc::new),
        admin_token: config.admin_token.map(Arc::new),
    };
    let app = routes::router(state);

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!("listening on http://{}", config.bind_addr);
    // Setelah sinyal datang, axum berhenti menerima koneksi baru dan menunggu request HTTP
    // yang sedang jalan selesai. Koneksi /ws urusan TaskTracker di bawah.
    let signal = shutdown.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            wait_for_signal().await;
            tracing::info!("shutdown: menutup koneksi");
            signal.cancel();
        })
        .await?;

    // close(): nggak ada task baru lagi, jadi wait() bisa selesai begitu yang ada beres.
    tasks.close();
    if tokio::time::timeout(SHUTDOWN_GRACE, tasks.wait())
        .await
        .is_err()
    {
        tracing::warn!(
            remaining = tasks.len(),
            "shutdown: sebagian task nggak selesai tepat waktu"
        );
    }
    db.close().await;
    tracing::info!("shutdown selesai");
    Ok(())
}

/// Batas waktu beres-beres. Lewat dari ini proses keluar saja, daripada menggantung.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Selesai saat Ctrl+C (semua OS) atau SIGTERM (yang dikirim `docker stop`/systemd).
async fn wait_for_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("gagal memasang handler Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("gagal memasang handler SIGTERM")
            .recv()
            .await;
    };
    // Windows nggak punya SIGTERM: cabang ini nggak pernah selesai.
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}
