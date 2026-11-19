# Sparkline history: design

**Status:** approved approach A (2026-11-19). Small-to-medium change: server, WS protocol, web tick store.

## Intent

When someone opens Tickstream, every row's sparkline already shows how the price moved over
the last 30 minutes, instead of starting as an empty line that fills while the page is open.
All rows share the same time scale, so their shapes can be compared. The data model is a
first step towards feature 2 (intraday chart from our own stored trades).

## Goals

- Server keeps, per watchlist symbol, the last **180 buckets of 10 seconds** (30 minutes):
  for each bucket, the price of the last trade in it.
- When a browser subscribes to a symbol, the server sends that symbol's history **before**
  any live trade for it, on the same WebSocket.
- The browser buckets live trades the same way, so history and live data join into one line
  at one scale.
- History exists even if nobody had the page open: the server subscribes upstream to every
  watchlist symbol, all the time.

## Non-goals

- Persisting history across restarts (a deploy empties it; it refills in minutes). Postgres
  storage belongs to feature 2.
- OHLC, volume or high/low per bucket. The sparkline only needs one price per bucket.
- A REST endpoint for history (approach B, rejected below).
- Changing how the row's price, change % or session high/low are shown.

## Anti-goals

- A sparkline whose line suddenly gets denser or jumps in scale where history ends and live
  data begins.
- Live trades arriving before the history and being lost or drawn out of order.
- The always-on upstream subscription taking live slots away from browsers.

## Constraints

- Finnhub free tier: one WebSocket per key, max 50 symbols. The watchlist is already capped
  at the same 50 (`watchlist::MAX_ENTRIES = hub::MAX_SYMBOLS`), and browsers may only
  subscribe to watchlist symbols, so holding every watchlist symbol costs no extra slots.
- The API key stays on the server. Premium endpoints (`/stock/candle` etc.) stay unused:
  history is built only from trades we receive.
- Buckets only exist when trades arrived in them. Outside market hours the history is the
  last 180 buckets that had trades (the last ~30 minutes of trading before the close), not a
  flat line. For liquid symbols in regular hours every bucket has trades, so the time axis
  is real.

## Approaches

- **A. History over the WebSocket, on subscribe (chosen).** After a successful subscribe the
  server sends `{"type":"history","symbol":"AAPL","points":[[t, price], ...]}` and only then
  forwards live trades for that symbol. Ordering is guaranteed by the single connection;
  reconnect re-subscribes and therefore re-seeds with no extra code. Cost: one new message
  type in the protocol.
- **B. `GET /api/sparklines` (rejected).** Easy to inspect, but history and live trades
  travel separately: trades can arrive first, so the browser must merge and de-duplicate,
  and reconnect must remember to refetch.

## Decided

- Bucket = `floor(timestamp_ms / 10_000) * 10_000`, keyed on the trade's own timestamp.
  A trade in the same bucket as the newest one replaces its price; a newer bucket is
  appended; at most 180 kept (oldest dropped). A trade older than the newest bucket is
  ignored for the sparkline.
- `points` entries are `[bucketStartMs, price]`, oldest first, so the browser can keep
  bucketing from where history ends. An unknown or empty symbol gets `points: []`.
- The server holds its own subscription to all watchlist symbols: taken at startup from the
  table, updated when the owner adds or removes a symbol. Removing a symbol drops its history.
- The browser's tick store gets the history as a seed and draws `points` as per-bucket prices.
  Price, volume, high and low keep coming from live trades as today.
- A trade that is in the history and also delivered live (it can be queued in the client's
  receiver while the history is read) only rewrites the same bucket with the same price, so
  the line is unaffected.

## Left to the builder

- Where the recorder lives (its own module/task reading the broadcast channel) and how the
  WS handler reads from it.
- Exact Rust and TypeScript names, and test layout. Server tests cover bucketing, the
  180 cap and history-before-trades ordering; web tests cover seeding and live bucketing.
