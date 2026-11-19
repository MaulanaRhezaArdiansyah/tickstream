# /metrics: kesehatan server dalam format Prometheus

Fitur 6. Desain pendek (perubahan kecil, cuma di server).

## Kenapa

Pemilik ingin menjawab "server-nya sehat nggak?" tanpa membuka log VPS:

1. Feed Finnhub masih mengalir? Berapa kali WebSocket tersambung ulang?
2. Jatah Finnhub hampir habis? Berapa panggilan REST per endpoint, dan berapa yang kena 429?
3. Cache bekerja? Berapa hit dan miss untuk quote, profil, status pasar, berita, dan daftar simbol?

Ditambah jumlah klien yang tersambung, jumlah request dan lama respons per route, dan
sejak kapan server hidup. Tujuan belajar: observability di Rust dengan format standar industri.

## Pendekatan

- **A. `/metrics` Prometheus, dikunci ADMIN_TOKEN (dipilih).** Crate `metrics` dan
  `metrics-exporter-prometheus`, plus middleware untuk request HTTP. Dibaca dengan `curl`
  dan token. Grafana Cloud gratis bisa membaca endpoint yang sama nanti, tanpa mengubah server.
- B. `/api/stats` JSON dan tampilan di web mode pemilik. Lebih enak dilihat, tapi ada UI
  yang harus dibuat dan nggak belajar format standar. Bisa nyusul kalau ternyata sering dicek.
- C. Prometheus dan Grafana di VPS. Ditolak: VPS dipakai bareng project lain, makan RAM,
  dan harus dirawat sendiri.

## Sasaran

- `GET /metrics` mengembalikan teks format Prometheus (`text/plain; version=0.0.4`).
- Hanya untuk pemilik: tanpa `Authorization: Bearer <ADMIN_TOKEN>` yang benar, balasannya 401
  (pakai extractor `Admin` yang sudah ada). Kalau token belum diatur, juga 401.
- Metrik (semua berawalan `tickstream_`):

| Metrik | Jenis | Label | Menjawab |
|---|---|---|---|
| `tickstream_ws_clients` | gauge | - | klien WebSocket yang sedang tersambung |
| `tickstream_finnhub_trades_total` | counter | - | jumlah trade yang masuk dari Finnhub |
| `tickstream_finnhub_last_trade_timestamp_seconds` | gauge | - | kapan trade terakhir masuk (unix detik); kalau jauh di belakang jam sekarang saat pasar buka, feed macet |
| `tickstream_finnhub_ws_connects_total` | counter | - | berapa kali sesi WebSocket Finnhub berhasil dibuka (lebih dari 1 = pernah reconnect) |
| `tickstream_finnhub_requests_total` | counter | `endpoint`, `outcome` | panggilan REST Finnhub; `outcome` = `ok`, `rate_limited` (429 dari Finnhub), `limited_locally` (limiter kita menolak sebelum memanggil), `error` |
| `tickstream_cache_requests_total` | counter | `cache`, `result` | `cache` = `quote`/`profile`/`market_status`/`news`/`symbols`; `result` = `hit`/`miss` |
| `tickstream_http_requests_total` | counter | `route`, `status` | request ke server per route |
| `tickstream_http_request_duration_seconds` | histogram | `route` | lama respons per route |
| `tickstream_start_time_seconds` | gauge | - | waktu server mulai (unix detik), untuk uptime |

## Batasan (anti-goal)

- **Nggak ada simbol, IP, token, atau API key di label.** Label `route` memakai pola route
  (`/api/watchlist/{symbol}`), bukan path asli; path yang nggak cocok route mana pun
  (file statis, 404) masuk satu label `other`. Ini menjaga jumlah deret tetap kecil dan nggak
  membocorkan apa pun.
- Tick per detik nggak dihitung di server: itu `rate()` dari counter trade. Lewat `curl`,
  baca dua kali dan bagi selisihnya dengan jeda waktunya. Grafana menghitungnya sendiri.
- `/health` tetap seperti sekarang (dipakai deploy dan rollback).
- nginx, Docker, dan web nggak berubah. `/metrics` lewat `location /` yang sudah ada,
  dan token yang menjaganya.
- Nggak menambah panggilan ke Finnhub.

## Yang diserahkan ke pembuat

- Versi crate dan cara memasang recorder (global di `main`; tes memakai recorder lokal atau
  handle sendiri supaya tes paralel nggak saling ganggu).
- Letak pencatatan: cache hit/miss dihitung dari apakah fungsi pengisi `try_get_with` jalan;
  outcome REST di jalur `get_json` dan limiter; klien di handler `/ws`; trade dan connect
  di actor stream.
- Bucket histogram (default exporter boleh).
- README: baris API baru dan contoh `curl` dengan token (token ditulis sebagai `$ADMIN_TOKEN`).
