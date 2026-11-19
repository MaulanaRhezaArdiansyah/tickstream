# Company News Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Each board row shows a 24 h news count, and clicking a row opens a side panel with that stock's latest news from Finnhub's free `/company-news`.

**Architecture:** The server gets `MarketDataSource::company_news`. `FinnhubRest` implements it behind a news-only rate limiter, `MarketCache` caches it for 15 minutes, and `GET /api/news` serves it for watchlist symbols only. A pure `prepare_news` function filters, counts, sorts and caps the articles. The web gets a `useNews` hook that fetches symbols one at a time once quotes have settled, a badge on `QuoteRow`, and a `NewsPanel` beside the board.

**Tech Stack:** Rust (Axum, governor, moka, serde), React 19 + TS, Vitest + Testing Library.

**Spec:** `docs/superpowers/specs/2026-11-20-company-news-design.md`

## Global Constraints

- Finnhub endpoint: `GET /company-news?symbol=…&from=YYYY-MM-DD&to=YYYY-MM-DD` (free). `from` = 7 days before today (UTC), `to` = today (UTC).
- No premium endpoints. The API key never reaches the browser.
- Response: `{"count24h": <n>, "items": [{"id","datetime","headline","source","summary","url"}]}`. `datetime` is Unix **ms**.
- Drop items whose `url` is not `http://` / `https://` or whose headline is empty (after trim).
- `count24h` = items with `datetime >= now - 24 h`, counted after the filter and before the cap.
- Items are newest first, max 20.
- Cache: 15 minutes per symbol in `MarketCache`. Errors are not cached.
- News limiter: 20 per minute, burst 5. It is acquired before the global limiter, within the same `MAX_WAIT` (2 s), else `MarketError::RateLimited`.
- `/api/news` serves watchlist symbols only (`watched_symbol`). No news → `{"count24h":0,"items":[]}`.
- Web: badge hidden when `count24h` is 0. Badge text e.g. `6 berita`.
- Web: news is fetched one symbol at a time, after every symbol's quote has settled. Opening the panel fetches that symbol immediately. Refresh every 15 minutes.
- Panel states and copy:
  - loading: `Memuat berita…`
  - empty: `Belum ada berita 7 hari terakhir`
  - error: the message plus a `Coba lagi` button
- Links: `target="_blank" rel="noopener noreferrer"`.
- Panel layout: summary clamped to 3 lines. Panel about 40% wide on desktop, full screen at ≤ 560px with a `Tutup` button.
- Panel behaviour:
  - Escape, ✕, or clicking the selected row closes it.
  - Another row switches it.
  - A symbol removed from the watchlist closes it.
- UI copy is Indonesian; comments and test names are Indonesian, as in the codebase.
- Commit trailers on every commit:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01SqsktYGqFFRqUUhoQw1Me6
  ```

## Review Focus

1. **Finnhub article missing `summary`, `source` or `image`, or with `null` fields.** One sloppy article must not fail the whole list; it is kept with empty strings. *Test: Task 1 `missing_fields_default_to_empty`.*
2. **One symbol's news request fails (429, 502).** The browser queue must carry on with the other symbols and retry the failed one on the next refresh, not stall. *Test: Task 4 `satu gagal, antrean tetap jalan`.*
3. **Switching the panel from AMD to NVDA while AMD is still loading.** NVDA's panel must never show AMD's articles. *Test: Task 4 `hasil disimpan per simbol` and Task 5 `panel menampilkan berita simbolnya sendiri`.*
4. **Malicious or odd links** (`javascript:alert(1)`, `ftp://…`, blank headline). These are dropped on the server, so they never render as clickable. *Test: Task 1 `unsafe_links_and_blank_headlines_are_dropped`.*
5. **An open symbol removed from the watchlist by the owner.** The panel must close rather than show a stale symbol. *Test: Task 5 `panel tertutup kalau simbolnya dihapus`.*

---

### Task 1: News shaping (pure server logic)

**Files:**
- Create: `server/src/market/news.rs` (declared as `mod news;` with `pub use news::{CompanyNews, NewsArticle, NewsItem, iso_date, prepare_news};` in `server/src/market/mod.rs`)

**Interfaces:**
- Produces:
  - `#[derive(Deserialize)] pub struct NewsArticle { id: i64, datetime: i64 /* Unix s */, headline: String, source: String, summary: String, url: String }`. Every field is `#[serde(default)]`. A `null` string deserializes to `""` (use `deserialize_with` and an `Option<String>` → `unwrap_or_default` helper). Unknown fields are ignored.
  - `#[derive(Clone, Serialize, Debug, PartialEq)] pub struct NewsItem { id, datetime /* ms */, headline, source, summary, url }`
  - `#[derive(Clone, Serialize, Debug, PartialEq)] #[serde(rename_all = "camelCase")] pub struct CompanyNews { #[serde(rename = "count24h")] pub count_24h: usize, pub items: Vec<NewsItem> }`
  - `pub fn prepare_news(articles: Vec<NewsArticle>, now_ms: i64) -> CompanyNews`
  - `pub fn iso_date(unix_secs: i64) -> String`. Returns UTC `YYYY-MM-DD` with no new dependency; use Howard Hinnant's `civil_from_days`.

- [ ] **Step 1: Write the failing tests** in a `#[cfg(test)] mod tests` in `news.rs`, with an `article(id, datetime_s, headline, url)` helper:
  - `newest_first_and_capped_at_20`: 25 articles at `datetime` 1..=25 → 20 items, the first with `datetime == 25_000`, the last `6_000`.
  - `count_24h_is_taken_before_the_cap`: `now_ms = 100_000_000`; 30 articles at `now - 1 h`, 2 at `now - 25 h` → `count_24h == 30`, `items.len() == 20`.
  - `exactly_24h_old_counts`: one article at `now_ms - 86_400_000` → `count_24h == 1`.
  - `unsafe_links_and_blank_headlines_are_dropped`:
    - urls `javascript:alert(1)`, `ftp://x`, `""` → dropped;
    - headline `"   "` → dropped;
    - `https://a` and `http://b` → kept (2 items).
  - `missing_fields_default_to_empty`: `serde_json::from_str::<Vec<NewsArticle>>(r#"[{"id":1,"datetime":10,"headline":"H","url":"https://x","summary":null}]"#)` parses; after `prepare_news`, `source == ""` and `summary == ""`.
  - `iso_date_formats_utc_days`:
    - `iso_date(0) == "1970-01-01"`
    - `iso_date(1_709_164_800) == "2024-02-29"`
    - `iso_date(1_795_132_800) == "2026-11-20"`, `iso_date(1_795_132_799) == "2026-11-19"`

- [ ] **Step 2: Run** `cd server && cargo test news` → expected: compile errors or FAIL (module missing).
- [ ] **Step 3: Implement** the types and both functions in `news.rs`.
- [ ] **Step 4: Run** `cd server && cargo test news` → expected: 6 passed. Then `cargo fmt && SQLX_OFFLINE=true cargo clippy -q --all-targets --locked -- -D warnings`.
  - The items are unused until Task 2. If clippy flags dead code, put `#![cfg_attr(not(test), allow(dead_code))]` at the top of `news.rs` and remove it in Task 2. Ledger it.
- [ ] **Step 5: Commit** `Shape Finnhub company news: filter, count, sort, cap`.

### Task 2: Fetch, rate-limit and cache company news

**Files:**
- Modify: `server/src/market/mod.rs`
  - Trait gains `async fn company_news(&self, symbol: &str) -> Result<CompanyNews, MarketError>;`
- Modify: `server/src/finnhub/rest.rs`
  - New `news_limiter: DefaultDirectRateLimiter` field built from `NEWS_QUOTA = Quota::per_minute(20).allow_burst(5)`.
  - `company_news` impl: acquire the news limiter within `MAX_WAIT` (else `RateLimited`), then call the existing `get_json`, which takes the global one.
  - `from` / `to` come from `iso_date(now_s - 7 * 86_400)` / `iso_date(now_s)` with `now` from `SystemTime::now()`.
  - Result: `prepare_news(articles, now_ms)`.
- Modify: `server/src/market/cache.rs`
  - New `NEWS_TTL = 15 min` and a `news: Cache<String, CompanyNews>` field, built the same way as `quotes`.
  - Add `company_news` to `CountingSource` (counting calls).
- Modify: `server/src/routes/tests.rs`
  - `FakeMarket::company_news` returns `CompanyNews { count_24h: 1, items: vec![one NewsItem { id: 1, datetime: 1_000, headline: format!("{symbol} news"), source: "Test".into(), summary: String::new(), url: "https://example.com".into() }] }`.

**Interfaces:**
- Consumes: Task 1's `CompanyNews`, `NewsArticle`, `prepare_news`, `iso_date`.
- Produces: `MarketDataSource::company_news(&self, symbol: &str) -> Result<CompanyNews, MarketError>` on every source, cached in `MarketCache`.

- [ ] **Step 1: Write the failing tests**
  - In `cache.rs`, `news_is_cached_per_symbol`:
    - `company_news("AAPL")` twice and `("NVDA")` once → source calls == 2;
    - the second AAPL result equals the first.
  - In `rest.rs` `#[cfg(test)]`, `news_limiter_allows_a_burst_of_5_then_rejects` (`#[tokio::test]`):
    - build `FinnhubRest::new(&SecretString::from("test"))`;
    - call a private `async fn acquire_news(&self) -> Result<(), MarketError>` (the limiter step only, no HTTP) 5 times → all `Ok`;
    - the 6th → `Err(MarketError::RateLimited)` (it waits about 2 s; acceptable).
- [ ] **Step 2: Run** `cd server && cargo test news` → expected: compile errors (method missing).
- [ ] **Step 3: Implement** the trait method, `acquire_news`, the Finnhub `company_news`, the cache method, and the two fakes. If Task 1 added a dead_code allow, remove it here.
- [ ] **Step 4: Run** the full `cd server && cargo test` → expected: all pass. Then fmt + clippy as in Task 1.
- [ ] **Step 5: Commit** `Fetch company news with its own rate limit and a 15-minute cache`.

### Task 3: `GET /api/news`

**Files:**
- Modify: `server/src/routes/market.rs`
  - `pub async fn news(State, AppQuery<SymbolQuery>) -> Result<Json<CompanyNews>, AppError>`, using `watched_symbol` the same way as `quote`.
- Modify: `server/src/routes/mod.rs`
  - `.route("/api/news", get(market::news))`
- Modify: `server/src/routes/tests.rs`
- Modify: `README.md`
  - API table row: `` | `GET /api/news?symbol=AAPL` | Latest company news (7 days, max 20) and the 24 h count; watchlist symbols only | ``

**Interfaces:**
- Consumes: Task 2's `company_news`.
- Produces: the HTTP contract in Global Constraints, used by Task 4.

- [ ] **Step 1: Write the failing test** `news_is_served_only_for_watched_symbols`, following `quotes_are_served_only_for_watched_symbols`:
  - `/api/news?symbol=aapl` → 200 with body `{"count24h":1,"items":[{"id":1,"datetime":1000,"headline":"AAPL news","source":"Test","summary":"","url":"https://example.com"}]}`;
  - `?symbol=AMD` (known but not watched) → 404;
  - `?symbol=` → 400.
- [ ] **Step 2: Run** `cd server && cargo test news_is_served` → expected: FAIL (404 from the `/api/{*rest}` fallback).
- [ ] **Step 3: Implement** the handler, the route, and the README row.
- [ ] **Step 4: Run** the full `cargo test` → all pass. Then fmt + clippy.
- [ ] **Step 5: Commit** `Serve company news for watchlist symbols at /api/news`.

### Task 4: Web data: `fetchNews`, `relativeTime`, `useNews`

**Files:**
- Modify: `web/src/types.ts`
  - `export interface NewsItem { id: number; datetime: number; headline: string; source: string; summary: string; url: string }`
  - `export interface CompanyNews { count24h: number; items: NewsItem[] }`
- Modify: `web/src/api/client.ts`
  - `export const fetchNews = (symbol: string, signal?: AbortSignal) => request<CompanyNews>(`/api/news?${symbolParam(symbol)}`, { signal })`
- Create: `web/src/lib/relativeTime.ts` and `relativeTime.test.ts`
- Create: `web/src/hooks/useNews.ts` and `useNews.test.ts`

**Interfaces:**
- Produces:
  - `relativeTime(ms: number, now: number): string`:
    - < 1 min → `baru saja`
    - < 60 min → `N menit lalu`
    - < 24 h → `N jam lalu`
    - < 48 h → `kemarin`
    - else → `N hari lalu`
  - `type NewsState = { status: 'loading' } | { status: 'ok'; news: CompanyNews } | { status: 'error'; message: string }`
  - `useNews(symbols: readonly string[], ready: boolean): { news: Readonly<Record<string, NewsState>>; load: (symbol: string) => void }`
    - Nothing is fetched until `ready` is true.
    - Then the hook fetches symbols with no state yet, **one in flight at a time**, in `symbols` order.
    - `load(symbol)` fetches that symbol right away, outside the queue, setting `loading` first. It is used by the panel on open and by `Coba lagi`.
    - Every 15 minutes (`NEWS_REFRESH_MS = 15 * 60_000`) every symbol is fetched again through the same queue. The old `ok` state stays visible while refetching, so the badge doesn't blink.
    - An error sets `error` and the queue moves on.
    - Aborted on unmount.
- `useNews` is consumed by Task 5.

- [ ] **Step 1: Write the failing tests**
  - `relativeTime.test.ts`, with `now = 1_000_000_000`:
    - `now - 30_000` → `baru saja`
    - `now - 5 * 60_000` → `5 menit lalu`
    - `now - 2 * 3_600_000` → `2 jam lalu`
    - `now - 30 * 3_600_000` → `kemarin`
    - `now - 3 * 86_400_000` → `3 hari lalu`
  - `useNews.test.ts`. Mock with `vi.mock('../api/client', …)`: `fetchNews` is a `vi.fn` returning controllable deferred promises; `errorMessage` stays real.
    - `belum fetch sebelum ready`: `ready=false` → `fetchNews` not called.
    - `satu per satu`:
      - `['AAPL','AMD']`, ready → exactly 1 call (AAPL);
      - resolve it → the 2nd call is AMD;
      - `news.AAPL` is `{status:'ok', news}`.
    - `satu gagal, antrean tetap jalan`: AAPL rejects with `new ApiError(429, …)` → `news.AAPL.status === 'error'` and AMD is fetched next.
    - `load langsung, tanpa antre`: while AAPL is pending, `load('AMD')` → `fetchNews` called with `'AMD'` immediately; `news.AMD.status === 'loading'`.
    - `hasil disimpan per simbol`: AAPL and AMD resolve with different headlines → each state holds its own headline.
    - `refresh tiap 15 menit` (`vi.useFakeTimers`): after both are ok, advance `15 * 60_000` → `fetchNews` called again for AAPL, and `news.AAPL` stays `ok` while pending.
- [ ] **Step 2: Run** `cd web && npm test` → expected: new tests FAIL (modules missing).
- [ ] **Step 3: Implement** the types, `fetchNews`, `relativeTime` and `useNews`.
- [ ] **Step 4: Run** `cd web && npm test && npm run -s lint && npm run -s build` → all green.
- [ ] **Step 5: Commit** `Fetch company news in the browser, one symbol at a time`.

### Task 5: Badge, side panel, wiring

**Files:**
- Modify: `web/src/components/QuoteRow.tsx` and `QuoteRow.test.tsx`
- Create: `web/src/components/NewsPanel.tsx` and `NewsPanel.test.tsx`
- Modify: `web/src/components/QuoteBoard.tsx`, `web/src/App.tsx`, `web/src/index.css`
- Create: `web/src/lib/selection.ts` and `selection.test.ts`

**Interfaces:**
- Consumes: Task 4's `useNews`, `NewsState`, `relativeTime`.
- Produces:
  - `QuoteRow` gains props:
    - `newsCount?: number` — badge `{n} berita` when > 0;
    - `selected?: boolean` — row gets class `selected` and `aria-current="true"`;
    - `onSelect?: (symbol: string) => void` — must be stable.
  - The ticker/name cell becomes `<button type="button" className="who" aria-label={`Berita ${symbol}`}>`, so keyboard users get Enter/Space for free. Mouse clicks anywhere on the row also call `onSelect`. The remove button calls `event.stopPropagation()`.
  - `NewsPanel({ symbol, name, price, change, pct, state, onClose, onRetry }: …)` renders `<aside aria-label={`Berita ${symbol}`}>`, containing:
    - a header with the name, price and change, and a close button `aria-label="Tutup berita"` showing ✕ (text `Tutup` at ≤ 560px via CSS);
    - an empty `<div className="chart-slot" />` reserved for feature 2;
    - the list in the states from Global Constraints. Each item is: meta line `{relativeTime} · {source}`, an `<a>` headline with `target="_blank" rel="noopener noreferrer"`, and the summary in a `<p className="clamp-3">`.
  - It listens for `keydown` Escape on `document` → `onClose`.
  - On mount it focuses the close button. On close, App returns focus to the row's button.
  - `visibleSelection(selected: string | null, symbols: readonly string[]): string | null` → `selected` when it is in `symbols`, else `null`.
  - App:
    - holds `selected` state;
    - `ready = symbols.every(s => snapshots[s]?.quote || snapshots[s]?.error)`;
    - calls `useNews(symbols, ready)`;
    - opening calls `load(symbol)` when that symbol's state is not `ok`;
    - renders the board and the panel in a `.workspace` grid (board | panel) when a symbol is open.
  - QuoteBoard passes `newsCount`, `selected` and `onSelect` through.

- [ ] **Step 1: Write the failing tests**
  - `QuoteRow.test.tsx`:
    - `badge berita`: `newsCount={6}` → text `6 berita` visible; `newsCount={0}` → no `berita` text.
    - `klik baris memilih simbol`: click the `Berita AAPL` button → `onSelect` called with `'AAPL'`.
    - `hapus nggak ikut memilih`: with `onRemove` and `onSelect`, click the remove button → `onRemove` called, `onSelect` not called.
  - `NewsPanel.test.tsx`, with `now` = `Date.now()`:
    - `memuat`: state `{status:'loading'}` → `Memuat berita…`.
    - `daftar berita`: an ok state with one item → link name = headline, `href` = its url, `target` `_blank`, `rel` `noopener noreferrer`; source text visible.
    - `kosong`: ok state with `items: []` → `Belum ada berita 7 hari terakhir`.
    - `gagal`: error state `'Kuota habis'` → message visible; clicking `Coba lagi` calls `onRetry`.
    - `tutup`: clicking `Tutup berita` → `onClose`; `fireEvent.keyDown(document, {key:'Escape'})` → `onClose` again.
    - `panel menampilkan berita simbolnya sendiri`: rerender from symbol `AMD` (headline `AMD x`) to `NVDA` with state `{status:'loading'}` → `AMD x` no longer in the document.
  - `selection.test.ts`, `panel tertutup kalau simbolnya dihapus`:
    - `visibleSelection('AMD', ['AAPL']) === null`
    - `visibleSelection('AMD', ['AMD']) === 'AMD'`
    - `visibleSelection(null, ['AMD']) === null`
- [ ] **Step 2: Run** `cd web && npm test` → expected: the new tests FAIL.
- [ ] **Step 3: Implement** the components, App wiring and CSS:
  - `.workspace` is a 2-column grid when open (`minmax(0,1fr) minmax(320px, 40%)`). The panel is sticky at the top with its own scroll.
  - At ≤ 560px the panel is `position: fixed; inset: 0` and full screen.
  - Add `.clamp-3`, the badge, `.row.selected`, and `overflow-wrap: anywhere` on headlines and summaries.
  - Follow the existing tokens in `index.css`.
- [ ] **Step 4: Run** `cd web && npm test && npm run -s lint && npm run -s build` → all green.
- [ ] **Step 5: Look at it.** Run the Vite dev server against prod's public API through the existing dev proxy. Read-only REST is fine. Do **not** run the local Rust server: it opens a second Finnhub socket on the prod key. Check desktop and 390px width with Claude in Chrome: badge, panel open/switch/close, Escape, mobile full screen.
  - If the dev proxy cannot reach prod, ledger that and check after deploy instead.
- [ ] **Step 6: Commit** `Show a news badge per row and a news side panel`.
