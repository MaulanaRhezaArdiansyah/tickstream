# Intraday Chart Implementation Plan

> **For agentic workers:** Execute this plan task by task, in order. If the
> superpowers skills are available, use `superpowers:executing-plans`. Steps use
> checkbox (`- [ ]`) syntax for tracking. Tick them as you go.

**Goal:** Record one-minute candles for every watchlist symbol in Postgres and show them as a live `1H`/`5H` candlestick chart in the existing side panel.

**Architecture:** `Recorder` feeds trades into an in-memory `CandleBuffer`. A background task writes it to a `candles` table every 5 seconds. `GET /api/candles` reads the table through a 5-second cache, and the web panel draws the result with `lightweight-charts`, moving the newest candle from existing WebSocket ticks.

**Tech Stack:** Rust, Axum 0.8, Tokio, SQLx 0.9 (Postgres), moka, metrics. React 19, TypeScript, Vite, Vitest, `lightweight-charts` 5.

**Spec:** `docs/superpowers/specs/2026-11-24-intraday-chart-design.md`. Read it fully before Task 1.

## Hard Rules

Breaking one of these can damage production or leak a secret. If a task seems to need
one, **stop and report instead**.

1. **Never run the server binary** (`cargo run`, `target/*/tickstream*`, `docker compose up` of the app). The free Finnhub plan allows one WebSocket per key, so a local server kicks production off its feed. `cargo test` is safe because tests use a fake feed.
2. **Never read, print, copy or commit `server/.env` or any `.env`.** Use the dev database URL given below.
3. **Never call Finnhub** from code or the shell. Never use premium endpoints: `/stock/candle`, `/stock/profile` (v1), WebSocket News & Press Releases, news sentiment.
4. **Do not push, open a PR, merge, or touch `main`.** Merging to `main` deploys to production. Commit only on branch `intraday-chart`, then stop and report. Claude reviews the branch next.
5. **Do not SSH to the VPS** or change anything in `deploy/` or `.github/`.
6. Do not widen scope: nothing from the spec's Non-goals, no refactors of unrelated code.

## Environment

The repo lives in WSL Ubuntu at `/home/astech/projects/us-stock`. Do not use the old copy at `C:\Users\Astech\Projects\us-stock`. From a Windows shell, run commands like this:

```bash
MSYS_NO_PATHCONV=1 wsl.exe -d Ubuntu -u astech -- bash -c ". ~/.cargo/env; cd /home/astech/projects/us-stock && <command>"
```

Setup, once, from the repo root:

```bash
git switch intraday-chart
docker compose up -d db     # dev Postgres only, on 127.0.0.1:5433
export DATABASE_URL=postgres://usstock:usstock@127.0.0.1:5433/usstock   # dev-only credentials from docker-compose.yml
```

`DATABASE_URL` must be set in every shell that runs `cargo`. The commands in this plan assume it.

| Purpose | Command, from `server/` unless noted |
|---|---|
| Apply migrations to dev DB | `cargo sqlx migrate run` |
| Refresh the `query!` cache (after adding or changing a query) | `cargo sqlx prepare -- --all-targets`, then `git add .sqlx` |
| Rust checks | `cargo fmt --check && SQLX_OFFLINE=true cargo clippy --all-targets --locked -- -D warnings && cargo test --locked` |
| Web checks, from `web/` | `npm run lint && npm test && npm run build` |

## Global Constraints

- `sqlx` has no `chrono`/`time` feature, and none may be added. Convert in SQL: `to_timestamp(...)` on write, `extract(epoch FROM ...)::bigint` on read.
- Code identifiers and commit messages in English. Code comments and UI text in Indonesian. Comments explain *why*; the owner is learning Rust and React.
- Commit messages: plain imperative sentence, no `feat:` prefix, e.g. `Record one-minute candles in memory`.
- Stored history: 14 days. Flush: every 5 seconds. Buffer cap: 3,000 candles. Final flush timeout: 3 seconds. API cache: 5 seconds, 100 entries.
- Trade accepted only if price is finite and `> 0`, and timestamp is within `[now - 1 hour, now + 1 minute]`. Zero volume is accepted.
- Session date: New York date, Monday to Friday, with a candle at `09:30 <= local time < 16:00`.
- A symbol containing `:` is 24-hour (crypto): rolling 24h / 120h windows.
- Chart colors: up `#52c48c`, down `#f2705f`. Axis times in WIB (`Asia/Jakarta`). Keep the TradingView attribution logo.
- UI copy, exact:
  - Empty: `Belum ada data grafik. Data mulai terkumpul saat ada transaksi.`
  - Error: `Grafik nggak bisa dimuat.`, button `Coba lagi`
  - Always-visible note: `Dari transaksi yang diterima feed gratis, bisa beda dari angka resmi.`

## Review Focus

Each has a test in the owning task.

1. A live tick older than the chart's last bar (Finnhub delivers out of order): ignored, chart does not throw. Task 4.
2. The owner clicks quickly between symbols or ranges: a slow earlier response must never overwrite the current chart. Task 4.
3. Postgres is down during a flush: candles stay in the buffer and nothing panics. Task 2.
4. A crypto symbol with `:` passes through URL encoding and the 24-hour window. Tasks 3 and 4.
5. The panel closes while a request is in flight or ticks are arriving: chart removed once, no call on a removed chart. Task 5.

---

### Task 1: Candle table and in-memory buffer

**Files:**
- Create: `server/migrations/20261124000000_create_candles.sql` (exact SQL in the spec's "Table" section)
- Create: `server/src/candles/mod.rs`, `server/src/candles/buffer.rs`
- Modify: `server/src/main.rs` (`mod candles;`), `server/src/history.rs`

**Interfaces:**
- Produces, in `candles::buffer`, re-exported from `candles`:

```rust
pub const MAX_PENDING: usize = 3_000;

#[derive(Debug, Clone, PartialEq)]
pub struct PendingCandle {
    pub symbol: String,
    /// Awal menit, Unix detik.
    pub minute_s: i64,
    pub open: f64, pub high: f64, pub low: f64, pub close: f64, pub volume: f64,
    pub last_trade_ms: i64,
}

#[derive(Default)]
pub struct CandleBuffer { /* Mutex<BTreeMap<(i64, String), PendingCandle>> */ }

impl CandleBuffer {
    pub fn record<'a>(&self, trades: impl IntoIterator<Item = &'a Trade>, now_ms: i64);
    /// Ambil semua isi buffer dan kosongkan.
    pub fn take(&self) -> Vec<PendingCandle>;
    /// Kembalikan hasil `take` yang gagal ditulis. Yang dikembalikan dianggap LEBIH DULU.
    pub fn restore(&self, candles: Vec<PendingCandle>);
    pub fn len(&self) -> usize;
}
```

- Produces on `Recorder`: `pub fn candle_buffer(&self) -> Arc<CandleBuffer>`. `Recorder::new` keeps its signature.

- [ ] **Step 1: Write the failing tests in `buffer.rs`**

Use `const M: i64 = 1_699_999_980_000` (a minute start in ms), `const NOW: i64 = M + 30_000`, and a local `trade(symbol, ms, price, volume)` helper. Test names and assertions:

```rust
#[test] fn builds_ohlc_within_a_minute() // M:10 v1, M+1s:12 v2, M+2s:9 v3, M+3s:11 v4
// → one candle, minute_s == M/1000, o=10 h=12 l=9 c=11 volume=10, last_trade_ms == M+3_000
#[test] fn next_minute_starts_a_new_candle() // M and M+60_000 → 2 candles, sorted oldest first
#[test] fn out_of_order_trade_does_not_change_close() // M+5s:20 then M+1s:30 → close 20, high 30
#[test] fn rejects_bad_trades()
// price 0, -1, NaN, INFINITY; ms = NOW-3_600_001; ms = NOW+60_001 → len() == 0
#[test] fn accepts_zero_volume_and_boundary_times()
// v=0; ms = NOW-3_600_000; ms = NOW+60_000 → all recorded
#[test] fn restore_merges_with_newer_trades()
// record M:10, take; record M+1s:20; restore(taken)
// → one candle: open 10, close 20, high 20, low 10, volume 2
#[test] fn cap_drops_the_oldest_minutes()
// 3_001 distinct minutes via restore → len()==3_000, the oldest minute is gone
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test candles::buffer`
Expected: compile error or FAIL, because the buffer is not implemented.

- [ ] **Step 3: Implement `CandleBuffer` and wire it into `Recorder`**

Key the map by `(minute_s, symbol)` so `pop_first` drops the oldest minute when the cap is exceeded, in both `record` and `restore`. Use one private `merge(earlier, later)` for both paths: open from `earlier`, widen high/low, add volume, and take close and `last_trade_ms` from `later` when `later.last_trade_ms >= earlier.last_trade_ms`. Never hold the lock across `.await`.

In `Recorder::record`, while the subscription lock is held, pass the **same filtered trades** to both `history.record` and `candles.record`, with the current time from `SystemTime`. Existing tests use timestamp 0: the buffer rejects those and `History` must behave exactly as before.

- [ ] **Step 4: Add a Recorder test in `history.rs`**

```rust
#[test] fn recorder_buffers_candles_for_watched_symbols_only()
// watch AAPL; record current-time trades for AAPL and NVDA
// → candle_buffer().take() has exactly one candle, symbol == "AAPL"
```

- [ ] **Step 5: Run tests, apply the migration, and commit**

Run: `cargo test candles:: && cargo test history:: && cargo sqlx migrate run`
Expected: all PASS, migration applied.

```bash
git add server/migrations server/src/candles server/src/main.rs server/src/history.rs
git commit -m "Record one-minute candles in memory"
```

---

### Task 2: Flush to Postgres, retention, metric

**Files:**
- Create: `server/src/candles/store.rs`
- Modify: `server/src/candles/mod.rs`, `server/src/metrics.rs`, `server/src/main.rs`, `server/.sqlx/`

**Interfaces:**
- Consumes: `CandleBuffer`, `PendingCandle`, `Recorder::candle_buffer`.
- Produces:

```rust
// store.rs
pub async fn upsert(db: &PgPool, candles: &[PendingCandle]) -> sqlx::Result<()>;
pub async fn purge_older_than_days(db: &PgPool, days: i32) -> sqlx::Result<u64>;
// mod.rs
pub async fn flush(buffer: &CandleBuffer, db: &PgPool) -> sqlx::Result<()>;
pub fn spawn_flusher(buffer: Arc<CandleBuffer>, db: PgPool, shutdown: CancellationToken, tasks: &TaskTracker);
// metrics.rs
pub fn candle_flush(ok: bool); // tickstream_candle_flushes_total{outcome="ok"|"error"}
```

- [ ] **Step 1: Write the failing `#[sqlx::test]` tests in `store.rs`**

```rust
#[sqlx::test] async fn upsert_inserts_a_candle(db: PgPool)
#[sqlx::test] async fn second_flush_merges(db: PgPool)
// stored o10 h12 l9 c11 v5 last=2000; incoming o50 h13 l8 c14 v1 last=3000
// → o10 h13 l8 c14 v6 last=3000
#[sqlx::test] async fn late_trade_keeps_close(db: PgPool)
// stored c11 last=3000; incoming c99 h99 v1 last=1000
// → close 11, last 3000, high 99, volume adds
#[sqlx::test] async fn purge_removes_only_old_rows(db: PgPool)
// rows at now()-15 days and now()-13 days; purge 14 → returns 1, the 13-day row remains
#[sqlx::test] async fn failed_flush_keeps_candles(db: PgPool)
// record one candle; db.close().await; flush(...) is Err; buffer.len() == 1
#[sqlx::test] async fn flush_of_empty_buffer_counts_nothing(db: PgPool)
// with metrics::testing::capture(): Ok, and both outcome counters stay 0
#[sqlx::test] async fn flush_counts_outcome(db: PgPool)
// one successful flush → counter tickstream_candle_flushes_total{outcome="ok"} == 1
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test candles::store`
Expected: compile error or FAIL.

- [ ] **Step 3: Implement `upsert`, `purge_older_than_days`, `flush`, and the metric**

`upsert` is one statement, using `query!`:

```sql
INSERT INTO candles (symbol, minute, open, high, low, close, volume, last_trade_ms)
SELECT s, to_timestamp(m::float8), o, h, l, c, v, t
FROM UNNEST($1::text[], $2::bigint[], $3::float8[], $4::float8[],
            $5::float8[], $6::float8[], $7::float8[], $8::bigint[])
     AS input(s, m, o, h, l, c, v, t)
ON CONFLICT (symbol, minute) DO UPDATE SET
    high = GREATEST(candles.high, EXCLUDED.high),
    low = LEAST(candles.low, EXCLUDED.low),
    volume = candles.volume + EXCLUDED.volume,
    close = CASE WHEN EXCLUDED.last_trade_ms >= candles.last_trade_ms
                 THEN EXCLUDED.close ELSE candles.close END,
    last_trade_ms = GREATEST(candles.last_trade_ms, EXCLUDED.last_trade_ms)
```

`flush`: `take()`, return `Ok` with no metric when empty. On error call `restore`, count `error`, return the error. On success count `ok`. Add the metric to `describe()`.

- [ ] **Step 4: Implement `spawn_flusher` and start it in `main.rs`**

One task on the `TaskTracker`, in a `tokio::select!` loop with `shutdown`:
- a 5-second interval with `MissedTickBehavior::Skip` calls `flush` and logs a `tracing::warn!` on error;
- a 1-hour interval (first tick fires at startup) calls `purge_older_than_days(db, 14)` and logs a warning on error;
- on shutdown, one last `flush` inside `tokio::time::timeout(3 s)`, then exit.

In `main.rs`, start it right after `Arc::clone(&recorder).spawn(...)`.

- [ ] **Step 5: Refresh the query cache, verify, and commit**

Run: `cargo sqlx prepare -- --all-targets && cargo fmt && cargo test candles::`
Expected: all PASS.

```bash
git add server/src server/.sqlx
git commit -m "Write candles to Postgres every five seconds"
```

---

### Task 3: `GET /api/candles`

**Files:**
- Create: `server/src/candles/query.rs`
- Modify: `server/src/candles/mod.rs`, `server/src/state.rs`, `server/src/main.rs`, `server/src/routes/market.rs`, `server/src/routes/mod.rs`, `server/src/routes/tests.rs`, `server/.sqlx/`

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CandleRange {
    #[serde(rename = "1d")] OneDay,
    #[serde(rename = "5d")] FiveDays,
}
// session_dates: 1 | 5; bucket_seconds: 60 | 300; rolling_hours: 24 | 120

/// `[unixSeconds, open, high, low, close]` setelah di-serialize.
pub type CandleRow = (i64, f64, f64, f64, f64);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandleSeries {
    pub symbol: String,
    pub range: CandleRange,
    pub bucket_seconds: i32,
    pub candles: Vec<CandleRow>,
}

pub async fn load(db: &PgPool, symbol: &str, range: CandleRange) -> sqlx::Result<CandleSeries>;

#[derive(Clone)]
pub struct CandleCache { /* moka::future::Cache<(String, CandleRange), Arc<CandleSeries>> */ }
impl CandleCache {
    pub fn new() -> Self; // 5 s TTL, 100 entries
    pub async fn get(&self, db: &PgPool, symbol: &str, range: CandleRange)
        -> sqlx::Result<Arc<CandleSeries>>;
}
```

- `AppState` gains `pub candles: CandleCache`.
- Route: `GET /api/candles`, handler `market::candles`.

- [ ] **Step 1: Write the failing `#[sqlx::test]` tests in `query.rs`**

Use a helper that inserts a candle at New York local time with
`($2::timestamp AT TIME ZONE 'America/New_York')`, and a helper `ny(db, "...")` that returns that instant as Unix seconds. Calendar: 2026-11-20 is a Friday, 21 Saturday, 23 Monday, 24 Tuesday. US DST ended on Sunday 2026-11-01.

```rust
#[sqlx::test] async fn one_day_is_the_latest_session(db: PgPool)
// NVDA at Fri 10:00 and Mon 04:30, 10:00, 17:00 → exactly the three Monday candles
#[sqlx::test] async fn premarket_keeps_last_session(db: PgPool)
// Fri 10:00, Fri 17:00, Mon 04:30 (no Monday regular candle) → all three
#[sqlx::test] async fn first_regular_candle_moves_to_today(db: PgPool)
// previous data + Mon 09:30 → only Monday's candles (04:30, 09:30)
#[sqlx::test] async fn close_at_1600_is_not_regular_hours(db: PgPool)
// Fri 10:00, Mon 16:00 → both returned, because Monday is not a session date
#[sqlx::test] async fn weekend_print_is_kept_but_not_a_session(db: PgPool)
// Fri 10:00, Sat 10:00 → both returned
#[sqlx::test] async fn five_days_skips_the_weekend(db: PgPool)
// regular candles on Nov 16,17,18,19,20,23 → first bucket is on Nov 17; Nov 16 is gone
#[sqlx::test] async fn fewer_sessions_return_everything(db: PgPool)
// two session dates and range 5d → all candles
#[sqlx::test] async fn no_session_returns_everything(db: PgPool)
// only a 05:00 pre-market candle, range 1d → that candle
#[sqlx::test] async fn window_start_is_correct_across_dst(db: PgPool)
// Fri 2026-10-30 10:00, Mon 2026-11-02 00:30 and 10:00 (00:30 is included, the window opens at midnight)
// range 1d → first candle time == ny("2026-11-02 00:30")
#[sqlx::test] async fn crypto_uses_a_rolling_window(db: PgPool)
// BINANCE:BTCUSDT at now()-25h and now()-1h: 1d → one candle; 5d → two
#[sqlx::test] async fn five_day_rollup_merges_minutes(db: PgPool)
// 10:00 o1 h5 l1 c2, 10:01 o2 h9 l0 c3, 10:05 o3 h3 l3 c3
// → 2 rows, bucket_seconds 300, first (o1 h9 l0 c3)
```

- [ ] **Step 2: Add failing route tests to `routes/tests.rs`**

Add `candles: CandleCache::new()` to the test `AppState`.

```rust
#[sqlx::test] async fn candles_returns_the_series(db: PgPool)
// insert an NVDA candle; GET /api/candles?symbol=nvda&range=1d → 200
// body symbol "NVDA", range "1d", bucketSeconds 60, candles[0] has 5 elements
#[sqlx::test] async fn candles_empty_for_watched_symbol(db: PgPool)   // 200, candles == []
#[sqlx::test] async fn candles_rejects_bad_range(db: PgPool)          // range=1w → 400; no range → 400
#[sqlx::test] async fn candles_rejects_unwatched_symbol(db: PgPool)   // symbol=AMD → 404
#[sqlx::test] async fn candles_accepts_encoded_crypto_symbol(db: PgPool)
// symbol=BINANCE%3ABTCUSDT&range=5d → 200, symbol "BINANCE:BTCUSDT"
```

- [ ] **Step 3: Run them and confirm they fail**

Run: `cargo test candles`
Expected: compile error or FAIL.

- [ ] **Step 4: Implement `load`, `CandleCache`, and the route**

`load` makes two queries. First the window start in Unix seconds. For a symbol containing `:` it is `now - rolling_hours`. Otherwise run this, with `$2 = session_dates - 1`, and use `0` (everything) when no row is returned:

```sql
SELECT extract(epoch FROM (d::timestamp AT TIME ZONE 'America/New_York'))::bigint AS "start!"
FROM (
    SELECT DISTINCT (minute AT TIME ZONE 'America/New_York')::date AS d
    FROM candles
    WHERE symbol = $1
      AND extract(isodow FROM minute AT TIME ZONE 'America/New_York') < 6
      AND (minute AT TIME ZONE 'America/New_York')::time >= TIME '09:30'
      AND (minute AT TIME ZONE 'America/New_York')::time <  TIME '16:00'
) dates
ORDER BY d DESC
OFFSET $2 LIMIT 1
```

Then the candles:

```sql
SELECT extract(epoch FROM date_bin($3::int * interval '1 second', minute,
                                   TIMESTAMPTZ '2000-01-01'))::bigint AS "t!",
       (array_agg(open ORDER BY minute))[1] AS "open!",
       max(high) AS "high!",
       min(low) AS "low!",
       (array_agg(close ORDER BY minute DESC))[1] AS "close!"
FROM candles
WHERE symbol = $1 AND minute >= to_timestamp($2::float8)
GROUP BY 1
ORDER BY 1
```

`CandleCache::get` uses `try_get_with`, as `market/cache.rs` does. It returns `Arc<sqlx::Error>` on failure, which is not `Clone`-unwrappable, so map it to `sqlx::Error::Protocol(err.to_string())`. Errors are not cached.

The handler takes `AppQuery<CandleQuery>` (`symbol: String`, `range: CandleRange`), calls the existing `watched_symbol`, and returns `Json<Arc<CandleSeries>>`. Register the route beside `/api/news`. Create the cache in `main.rs`.

- [ ] **Step 5: Refresh the query cache, run all Rust checks, and commit**

Run: `cargo sqlx prepare -- --all-targets && cargo fmt && SQLX_OFFLINE=true cargo clippy --all-targets --locked -- -D warnings && cargo test --locked`
Expected: all PASS, no warnings.

```bash
git add server/src server/.sqlx
git commit -m "Serve stored candles from /api/candles"
```

---

### Task 4: Web data layer

**Files:**
- Create: `web/src/lib/candles.ts`, `web/src/lib/candles.test.ts`, `web/src/hooks/useCandles.ts`, `web/src/hooks/useCandles.test.ts`
- Modify: `web/src/types.ts`, `web/src/api/client.ts`

**Interfaces:**
- Produces:

```ts
// types.ts
export type CandleRange = '1d' | '5d'
export type CandleRow = [time: number, open: number, high: number, low: number, close: number]
export interface CandleSeries { symbol: string; range: CandleRange; bucketSeconds: number; candles: CandleRow[] }

// client.ts
export const fetchCandles: (symbol: string, range: CandleRange, signal?: AbortSignal) => Promise<CandleSeries>

// lib/candles.ts
export interface Bar { time: number; open: number; high: number; low: number; close: number }
export function toBars(rows: readonly CandleRow[]): Bar[]
/** `null` = tick lebih lama dari bar terakhir; `update()` chart bakal throw kalau dikasih itu. */
export function applyTick(last: Bar | undefined, price: number, timestampMs: number, bucketSeconds: number): Bar | null
export function formatWibTime(unixSeconds: number): string // "05:13"  (en-GB, Asia/Jakarta, h23)
export function formatWibDay(unixSeconds: number): string  // "15 Nov" (id-ID, Asia/Jakarta)

// hooks/useCandles.ts
export type CandlesState =
  | { status: 'loading' }
  | { status: 'ok'; series: CandleSeries }
  | { status: 'error'; message: string }
export function useCandles(symbol: string, range: CandleRange, connection: ConnectionState):
  { state: CandlesState; retry: () => void }
```

- [ ] **Step 1: Write failing tests for `lib/candles.ts`**

```ts
it('mengubah baris server jadi bar')
// toBars([[60,1,3,0,2]]) → [{time:60, open:1, high:3, low:0, close:2}]
it('tick di bucket yang sama memperbarui bar')
// last {time:60,o10,h12,l9,c11}, price 13, ts 61_000, bucket 60 → {time:60,o10,h13,l9,c13}
it('tick di bucket baru membuat bar baru')
// price 13, ts 120_000 → {time:120,o13,h13,l13,c13}
it('bar pertama tanpa riwayat')
// last undefined, price 5, ts 61_000, bucket 60 → {time:60,o5,h5,l5,c5}
it('tick lebih lama dari bar terakhir diabaikan')
// last.time 120, ts 119_999, bucket 60 → null
it('bucket 5 menit dibulatkan ke bawah')
// last undefined, ts 899_000, bucket 300 → time 600
it('format waktu WIB')
// formatWibTime(1_700_000_000) === '05:13', formatWibDay(1_700_000_000) === '15 Nov'
```

- [ ] **Step 2: Write failing tests for `useCandles`, mocking `../api/client`**

Follow `useNews.test.ts`.

```ts
it('memuat lalu ok')
it('error lalu retry berhasil')
it('respons lama nggak menimpa simbol yang baru dipilih')
// AMD resolves AFTER the rerender to NVDA → state holds NVDA; AMD's signal is aborted
it('ganti range kembali ke loading dan mengambil range baru')
it('reconnect mengambil ulang tanpa mengosongkan data')
// ok → connection 'reconnecting' → 'live': fetch called again, status stays 'ok' meanwhile
it('koneksi pertama connecting ke live nggak mengambil dua kali')
it('refresh yang gagal nggak menghapus data yang sudah tampil')
it('simbol crypto dikodekan di URL')
// real fetchCandles with stubbed global fetch: URL contains symbol=BINANCE%3ABTCUSDT&range=5d
```

- [ ] **Step 3: Run them and confirm they fail**

Run, from `web/`: `npx vitest run src/lib/candles.test.ts src/hooks/useCandles.test.ts`
Expected: FAIL, modules not found.

- [ ] **Step 4: Implement types, `fetchCandles`, `lib/candles.ts`, and `useCandles`**

Each fetch gets its own `AbortController`, aborted on cleanup. Symbol or range change sets `loading`. A refetch (retry after an error, or a return to `live` from `reconnecting`) keeps existing `ok` data. Track the previous connection in a ref so only `reconnecting → live` triggers a refetch.

- [ ] **Step 5: Run web checks and commit**

Run, from `web/`: `npm run lint && npm test`
Expected: all PASS.

```bash
git add web/src
git commit -m "Fetch candles and apply live ticks on the web"
```

---

### Task 5: Chart in the side panel

**Files:**
- Create: `web/src/components/IntradayChart.tsx`, `web/src/components/IntradayChart.test.tsx`
- Modify: `web/src/components/NewsPanel.tsx`, `web/src/components/NewsPanel.test.tsx`, `web/src/App.tsx`, `web/src/index.css`, `web/package.json`, `web/package-lock.json`

**Interfaces:**
- Consumes: everything Task 4 produces, `LiveTick` from `lib/tickStore`, `ConnectionState` from `useLiveTicks`.
- Produces:

```ts
// default export, so React.lazy can load it
export default function IntradayChart(props: {
  symbol: string
  live: LiveTick | undefined
  connection: ConnectionState
}): JSX.Element
```

- `NewsPanel` gains required props `live: LiveTick | undefined` and `connection: ConnectionState`.

- [ ] **Step 1: Install the library**

Run, from `web/`: `npm install lightweight-charts@^5.3.0`
Expected: `package.json` and `package-lock.json` updated.

- [ ] **Step 2: Write failing component tests with `lightweight-charts` mocked**

jsdom has no canvas. `vi.mock('lightweight-charts', ...)` must return a `createChart` whose chart has spies for `addSeries`, `remove`, `applyOptions`, and `timeScale().fitContent`, plus a series with `setData` and `update`. Mock `../hooks/useCandles` too.

```tsx
it('menampilkan toggle 1H dan 5H, default 1H')   // aria-pressed true on 1H, false on 5H
it('klik 5H meminta range 5d')                    // useCandles called with ('NVDA','5d','live')
it('mengisi chart saat data datang')              // series.setData called with the bars
it('kosong menampilkan pesan')                    // exact Empty copy
it('error menampilkan pesan dan Coba lagi memanggil retry')
it('catatan feed selalu tampil')                  // in loading, empty, error and ok states
it('punya label untuk pembaca layar')             // role img, name "Grafik harga NVDA, 1 hari"
it('tick baru memanggil update')
it('tick lama nggak memanggil update')            // live.timestamp older than the last bar
it('tutup panel menghapus chart sekali dan tick berikutnya nggak dipakai')
// unmount → chart.remove called once; series.update not called afterwards
it('ganti simbol memuat data baru tanpa membuat chart kedua') // createChart called once
```

- [ ] **Step 3: Run them and confirm they fail**

Run, from `web/`: `npx vitest run src/components/IntradayChart.test.tsx`
Expected: FAIL, module not found.

- [ ] **Step 4: Implement `IntradayChart.tsx`**

- Create the chart once in an effect: `createChart(container, { autoSize: true, ... })`, `chart.addSeries(CandlestickSeries, ...)`, and `chart.remove()` in cleanup. Keep chart, series and last bar in refs.
- Keep the container mounted in every state and draw loading, empty and error on top of it, so chart and panel height never change.
- `setData` when fetched bars change. Call `timeScale().fitContent()` only when symbol or range changed, so a background refetch keeps the owner's zoom.
- For live ticks, call `applyTick` against the last-bar ref and call `series.update` only when it is not `null`. Skip ticks until status is `ok`.
- Text colors should match `--muted` (`#9ba5a0`), grid `--line-soft` (`#222e2b`), transparent background.
- WIB: `localization.timeFormatter` = `` `${formatWibDay(t)} ${formatWibTime(t)}` ``. `timeScale.tickMarkFormatter` uses `formatWibTime` for `TickMarkType.Time`/`TimeWithSeconds`, otherwise `formatWibDay`.
- Do not disable `layout.attributionLogo`.
- Toggle labels `1H` and `5H`, as `<button type="button" aria-pressed=...>`. Range state lives in this component and is kept when the symbol changes.
- `aria-label`: `Grafik harga ${symbol}, ${range === '1d' ? '1 hari' : '5 hari'}`.

- [ ] **Step 5: Integrate in `NewsPanel` and `App`**

In `NewsPanel`, load the chart with `const IntradayChart = lazy(() => import('./IntradayChart'))` and replace `<div className="chart-slot" />` with it inside `<Suspense>`. The fallback is an empty box of the same height. Pass `live={ticks[open]}` and `connection={connection}` from `App`.

In `NewsPanel.test.tsx`, add `vi.mock('./IntradayChart', () => ({ default: () => null }))` and pass the new props. Existing assertions must stay unchanged.

Add CSS to `index.css` using existing variables. The chart area has a fixed height of `240px`. Toggle buttons meet the existing focus-ring style and stay usable at the `560px` full-screen breakpoint.

- [ ] **Step 6: Run every check**

Run, from `web/`: `npm run lint && npm test && npm run build`
Expected: all PASS. The build output shows `lightweight-charts` in a **separate chunk**, not in the main bundle.

Run, from `server/`: `cargo fmt --check && SQLX_OFFLINE=true cargo clippy --all-targets --locked -- -D warnings && cargo test --locked`
Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add web
git commit -m "Show the intraday chart in the side panel"
```

---

## Handoff

When Task 5 is done:

1. Run `git status --short` (must be clean) and `git log --oneline main..intraday-chart`.
2. **Do not push or open a PR.**
3. Report to the owner: commits made, final check output, any deviation from this plan and why, and anything you could not verify. The chart has not been seen in a real browser with real data, because the server must not run locally. State that plainly.
