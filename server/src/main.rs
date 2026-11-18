mod config;
mod error;
mod finnhub;
mod hub;
mod market;
mod routes;
mod state;

use std::sync::Arc;

use tracing_subscriber::EnvFilter;

use tokio::sync::broadcast;

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

    let finnhub = Arc::new(FinnhubRest::new(&config.finnhub_api_key)?);

    // Kapasitas = berapa batch yang boleh numpuk untuk client yang lambat sebelum ia
    // dianggap tertinggal. Receiver pertama dibuang: tiap client bikin miliknya sendiri.
    let (trades, _) = broadcast::channel(1024);
    let stream = FinnhubStream::spawn(&config.finnhub_api_key, trades.clone());

    let state = AppState {
        market: Arc::new(MarketCache::new(finnhub)),
        hub: Arc::new(Hub::new(Arc::new(stream), trades)),
    };
    let app = routes::router(state);

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!("listening on http://{}", config.bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
