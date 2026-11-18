mod config;
mod error;
mod finnhub;
mod market;
mod routes;
mod state;

use std::sync::Arc;

use tracing_subscriber::EnvFilter;

use crate::{
    config::Config,
    finnhub::{FinnhubRest, FinnhubStream},
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

    let finnhub = Arc::new(FinnhubRest::new(&config.finnhub_api_key)?);
    let state = AppState {
        market: Arc::new(MarketCache::new(finnhub)),
    };
    let app = routes::router(state);

    // SEMENTARA (M5): subscribe daftar tetap supaya tick kelihatan di log.
    // Di M6 diganti Hub yang subscribe sesuai simbol milik client.
    // BTC diperdagangkan 24 jam, jadi tetap ada tick walau bursa US tutup.
    let stream = FinnhubStream::spawn(&config.finnhub_api_key);
    for symbol in ["AAPL", "NVDA", "MSFT", "BINANCE:BTCUSDT"] {
        stream.subscribe(symbol).await;
    }

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!("listening on http://{}", config.bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
