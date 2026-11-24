# Intraday chart from our own stored trades

Status: approved by the owner on 2026-11-24. Feature 2 on the Tickstream roadmap.

## Why

The owner opens the dashboard and sees that a stock moved a lot while they were
asleep. Today the board shows the size of the move, and a 30-minute sparkline, but
not how the move unfolded. Clicking a row should show that.

Finnhub's candle endpoint is premium and banned here, so history cannot be fetched.
It only exists if this server records it, starting the day this feature goes live.

Success: the owner clicks NVDA in the morning (WIB) and sees last night's session as
candlesticks, can switch to the last five trading days, and sees the newest candle
move while the market is open.

## Goals

- Record one-minute OHLC candles for every watchlist symbol into Postgres, whether or
  not a browser is open.
- `GET /api/candles?symbol=NVDA&range=1d|5d` returns stored candles.
- A candlestick chart in the existing side panel, in the empty `.chart-slot`, with a
  `1H` / `5H` toggle (`H` = hari). Default `1H`.
- The newest candle updates live from the existing WebSocket ticks.

## Non-goals

- Buy/sell signals, indicators (MA, RSI), drawing tools, or any wording that tells the
  reader what to do.
- Backfilling history from any source.
- A volume pane. The free feed covers only part of the market's volume, so the bars
  would mislead. Volume is stored anyway, because it cannot be recovered later.
- Ranges beyond 5 days, custom intervals, a separate page, or a router.
- Price alerts (feature 1).
- Filtering trades by Finnhub condition codes.

## Anti-goals (a failure even if it works)

- Calling `/stock/candle` or any other premium endpoint.
- The Finnhub API key reaching the browser, a log line, or a response.
- Recording slowing down or breaking the live feed. A database outage must not affect
  ticks on the board.
- A chart that looks official while hiding that it comes from a partial feed.
- Opening a second Finnhub WebSocket: **never run the server binary locally**. The
  free tier allows one socket per key and a local run kicks production off the feed.
  `cargo test` is fine because tests use a fake feed.

## Constraints

- VPS: 1 vCPU, 2 GB RAM, shared with other sites. Nothing heavy in the request path.
- `sqlx` is built **without** `chrono`/`time`. Do not add them. Convert in SQL:
  `to_timestamp(...)` on write, `extract(epoch FROM ...)::bigint` on read.
- `query!` macros are checked against `server/.sqlx`, and CI builds with
  `SQLX_OFFLINE=true`. New queries need `cargo sqlx prepare` and the result committed.
- CI runs `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo test --locked`, then in `web`: `npm run lint`, `npm test`, build.
- Code and commits in English, code comments and UI text in Indonesian, matching the
  repo. The owner is learning Rust and React: comments explain the why.
- KISS, DRY, YAGNI, small commits.

## Approaches

**Storage (one sensible way).** Store one-minute candles and derive five-minute candles
in the SQL query. Rejected: raw trades (far too many rows for this VPS) and a second
table per interval (duplicated data that can disagree).

**Rendering (owner chose A).**

- **A. `lightweight-charts` 5.x. Chosen.** Candlesticks, axes, crosshair, zoom and touch
  come for free, so little code is left to get wrong. Apache-2.0, requires TradingView
  attribution.
- B. Hand-written canvas like `Sparkline`. No dependency, but axes, crosshair, zoom,
  touch and session gaps would be several hundred lines outside this project's focus.

**Range (owner chose B).** A `1H` / `5H` toggle, rather than only the last session.

## Design

### Data flow

```
Finnhub WS → broadcast → Recorder::record → CandleBuffer (memory)
                                                 │ every 5 s
                                                 ▼
                                          Postgres `candles`
                                                 ▲
Browser panel → GET /api/candles → 5 s cache ────┘
Browser panel ← existing /ws ticks → newest candle moves
```

### Table

New migration:

```sql
CREATE TABLE candles (
    symbol        TEXT             NOT NULL,
    minute        TIMESTAMPTZ      NOT NULL,
    open          DOUBLE PRECISION NOT NULL,
    high          DOUBLE PRECISION NOT NULL,
    low           DOUBLE PRECISION NOT NULL,
    close         DOUBLE PRECISION NOT NULL,
    volume        DOUBLE PRECISION NOT NULL,
    -- Decides which close wins when a late trade arrives in a later flush.
    last_trade_ms BIGINT           NOT NULL,
    PRIMARY KEY (symbol, minute)
);
```

No foreign key to `watchlist`: removing a symbol keeps its candles until retention
removes them, so an accidental removal does not destroy data that cannot be recovered.

### Recording (`server/src/candles/`)

`CandleBuffer` holds pending candles keyed by `(symbol, minute)` behind a
`std::sync::Mutex` that is never held across `.await`.

- `Recorder::record` passes it the same filtered trades it gives `History`. Recording
  stays synchronous and does no I/O.
- A trade is ignored if its price is not finite or not above 0, or if its timestamp is
  more than 1 hour old or more than 1 minute in the future. A zero volume is accepted.
- Within the buffer: first trade sets open, `high`/`low` widen, volume adds, and close
  comes from the trade with the highest timestamp (ties: the later arrival).
- A background task on the existing `TaskTracker` flushes every 5 seconds: take the whole
  map out of the lock, then write it with one `INSERT ... SELECT FROM UNNEST(...)`.
- On conflict: keep `open`, `high = GREATEST`, `low = LEAST`, `volume` adds, and `close`
  and `last_trade_ms` update only if the incoming `last_trade_ms` is at least the stored one.
- A failed flush merges the candles back into the buffer using the same rules and logs a
  warning. The buffer holds at most 3,000 candles; past that the oldest minutes are
  dropped with a warning, so a long database outage cannot grow memory without limit.
- On shutdown the task does one last flush. It must fit within the existing 5-second
  `SHUTDOWN_GRACE`, so bound the final write with a 3-second timeout.
- Once at startup and then hourly: `DELETE FROM candles WHERE minute < now() - interval '14 days'`.
  Fourteen days keeps five trading days intact across long holidays.
- One counter, `tickstream_candle_flushes_total{outcome="ok|error"}`, because lost
  candles cannot be recovered and a silent failure must be visible on `/metrics`.

### API

`GET /api/candles?symbol=NVDA&range=1d`. Public like the other market reads, and guarded
by the existing `watched_symbol` so only watchlist symbols work.

```json
{ "symbol": "NVDA", "range": "1d", "bucketSeconds": 60,
  "candles": [[1700000000, 130.1, 131.0, 129.9, 130.7]] }
```

Each candle is `[unixSeconds, open, high, low, close]`, oldest first. An unknown `range`
is a 400. A watched symbol with no data is a 200 with an empty list.

| Range | Stocks | Symbols containing `:` (crypto, 24h) | Bucket |
|---|---|---|---|
| `1d` | from the start of the latest **session date**, through the newest candle | last 24 hours | 60 s |
| `5d` | from the start of the 5th-latest **session date**, through the newest candle | last 120 hours | 300 s |

- A **session date** is a New York date, Monday to Friday, with at least one candle in
  regular hours (`09:30 <= local time < 16:00`). New York date:
  `(minute AT TIME ZONE 'America/New_York')::date`. This works across weekends and holidays
  with no market calendar, and DST is handled by the time zone conversion.
- The window starts at 00:00 New York on that date and includes **everything after it**:
  that date's pre-market and after-hours, plus any later candles that are not a session date
  yet. So during today's pre-market, `1H` shows last night's full session with today's
  pre-market continuing on the right, and the newest candle is always in view. When the first
  regular-hours candle of today arrives, today becomes the latest session date and `1H` moves
  to today. Owner's decision: a thin pre-market must not replace last night's session.
- If there are fewer session dates than the range asks for (including none, right after
  launch), return all stored candles for the symbol.
- Five-minute candles: `date_bin('5 minutes', minute, TIMESTAMPTZ '2000-01-01')`, first
  `open`, `max(high)`, `min(low)`, last `close`.
- Responses are cached in memory for 5 seconds per `(symbol, range)` using `moka` (already
  a dependency, at most 100 entries), so a public endpoint cannot hammer Postgres.

### Web

- `web/src/lib/candles.ts`: pure, unit-tested helpers. Converting the response to chart
  bars, and `applyTick(lastBar, price, timestampMs, bucketSeconds)`, which returns the
  updated or new bar, or `null` for a tick older than the last bar.
  **The `null` case matters: the library throws if `update()` gets an older time.**
- `web/src/hooks/useCandles.ts`: fetches on symbol or range change and when the WebSocket
  returns to `live`. Aborts the previous request. States: loading, error with retry, ok.
- `web/src/components/IntradayChart.tsx`: creates the chart once, calls `setData` when
  fetched data changes and `update` for live ticks, resizes with the panel
  (`autoSize: true`), and calls `chart.remove()` on unmount.
- Load the chart with `React.lazy`, so the board's first load does not pay for it.
- Live ticks come from the existing `ticks[symbol]` in `App`. `NewsPanel` receives what the
  chart needs and renders it in place of `.chart-slot`. Keep the slot's height stable
  across loading, empty and error states, so the news list does not jump.
- Times on the axis and crosshair are **WIB** (`Asia/Jakarta`) via `localization.timeFormatter`
  and `tickMarkFormatter`, matching the session bar.
- Colors match the board: up `#52c48c`, down `#f2705f`, on the panel's dark surface.
- Keep the TradingView attribution logo on (`layout.attributionLogo`, the default).
- The chart compresses time with no trades, so nights and weekends do not leave gaps.
- Toggle: two real buttons with `aria-pressed`. The chart container has `role="img"` and an
  `aria-label` such as "Grafik harga NVDA, 1 hari".
- UI copy:
  - Empty: "Belum ada data grafik. Data mulai terkumpul saat ada transaksi."
  - Error: "Grafik nggak bisa dimuat." with a "Coba lagi" button.
  - Always-visible note: "Dari transaksi yang diterima feed gratis, bisa beda dari angka
    resmi."

### Known limits (accepted)

- History starts on deploy day. `5H` fills up over five trading days.
- Downtime or a lost Finnhub socket leaves permanent holes.
- A freshly loaded chart can miss up to 5 seconds of unflushed trades in its last candle.
  The next load corrects it.
- A late trade that is earlier than a candle's first trade does not change that candle's open.
- A stray regular-hours print on a weekday market holiday counts as a session date, since
  there is no market calendar. This should be rare.

## Testing

- Rust unit: bucket flooring, OHLC within a minute, out-of-order close, rejected trades,
  merge after a failed flush, buffer cap.
- `#[sqlx::test]`: upsert across two flushes including a late trade; 1d/5d selection across
  a weekend gap and the New York date boundary; pre-market today keeps last night's session in
  `1d`, and the first regular-hours candle moves `1d` to today; a stray weekend print neither
  counts as a session date nor disappears; fewer session dates than requested; a DST date;
  the crypto rolling window; the 5-minute rollup; retention delete.
- Route tests: 200, empty list, 400 on a bad range, rejection for an unwatched symbol.
- Web: `candles.ts` unit tests including the older-tick `null`; `useCandles` states; the
  component with `lightweight-charts` mocked, since jsdom has no canvas.

## Left to the builder

File-internal structure, helper names, and CSS details, as long as they follow the
patterns in `history.rs`, `market/cache.rs`, `useNews.ts` and `index.css`.
