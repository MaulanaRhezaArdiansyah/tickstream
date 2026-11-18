use std::net::SocketAddr;

use anyhow::Context;

/// Konfigurasi server, dibaca sekali dari environment saat startup.
pub struct Config {
    pub bind_addr: SocketAddr,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = std::env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .context("BIND_ADDR harus berformat host:port, mis. 127.0.0.1:8080")?;

        Ok(Self { bind_addr })
    }
}
