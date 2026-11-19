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
    /// `None` = webhook dimatikan.
    pub webhook_secret: Option<SecretString>,
    /// `None` = nggak ada pemilik, watchlist cuma bisa dibaca.
    pub admin_token: Option<SecretString>,
}

/// Token pendek bisa ditebak; `openssl rand -hex 32` menghasilkan 64 karakter.
const MIN_ADMIN_TOKEN_LEN: usize = 32;

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

        let webhook_secret = std::env::var("FINNHUB_WEBHOOK_SECRET")
            .ok()
            .filter(|secret| !secret.trim().is_empty())
            .map(SecretString::from);

        let admin_token = match std::env::var("ADMIN_TOKEN") {
            Ok(token) if token.trim().is_empty() => None,
            Ok(token) if token.len() < MIN_ADMIN_TOKEN_LEN => anyhow::bail!(
                "ADMIN_TOKEN minimal {MIN_ADMIN_TOKEN_LEN} karakter (buat dengan `openssl rand -hex 32`)"
            ),
            Ok(token) => Some(SecretString::from(token)),
            Err(_) => None,
        };

        Ok(Self {
            bind_addr,
            finnhub_api_key,
            database_url,
            webhook_secret,
            admin_token,
        })
    }
}
