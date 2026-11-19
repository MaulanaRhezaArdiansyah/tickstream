# Sparkline History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every row's sparkline shows the last 30 minutes (180 × 10 s buckets) as soon as the page opens.

**Architecture:** A server-side `Recorder` holds its own hub subscription to every watchlist symbol and folds the trade broadcast into per-symbol buckets. On a successful WS subscribe the server sends a `history` message before forwarding live trades. The web `TickStore` keeps sparkline points separate from live trade state, seeds them from `history` and buckets live trades the same way.

**Tech Stack:** Rust (Axum 0.8, Tokio, SQLx 0.9, tokio-tungstenite 0.30), React 19 + TS, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-19-sparkline-history-design.md`

## Global Constraints

- `BUCKET_MS = 10_000`, `MAX_BUCKETS = 180`, on both server and web. Bucket start = `floor(timestamp_ms / 10_000) * 10_000`, from the trade's own timestamp.
- Same bucket as the newest → replace its price; newer → append; older than the newest → ignore; keep at most 180 (drop oldest).
- WS message: `{"type":"history","symbol":"AAPL","points":[[bucketStartMs, price], ...]}`, oldest first; a symbol with no history gets `"points":[]`. Sent only after a subscribe succeeds, and before any trade for that symbol.
- History is in memory only. No new dependencies, no premium Finnhub endpoints, API key stays server-side.
- Server checks: `cargo fmt`, `SQLX_OFFLINE=true cargo clippy --all-targets --locked -- -D warnings`, `cargo test`. Web checks: `npm run -s lint`, `npm test`, `npm run -s build`. Prettier `--no-semi --single-quote --print-width 100`.
- Comments and test names follow the repo: Indonesian, explaining the why for a Laravel/Vue developer learning Rust/React. Characterization rule from the last plan still applies to changed tests: see each new test fail before the code exists.
- Pushing to `main` deploys. Work on branch `sparkline-history`; integrate through a PR.

## Review Focus

- A trade at exactly a bucket boundary (`10_000`) starts a new bucket; `9_999` stays in bucket `0` — pinned in Task 1 and Task 4.
- An out-of-order trade older than the newest bucket must not rewrite or reorder the line — Task 1 and Task 4.
- Reconnect: a second `history` for the same symbol replaces the browser's points instead of appending a duplicate tail — Task 4.
- At night a symbol has history but no live trade: the sparkline draws, while price, volume and high/low still come from the quote (no fake live price, volume stays `–`) — Task 4.
- Removing a symbol and adding it back starts with empty history; a rejected subscribe sends no history — Task 2 and Task 3.

---

### Task 1: Bucketed history

**Files:**
- Create: `server/src/history.rs` (add `mod history;` in `server/src/main.rs`)

**Interfaces:**
- Produces: `pub const BUCKET_MS: i64 = 10_000; pub const MAX_BUCKETS: usize = 180;` `pub type Point = (i64, f64);` (serializes as `[t, price]`) and `pub struct History` with `fn record(&self, trades: &[Trade])`, `fn points(&self, symbol: &str) -> Vec<Point>`, `fn forget(&self, symbol: &str)`. Interior `std::sync::Mutex<HashMap<String, VecDeque<Point>>>`, `Default`.

- [ ] **Step 1: Write the failing tests** in `#[cfg(test)] mod tests` (helper `trade(symbol, t, price)`):
  - `same_bucket_keeps_the_last_price`: AAPL at t 0 → 10.0, 9_999 → 11.0, 10_000 → 12.0 ⇒ `points("AAPL") == [(0, 11.0), (10_000, 12.0)]`.
  - `bucket_start_is_floored`: t 1_700_000_019_999 ⇒ bucket 1_700_000_010_000.
  - `older_trade_is_ignored`: 20_000 → 5.0 then 10_000 → 9.0 ⇒ `[(20_000, 5.0)]`.
  - `keeps_only_the_newest_180`: 181 trades at t `i * 10_000`, price `i` ⇒ len 180, first `(10_000, 1.0)`, last `(1_800_000, 180.0)`.
  - `symbols_are_separate_and_unknown_is_empty`: AAPL and NVDA in one `record` call each get their own point; `points("MSFT")` is empty.
  - `forget_drops_history`.
- [ ] **Step 2: Run** `cargo test history` — Expected: compile error / FAIL (module missing).
- [ ] **Step 3: Implement** `History` in `server/src/history.rs`, with a module doc comment on why buckets (one scale for every row, independent of trade rate).
- [ ] **Step 4: Run** `cargo test history` — Expected: 6 passed. Then the full server checks.
- [ ] **Step 5: Commit** "Keep 30 minutes of 10-second price buckets per symbol".

### Task 2: Recorder: always watch the watchlist

**Files:**
- Modify: `server/src/history.rs`, `server/src/state.rs`, `server/src/main.rs`, `server/src/routes/watchlist.rs`, `server/src/routes/tests.rs`

**Interfaces:**
- Consumes: `History` (Task 1); `Hub::subscription()`, `Subscription::{add, remove}`, `HubError`, `Hub::trades()`; `watchlist::list`.
- Produces: `pub struct Recorder { history: History, subscription: Mutex<Subscription> }` with
  - `pub fn new(hub: &Arc<Hub>) -> Arc<Self>`
  - `pub fn watch(&self, symbol: &str) -> Result<(), HubError>`
  - `pub fn unwatch(&self, symbol: &str)` — removes the subscription and forgets the history
  - `pub fn points(&self, symbol: &str) -> Vec<Point>`
  - `pub fn record(&self, trades: &[Trade])` (delegates; used by the loop and by tests)
  - `pub async fn watch_watchlist(&self, db: &PgPool) -> sqlx::Result<()>` — watches every row; a `HubError` is logged with `tracing::warn!`, not returned
  - `pub fn spawn(self: Arc<Self>, trades: broadcast::Receiver<TradeBatch>, shutdown: CancellationToken, tasks: &TaskTracker)` — loop: `select!` on `shutdown.cancelled()` / `trades.recv()`; `Ok` → `record`, `Lagged` → `warn!` and continue, `Closed` → stop.
  - `AppState.recorder: Arc<Recorder>`.

- [ ] **Step 1: Write the failing tests.**
  In `history.rs` tests (reuse a recording feed like `hub.rs` tests):
  - `watching_subscribes_upstream_once_and_unwatch_releases`: `watch("AAPL")` twice ⇒ feed log `["+AAPL"]`; `unwatch("AAPL")` ⇒ `["+AAPL", "-AAPL"]`.
  - `unwatch_forgets_history`: record AAPL, `unwatch`, `watch` again ⇒ `points("AAPL")` empty.
  - `#[tokio::test] spawned_recorder_reads_the_broadcast`: `spawn` with a receiver, send one batch, poll `points` (up to ~1 s with `tokio::time::sleep(10ms)`) ⇒ one point; cancel the token ⇒ `tasks.close(); tasks.wait()` finishes within 1 s.
  In `routes/tests.rs` (add `state_with_feed(db, feed)`; `state(db)` builds its `Recorder` from the same hub):
  - `#[sqlx::test] recorder_watches_the_seeded_watchlist`: `watch_watchlist` ⇒ feed log is the 9 seeded symbols with `+`, in table order.
  - `#[sqlx::test] watchlist_changes_follow_upstream`: POST AMD ⇒ log ends with `+AMD`; DELETE AMD ⇒ ends with `-AMD`.
- [ ] **Step 2: Run** `cargo test` — Expected: compile errors for `Recorder` / `recorder` field.
- [ ] **Step 3: Implement** `Recorder`; add the state field; in `main.rs` build `hub` as an `Arc` first, then `Recorder::new(&hub)`, `watch_watchlist(&db).await?`, `spawn(hub.trades(), shutdown.clone(), &tasks)`. In `routes/watchlist.rs` call `recorder.watch` after a successful insert (warn on `Err`) and `recorder.unwatch` after a successful delete.
- [ ] **Step 4: Run** the full server checks — Expected: all green.
- [ ] **Step 5: Commit** "Record trades for every watchlist symbol, even with no visitors".

### Task 3: `history` over the WebSocket

**Files:**
- Modify: `server/src/routes/ws.rs` (module doc lists the new message), `server/src/routes/tests.rs`, `README.md` (only if it documents WS messages)

**Interfaces:**
- Consumes: `Recorder::points`, `AppState.recorder` (Task 2).
- Produces: `ServerMessage::History { symbol: String, points: Vec<Point> }` → `{"type":"history",...}`. `handle_command` returns `Ok(Some(symbol))` after a successful subscribe, `Ok(None)` otherwise; `client` then sends the history before the next loop turn, so it always precedes that symbol's trades.

- [ ] **Step 1: Write the failing test** in `routes/tests.rs`, over a real socket: bind `127.0.0.1:0`, `tokio::spawn(axum::serve(listener, router))`, connect with `tokio_tungstenite::connect_async("ws://{addr}/ws")`. `#[sqlx::test] ws_sends_history_after_subscribe`:
  - before connecting, `state.recorder.record(&[AAPL @ t 20_000, 100.5])`
  - send `{"type":"subscribe","symbol":"aapl"}` ⇒ next text message equals `{"type":"history","symbol":"AAPL","points":[[20000,100.5]]}`
  - send subscribe `NVDA` ⇒ `{"type":"history","symbol":"NVDA","points":[]}`
  - send subscribe `AMD` (not in the watchlist) ⇒ next message has `"type":"error"`; then subscribe `MSFT` ⇒ the next message is MSFT's history (proves no history was sent for AMD).
  Wrap each read in `tokio::time::timeout(2s)`.
- [ ] **Step 2: Run** `cargo test ws_sends_history` — Expected: FAIL (first message missing / timeout).
- [ ] **Step 3: Implement** the message and the send in `client`; pass the recorder in from `ws()`.
- [ ] **Step 4: Run** the full server checks — Expected: all green.
- [ ] **Step 5: Commit** "Send each symbol's sparkline history when a browser subscribes".

### Task 4: Web: seed and bucket the sparkline

**Files:**
- Modify: `web/src/types.ts`, `web/src/lib/tickStore.ts`, `web/src/lib/tickStore.test.ts`, `web/src/hooks/useLiveTicks.ts`, the component that renders `QuoteRow` (find with `grep -n "<QuoteRow" web/src`), `web/src/components/QuoteRow.tsx`, `web/src/components/QuoteRow.test.tsx`

**Interfaces:**
- Consumes: the `history` message (Task 3).
- Produces:
  - `ServerMessage` gains `{ type: 'history'; symbol: string; points: [number, number][] }`.
  - `LiveTick` loses `points`. New `export type Sparks = Readonly<Record<string, readonly number[]>>`.
  - `TickStore.seed(symbol: string, points: readonly (readonly [number, number])[]): void` — replaces that symbol's sparkline (server history is authoritative, so a reconnect never duplicates the tail) and schedules a frame. It creates no `LiveTick`.
  - `TickStore.getSparks = (): Sparks` — new object only for symbols whose points changed in that frame; untouched symbols keep their array reference. Same `subscribe`/frame batching as `getSnapshot`.
  - `useLiveTicks` returns `{ ticks, sparks, connection, ticksPerSecond }`; `'history'` messages call `store.seed`.
  - `QuoteRow` gets `points: readonly number[] | undefined` (falls back to `NO_POINTS`); the caller passes `sparks[symbol]`.

- [ ] **Step 1: Write the failing tests** in `tickStore.test.ts` (replace the old 300-point test and drop `points` from the up/down test):
  - `sparkline per 10 detik: kelompok yang sama menimpa harga`: trades t 0 → 10, 9_999 → 11, 10_000 → 12 ⇒ `getSparks().AAPL` `[11, 12]`.
  - `trade yang lebih lama dari kelompok terbaru diabaikan`: 20_000 → 5, 10_000 → 9 ⇒ `[5]`.
  - `sparkline menyimpan paling banyak 180 titik`: 181 trades at `i * 10_000` price `i` ⇒ length 180, first `1`.
  - `seed mengisi sparkline tanpa membuat harga live`: seed AAPL `[[0, 5], [10_000, 6]]`, frame ⇒ sparks `[5, 6]`, `getSnapshot().AAPL` is `undefined`.
  - `trade live menyambung dari riwayat`: after that seed, trade t 15_000 → 7 ⇒ `[5, 7]`; t 20_000 → 8 ⇒ `[5, 7, 8]`.
  - `seed kedua menggantikan, bukan menambah`: trades then seed `[[0, 1]]` ⇒ `[1]`.
  - extend `simbol yang nggak berubah tetap objek yang sama` to assert `getSparks().MSFT` keeps its reference.
  In `QuoteRow.test.tsx`: `renderRow` passes `points`; existing cases stay green.
- [ ] **Step 2: Run** `npm test` — Expected: FAIL (`getSparks`/`seed` missing, type errors in build).
- [ ] **Step 3: Implement**: per-symbol `{ newest: number; prices: number[] }` in the store, the bucket rule from Global Constraints, `MAX_POINTS = 180`, `BUCKET_MS = 10_000`; update the `LiveTick` doc comments; wire `useLiveTicks`, the board and `QuoteRow`.
- [ ] **Step 4: Run** web checks (`npm run -s lint`, `npm test`, `npm run -s build`) — Expected: all green, no stderr noise.
- [ ] **Step 5: Commit** "Draw the sparkline from 30 minutes of history".
- [ ] **Step 6: Push the branch, open a PR to `main`, watch CI.** Do not merge without the user.
