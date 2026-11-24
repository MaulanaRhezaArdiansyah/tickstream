import type {
  CandleRange,
  CandleSeries,
  CompanyNews,
  MarketStatus,
  Profile,
  Quote,
  SymbolInfo,
} from '../types'

export class ApiError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
}

/** Pesan yang layak ditampilkan ke user dari error apa pun. */
export function errorMessage(err: unknown): string {
  return err instanceof ApiError ? err.message : 'gagal menghubungi server'
}

// Server membalas 429 kalau antrean limiter Finnhub penuh; biasanya lega lagi dalam 1-2 detik.
const RETRY_DELAY_MS = 2000
const MAX_RETRIES = 5

interface RequestOptions {
  method?: 'GET' | 'POST' | 'DELETE'
  body?: unknown
  signal?: AbortSignal
  /** Token pemilik, dikirim sebagai `Authorization: Bearer ...`. */
  token?: string
}

async function request<T>(
  path: string,
  { method = 'GET', body, signal, token }: RequestOptions = {},
) {
  const headers: Record<string, string> = {}
  const init: RequestInit = { method, signal, headers }
  if (body !== undefined) {
    init.body = JSON.stringify(body)
    headers['Content-Type'] = 'application/json'
  }
  if (token) headers.Authorization = `Bearer ${token}`

  for (let attempt = 0; ; attempt++) {
    const res = await fetch(path, init)
    if (res.ok) {
      // 204 No Content nggak punya body untuk di-parse.
      return (res.status === 204 ? undefined : await res.json()) as T
    }

    if (res.status === 429 && attempt < MAX_RETRIES) {
      await sleep(RETRY_DELAY_MS, signal)
      continue
    }

    // Error dari server kita berbentuk {"error": "..."}; selain itu pakai status HTTP.
    const errorBody = (await res.json().catch(() => null)) as {
      error?: string
    } | null
    throw new ApiError(res.status, errorBody?.error ?? `HTTP ${res.status}`)
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
  request<Quote>(`/api/quote?${symbolParam(symbol)}`, { signal })

export const fetchProfile = (symbol: string, signal?: AbortSignal) =>
  request<Profile>(`/api/profile?${symbolParam(symbol)}`, { signal })

export const fetchNews = (symbol: string, signal?: AbortSignal) =>
  request<CompanyNews>(`/api/news?${symbolParam(symbol)}`, { signal })

export const fetchCandles = (symbol: string, range: CandleRange, signal?: AbortSignal) =>
  request<CandleSeries>(
    `/api/candles?${symbolParam(symbol)}&range=${encodeURIComponent(range)}`,
    { signal },
  )

export const fetchMarketStatus = (signal?: AbortSignal) =>
  request<MarketStatus>('/api/market-status', { signal })

export const fetchSymbols = (signal?: AbortSignal) =>
  request<SymbolInfo[]>('/api/symbols', { signal })

export const fetchWatchlist = (signal?: AbortSignal) =>
  request<string[]>('/api/watchlist', { signal })

/** 204 kalau token pemilik diterima; ApiError 401 kalau nggak. */
export const checkOwnerToken = (token: string, signal?: AbortSignal) =>
  request<void>('/api/admin', { token, signal })

/** Mengembalikan simbol yang sudah dinormalisasi server (mis. "nvda" → "NVDA"). */
export const addToWatchlist = (symbol: string, token: string) =>
  request<string>('/api/watchlist', { method: 'POST', body: { symbol }, token })

export const removeFromWatchlist = (symbol: string, token: string) =>
  request<void>(`/api/watchlist/${encodeURIComponent(symbol)}`, {
    method: 'DELETE',
    token,
  })
