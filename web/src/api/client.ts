import type { MarketStatus, Profile, Quote } from '../types'

export class ApiError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
}

// Server membalas 429 kalau antrean limiter Finnhub penuh; biasanya lega lagi dalam 1-2 detik.
const RETRY_DELAY_MS = 2000
const MAX_RETRIES = 5

async function getJson<T>(path: string, signal?: AbortSignal): Promise<T> {
  for (let attempt = 0; ; attempt++) {
    const res = await fetch(path, { signal })
    if (res.ok) return (await res.json()) as T

    if (res.status === 429 && attempt < MAX_RETRIES) {
      await sleep(RETRY_DELAY_MS, signal)
      continue
    }

    // Error dari server kita berbentuk {"error": "..."}; selain itu pakai status HTTP.
    const body = (await res.json().catch(() => null)) as { error?: string } | null
    throw new ApiError(res.status, body?.error ?? `HTTP ${res.status}`)
  }
}

/** setTimeout versi Promise yang ikut batal kalau `signal` di-abort. */
function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(resolve, ms)
    signal?.addEventListener(
      'abort',
      () => {
        clearTimeout(timer)
        reject(signal.reason)
      },
      { once: true },
    )
  })
}

const symbolParam = (symbol: string) => `symbol=${encodeURIComponent(symbol)}`

export const fetchQuote = (symbol: string, signal?: AbortSignal) =>
  getJson<Quote>(`/api/quote?${symbolParam(symbol)}`, signal)

export const fetchProfile = (symbol: string, signal?: AbortSignal) =>
  getJson<Profile>(`/api/profile?${symbolParam(symbol)}`, signal)

export const fetchMarketStatus = (signal?: AbortSignal) =>
  getJson<MarketStatus>('/api/market-status', signal)
