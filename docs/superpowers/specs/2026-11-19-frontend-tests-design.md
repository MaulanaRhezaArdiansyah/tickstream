# Frontend tests (web)

## Why

The server has 32 tests; the web app has none, and CI only lints and builds it. The next
features (sparkline history, news, metrics, intraday charts) all touch the web app, so the
logic that is easiest to break silently should be pinned down first. This is also a way to
learn how React code is tested.

## Goals

- `npm test` in `web/` runs a Vitest suite; CI's Web job runs it before the build.
- Pure logic in `src/lib/` is covered:
  - `tickStore`: price changes, flash direction, high/low/volume, sparkline point limit,
    one snapshot per frame.
  - `session`: every phase and its boundaries in Jakarta time, US summer vs winter time,
    with the clock fixed inside each test.
  - `symbolSearch`: ranking of matches.
  - `format`: number, volume and symbol formatting.
- `QuoteRow` is tested from the user's side (Testing Library): owner vs visitor (remove
  button), logo fallback to the ticker's first letter, error row.

## Non-goals

- End-to-end tests in a real browser (Playwright).
- Tests for the WebSocket hook (`useLiveTicks`) and the fetch hooks: heavy mocking for
  little gain at this stage.
- Coverage targets.

## Approaches

1. **Vitest + Testing Library + jsdom (chosen).** Reuses the existing Vite config and
   TypeScript setup; Testing Library tests components by what a user sees.
2. Jest: needs its own transform setup for TypeScript/ESM next to Vite. No benefit here.
3. Playwright only: tests the real page, but slow, needs a running server and Finnhub data,
   and does not pin down the edge cases in `src/lib/`.

## Left to the builder

File layout (tests next to the code as `*.test.ts(x)`), exact cases, and how timers or
`requestAnimationFrame` are faked in the tickStore tests.
