# Company news per stock: design

Date: 2026-11-20. Status: approved by the owner (layout B).

## Intent

Tickstream shows live prices for a watchlist of US stocks. The board already
says *how much* a stock moved since yesterday's close. This feature answers
*why*, and the questions that follow from it, for the owner and for any
visitor who holds US stocks:

- Why did this stock move today?
- What is the latest news about company A?
- Has company B announced anything new (a product, a contract, an
  acquisition)?

Success: a person sees AMD −5.29% on the board, clicks the row, and within a
second reads the headlines that explain it, with the live price still in
view.

## Goals

- A news count badge on every board row: the number of articles in the last
  24 hours, so busy stocks stand out from the board.
- Clicking a row opens a side panel for that symbol: name, live price and
  change, a reserved slot for the intraday chart (feature 2, later), then
  the news list.
- The news list covers the last 7 days, newest first, at most 20 items.
  For busy stocks 20 items may span only hours; that is fine, since the
  newest articles are the ones that explain today's move.
  Each item: relative time ("2 jam lalu"), source, headline, short summary,
  and a link that opens the original article in a new tab.

## Non-goals

- Buy/sell recommendations, scores, or any wording that tells the reader
  what to do. The app presents news; the reader judges. (Finnhub's free
  analyst recommendation counts are a possible separate feature later.)
- Sentiment labels (premium on Finnhub, and banned by the owner).
- Article images, categories, search, filtering, or a cross-stock news feed.
- A separate page or URL per stock (option C was rejected); no router.
- Persisting news in Postgres. Memory cache only.
- Push of new news over the WebSocket.

## Anti-goals (a failure even if it works)

- The Finnhub API key reaching the browser in any form. The browser only
  calls our server.
- Calling any premium Finnhub endpoint (WebSocket news, `/stock/profile`
  v1, `/stock/candle`, news sentiment).
- Opening the board and getting 429 errors on quotes or profiles because
  news requests ate the shared rate limit.
- A link in the panel that runs script (`javascript:` URLs) or leaks the
  opener.

## Constraints (from the project)

- Server: Rust, Axum, `MarketDataSource` trait with `FinnhubRest` behind a
  moka `MarketCache`. Every REST call goes through one global limiter:
  60/min, burst 25, a request waits at most 2 s before 429. A cold board
  load already spends 2 calls per symbol (quote + profile).
- Market routes serve only watchlist symbols (`watched_symbol`); news
  follows the same rule. The watchlist holds at most 50 symbols and may
  include crypto symbols such as `BINANCE:BTCUSDT`.
- Web: React 19, TS, Vite, no router. Indonesian UI copy, casual but
  plain. Tests: Vitest + Testing Library on the web, `cargo test` with
  `#[sqlx::test]` on the server.

## Approaches considered

- **A. Row expands downward.** Simplest, keeps board context, but pushes
  the rows below far down and leaves no room for the feature 2 chart.
- **B. Side panel (chosen by the owner).** The board stays visible on the
  left with live prices; the panel sits on the right. On phones the panel
  is full screen with a close button. Feature 2's chart has a reserved
  slot at the top of the panel.
- **C. Separate page `/stock/AMD`.** Most room and a shareable link, but
  needs a router and hides the board while reading, which works against
  "why did it move" (you want the price in view).

## Decided

Server:
- New `MarketDataSource::company_news(symbol) -> Vec<NewsItem>` backed by
  Finnhub `GET /company-news?symbol=…&from=YYYY-MM-DD&to=YYYY-MM-DD`
  (free). `from` = 7 days before today (UTC), `to` = today (UTC).
- `NewsItem` sent to the browser: `id`, `datetime` (Unix ms), `headline`,
  `source`, `summary`, `url`. Other Finnhub fields are dropped.
- Server drops any item whose `url` is not `http`/`https` or whose
  headline is empty, counts the items from the last 24 hours (the true
  count, before any cap), sorts newest first and keeps at most 20.
- Cached in `MarketCache` for 15 minutes per symbol, shared by every
  visitor; errors are not cached (same as quotes).
- Route `GET /api/news?symbol=AMD` returns
  `{"count24h": 34, "items": [NewsItem, ...]}`; watchlist symbols only,
  same error shapes as `/api/quote`. A symbol Finnhub has no news for
  (crypto included) returns `{"count24h": 0, "items": []}`, not an error.
- **News has its own rate limit on top of the global one:** at most 20
  news calls per minute (burst 5). A news call waits for the news limiter
  first, then the global one. This guarantees quotes and profiles keep at
  least 40 of the 60 calls per minute, whatever many visitors or a cache
  expiry do. When the news limiter has no room within the usual wait, the
  news request gets the usual rate-limited error; the browser retries it
  later.

Web:
- The badge shows `count24h` from the server (the true count, so 34 and 3
  look different). "24 hours" is measured when the server fetched the
  news, at most 15 minutes stale.
- Rows with 0 articles in 24 h show no badge.
- Clicking a row (or Enter/Space on it) opens the panel; clicking the same
  row, the ✕ button, or Escape closes it. Opening another row switches the
  panel. If the open symbol is removed from the watchlist, the panel
  closes.
- The price and change in the panel are the same live values as the row.
- Panel states: loading, list, empty ("Belum ada berita 7 hari terakhir"),
  error with a retry button.
- Links: `target="_blank" rel="noopener noreferrer"`.

## My calls (owner can override)

- 7 days, max 20 items, 15-minute cache.
- **Loading badges without bursting the limiter:** the browser fetches news
  for watchlist symbols one at a time, after the board's quotes and
  profiles have loaded, rather than all at once. Opening a panel fetches
  that symbol right away. With the shared server cache, Finnhub sees at
  most one news call per symbol per 15 minutes regardless of visitors.
  A badge request that hits the rate limit is retried later in the queue;
  the badge simply appears a little later.
- The news-only limiter (20/min, burst 5): this, not browser pacing, is
  what guarantees the anti-goal on 429s, because several visitors or a
  cache expiry can request news in parallel.
- The badge uses the server's true 24 h count rather than counting the
  capped list, so busy stocks do not all show "20".
- News refreshes every 15 minutes while the page is open.
- The summary is clamped to 3 lines in the panel.
- On desktop, the panel takes roughly 40% of the width; the board's narrow
  columns hide as they already do at narrow widths.

## Left to the builder

- Component and hook names, the exact panel width and breakpoints, the
  fetch queue implementation, focus handling details (focus moves into the
  panel on open and back to the row on close), relative-time formatting.
- How `FakeMarket` in server tests returns news.
