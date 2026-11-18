# US Stock Live

Dashboard saham US real-time. Port dari PoC vanilla JS (`us-stock-dashboard.html`) ke:

- `server/` — Rust + Axum + Tokio. Proxy ke Finnhub; API key cuma ada di server.
- `web/` — React + TypeScript + Vite.

## Prasyarat

- Rust (stable) — `rustup`
- Node.js 20+
- Docker (untuk PostgreSQL)

## Menjalankan (dev)

```sh
docker compose up -d          # PostgreSQL di localhost:5432

cd server
cp .env.example .env
cargo run                     # http://localhost:8080
```

Cek: `curl http://localhost:8080/health` → `ok`.
