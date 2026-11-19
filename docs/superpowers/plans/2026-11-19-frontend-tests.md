# Frontend Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Vitest suite for the web app's pure logic and the quote row, run by `npm test` and by CI.

**Architecture:** Vitest reads the existing Vite config. Tests live next to the code as `*.test.ts(x)`. Logic tests run in the default Node environment; the one component test file opts into jsdom with a file comment. No global test APIs: every file imports from `vitest`.

**Tech Stack:** Vitest, @testing-library/react (+ @testing-library/dom), jsdom. React 19, TypeScript 7, Vite 8.

**Spec:** `docs/superpowers/specs/2026-11-19-frontend-tests-design.md`

## Global Constraints

- Repo: WSL `~/projects/us-stock`, web app in `web/`. Run commands there through WSL, not on the Windows copy.
- Code is already written: these are characterization tests. "Run it to make sure it fails" is replaced by "break the code on purpose, see the test fail, undo".
- Formatting: `npx prettier --no-semi --single-quote --print-width 100`. `npm run -s lint` and `npm run -s build` must stay clean (the build type-checks test files too, since `tsconfig.app.json` includes `src`).
- Test names and comments in Indonesian, like the rest of the code; commit messages in English with the session trailers:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01SqsktYGqFFRqUUhoQw1Me6`.
- No `@testing-library/jest-dom`: plain `expect` on query results (`toBeNull()`, `.textContent`).
- No snapshot tests.

## Review Focus

- A price equal to the previous one must not bump `flashSeq` (the row would re-flash for no change). → Task 2.
- Friday evening and weekend countdowns must point at Monday pre-market, and the WIB labels must shift by an hour in US summer. → Task 3.
- A visitor (no `onRemove`) must never see a remove button. → Task 5.
- A logo URL that fails to load must fall back to the ticker letter, not a broken image. → Task 5.
- Search input with surrounding spaces or lower case must match like the trimmed upper-case query. → Task 4.

---

### Task 1: Vitest setup, format tests, CI

**Files:**
- Modify: `web/package.json` (devDependencies, `"test": "vitest run"`)
- Modify: `web/vite.config.ts` (import `defineConfig` from `vitest/config`; no `test` block needed)
- Create: `web/src/lib/format.test.ts`
- Modify: `.github/workflows/ci.yml` (Web job: step `Test` running `npm test` between Lint and Build)
- Modify: `README.md` (Tests section: `npm run lint && npm test && npm run build`)

**Interfaces:**
- Produces: `npm test` (runs once, exits non-zero on failure) used by every later task.

- [ ] **Step 1: Install** `npm i -D vitest @testing-library/react @testing-library/dom jsdom` in `web/`; add the `test` script.
- [ ] **Step 2: Write `format.test.ts`**

```ts
import { describe, expect, it } from 'vitest'
import { fmt, fmtVol, sign, splitSymbol } from './format'

describe('fmt', () => {
  it('dua desimal dengan pemisah ribuan', () => expect(fmt(1234.5)).toBe('1,234.50'))
  it('kosong jadi strip', () => {
    expect(fmt(null)).toBe('–')
    expect(fmt(undefined)).toBe('–')
    expect(fmt(Number.NaN)).toBe('–')
  })
})
it('sign: plus untuk nol dan positif', () => {
  expect(sign(0)).toBe('+')
  expect(sign(2)).toBe('+')
  expect(sign(-2)).toBe('')
})
it('fmtVol ringkas', () => {
  expect(fmtVol(250)).toBe('250')
  expect(fmtVol(49_600)).toBe('49.6K')
  expect(fmtVol(1_234_567)).toBe('1.2M')
})
it('splitSymbol', () => {
  expect(splitSymbol('AAPL')).toEqual({ ticker: 'AAPL', venue: null })
  expect(splitSymbol('BINANCE:BTCUSDT')).toEqual({ ticker: 'BTCUSDT', venue: 'Binance' })
})
```

- [ ] **Step 3: Run** `npm test` → `Test Files 1 passed`. Change `'–'` in `fmt` to `'-'`, run again → 1 failed; undo.
- [ ] **Step 4: Add the CI step and README line; run** `npm run -s lint && npm run -s build` → clean.
- [ ] **Step 5: Commit** "Add Vitest and tests for number formatting".

### Task 2: tickStore

**Files:**
- Create: `web/src/lib/tickStore.test.ts`
- Modify: `web/src/lib/tickStore.ts` (doc comment of `flash` only: it keeps the last direction when the price repeats; `flashSeq` is what signals a new change)

**Interfaces:**
- Consumes: `TickStore` (`apply(trades)`, `subscribe`, `getSnapshot`, `takeTradeCount`, `dispose`), `Trade` from `../types` (`{ symbol, price, timestamp, volume }`).

`requestAnimationFrame` does not exist in Node: in `beforeEach`, `vi.stubGlobal('requestAnimationFrame', cb => { frames.push(cb); return frames.length })` and stub `cancelAnimationFrame`; a helper `nextFrame()` runs and clears the queued callbacks. `vi.unstubAllGlobals()` in `afterEach`.

- [ ] **Step 1: Write the tests**
  - `belum ada snapshot sebelum frame berikutnya`: after `apply([t('AAPL', 10)])`, `getSnapshot()` is `{}`; after `nextFrame()`, `getSnapshot().AAPL.price === 10`.
  - `satu frame untuk banyak batch`: two `apply` calls before a frame → `frames.length === 1`; listener subscribed via `subscribe` is called once after `nextFrame()`.
  - `naik, turun, sama`: prices 10 → 11 → 9 → 9 (each `apply` separately, one `nextFrame()` at the end) gives `flash === 'down'`, `flashSeq === 2`, `high === 11`, `low === 9`, `volume` = sum of the four volumes, `points` = `[10, 11, 9, 9]`.
  - `simbol lain tetap referensi yang sama`: after a frame with AAPL and MSFT, a second frame with only AAPL leaves `getSnapshot().MSFT` `toBe` the earlier object.
  - `sparkline maksimal 300 titik`: 301 trades priced 0..300 → `points.length === 300`, `points[0] === 1`.
  - `takeTradeCount mereset`: after 3 trades → `3`, then `0`.
  - `dispose membatalkan frame`: `apply` then `dispose()` → `cancelAnimationFrame` stub called with the frame id.
- [ ] **Step 2: Run** `npm test` → pass. Remove `if (trade.price !== draft.price)` guard temporarily → `naik, turun, sama` fails; undo.
- [ ] **Step 3: Fix the `flash` doc comment.** Lint, build.
- [ ] **Step 4: Commit** "Test the tick store".

### Task 3: US session and countdowns

**Files:**
- Create: `web/src/lib/session.test.ts`

**Interfaces:**
- Consumes: `usSession(now: Date): Session` (`phase`, `position`, `minutesToNext`, `wib`), `fmtDuration(minutes: number): string`.

All dates are absolute UTC instants (`new Date('…Z')`); no fake timers needed. Winter = November 2026 (New York UTC−5), summer = July 2026 (UTC−4).

- [ ] **Step 1: Write the tests** (a `it.each` table is fine)

| UTC instant | New York | phase | minutesToNext | other |
|---|---|---|---|---|
| `2026-11-19T09:00:00Z` | Thu 04:00 | `pre` | 330 | `position` 0 |
| `2026-11-19T14:30:00Z` | Thu 09:30 | `regular` | 390 | |
| `2026-11-19T14:44:00Z` | Thu 09:44 | `regular` | 376 | `position` ≈ 0.3583 (`toBeCloseTo(344 / 960)`) |
| `2026-11-19T21:00:00Z` | Thu 16:00 | `post` | 240 | |
| `2026-11-20T01:00:00Z` | Thu 20:00 | `closed` | 480 | `position` null |
| `2026-11-21T01:00:00Z` | Fri 20:00 | `closed` | 3360 | next is Monday |
| `2026-11-22T15:00:00Z` | Sun 10:00 | `closed` | 1080 | |
| `2026-11-23T08:00:00Z` | Mon 03:00 | `closed` | 60 | |

  Plus: `wib` in winter (`2026-11-19T14:44:00Z`) is `['16:00', '21:30', '04:00', '08:00']`; in summer (`2026-07-15T13:30:00Z`, NY 09:30, phase `regular`) it is `['15:00', '20:30', '03:00', '07:00']`.
  `fmtDuration`: `45 → '45 m'`, `260 → '4 j 20 m'`, `3000 → '2 hari 2 j'`, `3360 → '2 hari 8 j'`.
- [ ] **Step 2: Run** `npm test` → pass. Change `REGULAR_OPEN` to `9 * 60` → failures; undo.
- [ ] **Step 3: Commit** "Test the US session schedule in Jakarta time".

### Task 4: Symbol search ranking

**Files:**
- Create: `web/src/lib/symbolSearch.test.ts`

**Interfaces:**
- Consumes: `searchSymbols(list: readonly SymbolInfo[], query: string, limit: number): { items: SymbolInfo[]; total: number }`; `SymbolInfo` = `{ symbol, name, kind: 'stock' | 'etf' }`.

Fixture, sorted by symbol like the server's list: `AAPL APPLE INC`, `AMD ADVANCED MICRO DEVICES INC`, `AMDX AMDX CORP`, `BRK.A BERKSHIRE HATHAWAY INC CL A`, `BRK.B BERKSHIRE HATHAWAY INC CL B`, `BRKR BRUKER CORP`, `PNPL PINEAPPLE INC`, `SPY SPDR S&P 500 ETF` (kind `etf`).

- [ ] **Step 1: Write the tests** (compare `items.map(i => i.symbol)`)
  - `'AMD'` → `['AMD', 'AMDX']` (exact ticker first).
  - `'BRK'` → `['BRK.A', 'BRK.B', 'BRKR']`.
  - `'apple'` → `['AAPL', 'PNPL']` (name word start before substring).
  - `'  brk  '` → same as `'BRK'`.
  - `'BERKSHIRE'` with `limit` 1 → `items` length 1, `total` 2.
  - `''` with `limit` 3 → first three symbols, `total` 8.
  - `'ZZZ'` → `items` `[]`, `total` 0.
- [ ] **Step 2: Run** `npm test` → pass. Swap ranks 2 and 3 in `rankOf` → `'apple'` fails; undo.
- [ ] **Step 3: Commit** "Test symbol search ranking".

### Task 5: QuoteRow from the user's side

**Files:**
- Create: `web/src/components/QuoteRow.test.tsx` (first line `// @vitest-environment jsdom`)

**Interfaces:**
- Consumes: `QuoteRow` props `{ symbol: string; snapshot: Snapshot | undefined; live: LiveTick | undefined; onRemove?: (symbol: string) => void }`; `Snapshot` = `{ quote?: Quote; name?: string; logo?: string | null; error?: string }`.

Render inside `<ul>`. No test globals, so call `cleanup()` from `@testing-library/react` in `afterEach`. Quote fixture: `{ price: 110, open: 100, high: 112, low: 99, prevClose: 100, timestamp: 0 }`. jsdom has no canvas; `Sparkline` already returns early when `getContext` gives null, so the jsdom "not implemented" notice is expected noise.

- [ ] **Step 1: Write the tests**
  - `pengunjung nggak melihat tombol hapus`: no `onRemove` → `screen.queryByRole('button', { name: 'Hapus AAPL dari watchlist' })` is null.
  - `pemilik bisa menghapus`: with `onRemove = vi.fn()`, clicking that button (`fireEvent.click`) calls it with `'AAPL'`.
  - `harga dan perubahan`: shows `'110.00'` and text matching `/\+10\.00%/`.
  - `logo gagal dimuat jadi huruf`: snapshot `logo: 'https://example.com/a.png'` → `container.querySelector('img.logo')` exists; after `fireEvent.error(img)`, the img is gone and `.logo-letter` has text `'A'`.
  - `crypto tanpa logo`: symbol `BINANCE:BTCUSDT`, snapshot `{ quote }` → `.logo-letter` text `'B'`, text `'Binance'` and `'BTCUSDT'` shown.
  - `error tanpa harga`: snapshot `{ error: 'simbol XYZ tidak ditemukan' }`, no live → that text is shown.
- [ ] **Step 2: Run** `npm test` → all files pass. Remove `onError` from `CompanyLogo` → the logo test fails; undo.
- [ ] **Step 3: Lint, build; commit** "Test the quote row as a visitor and as the owner".
- [ ] **Step 4: Push and watch CI** (`gh run watch`): Web job shows a passing `Test` step; deploy succeeds.
