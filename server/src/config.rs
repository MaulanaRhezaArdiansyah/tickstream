use std::net::SocketAddr;

use anyhow::Context;
use secrecy::SecretString;

/// Konfigurasi server, dibaca sekali dari environment saat startup.
pub struct Config {
    pub bind_addr: SocketAddr,
    /// `SecretString` nggak bisa ke-print lewat `{:?}`, jadi key aman dari log yang nggak disengaja.
    pub finnhub_api_key: SecretString,
    /// Rahasia juga: URL-nya memuat password database.
    pub database_url: SecretString,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = std::env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .context("BIND_ADDR harus berformat host:port, mis. 127.0.0.1:8080")?;

        let finnhub_api_key: SecretString = std::env::var("FINNHUB_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
            .context("FINNHUB_API_KEY wajib diisi di server/.env")?
            .into();

        let database_url: SecretString = std::env::var("DATABASE_URL")
            .context("DATABASE_URL wajib diisi di server/.env")?
            .into();

        Ok(Self {
            bind_addr,
            finnhub_api_key,
            database_url,
        })
    }
}
