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
docker compose up -d          # PostgreSQL di localhost:5433

cd server
cp .env.example .env          # isi FINNHUB_API_KEY
cargo run                     # http://localhost:8080
```

Cek: `curl http://localhost:8080/health` → `ok`.
Endpoint: `/api/quote?symbol=NVDA`, `/api/profile?symbol=NVDA`, `/api/market-status`,
`GET/POST /api/watchlist` (body `{"symbol":"AMD"}`), `DELETE /api/watchlist/{symbol}`,
WebSocket `/ws` (kirim `{"type":"subscribe","symbol":"AAPL"}`). Semua error berbentuk `{"error":"..."}`.

Webhook Finnhub: isi `FINNHUB_WEBHOOK_SECRET` di `.env`, lalu daftarkan
`https://<host-publik>/webhook/finnhub` di dashboard Finnhub. Event disimpan mentah ke tabel
`webhook_events` (JSONB). Finnhub harus bisa menjangkau server; untuk dev lokal butuh tunnel.

Catatan: Finnhub free cuma mengizinkan satu koneksi WebSocket per API key. Jangan jalankan
dua server dengan key yang sama; koneksinya bakal saling memutus. Server baru membuka koneksi
itu begitu ada browser yang subscribe, dan menutupnya dengan sopan saat Ctrl+C.

### Test & cek

```sh
cd server
cargo test                    # butuh Postgres jalan: tiap test route dapat database sendiri
cargo clippy --all-targets -- -D warnings
```

Ubah query `sqlx::query!`? Jalankan `cargo sqlx prepare` lalu commit `server/.sqlx/`, supaya
build tanpa database (`SQLX_OFFLINE=true`) tetap jalan.

### Web (React + Vite)

```sh
cd web
npm install
npm run dev                   # http://localhost:5173, /api diteruskan ke server :8080
```

Server di port lain? `API_TARGET=http://127.0.0.1:8090 npm run dev`.
