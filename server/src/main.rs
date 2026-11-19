mod config;
mod error;
mod extract;
mod finnhub;
mod hub;
mod market;
mod routes;
mod state;
mod watchlist;

use std::{sync::Arc, time::Duration};

use anyhow::Context;
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;
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

    // Kapasitas = berapa batch yang boleh numpuk untuk client yang lambat sebelum ia
    // dianggap tertinggal. Receiver pertama dibuang: tiap client bikin miliknya sendiri.
    let (trades, _) = broadcast::channel(1024);
    let stream = FinnhubStream::spawn(&config.finnhub_api_key, trades.clone());

    let state = AppState {
        market: Arc::new(MarketCache::new(finnhub)),
        hub: Arc::new(Hub::new(Arc::new(stream), trades)),
        db,
    };
    let app = routes::router(state);

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!("listening on http://{}", config.bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
