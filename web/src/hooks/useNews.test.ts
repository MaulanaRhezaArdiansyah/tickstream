// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { ApiError, fetchNews } from '../api/client'
import type { CompanyNews } from '../types'
import { NEWS_REFRESH_MS, useNews } from './useNews'

vi.mock('../api/client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api/client')>()),
  fetchNews: vi.fn(),
}))

const mockFetch = vi.mocked(fetchNews)

/** Promise yang hasilnya ditentukan tes, supaya urutan antrean bisa diperiksa langkah demi langkah. */
function deferred() {
  let resolve!: (news: CompanyNews) => void
  let reject!: (err: unknown) => void
  const promise = new Promise<CompanyNews>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

const pending: Record<string, ReturnType<typeof deferred>[]> = {}

const news = (headline: string): CompanyNews => ({
  count24h: 1,
  items: [{ id: 1, datetime: 0, headline, source: 'Test', summary: '', url: 'https://x' }],
})

const SYMBOLS = ['AAPL', 'AMD'] as const

beforeEach(() => {
  for (const key of Object.keys(pending)) delete pending[key]
  mockFetch.mockReset()
  mockFetch.mockImplementation((symbol) => {
    const d = deferred()
    ;(pending[symbol] ??= []).push(d)
    return d.promise
  })
})

afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

const calls = () => mockFetch.mock.calls.map(([symbol]) => symbol)

it('belum fetch sebelum ready', () => {
  renderHook(() => useNews(SYMBOLS, false))
  expect(mockFetch).not.toHaveBeenCalled()
})

it('satu per satu', async () => {
  const { result } = renderHook(() => useNews(SYMBOLS, true))
  expect(calls()).toEqual(['AAPL'])

  await act(async () => pending.AAPL[0].resolve(news('Apple')))
  expect(calls()).toEqual(['AAPL', 'AMD'])
  expect(result.current.news.AAPL).toEqual({ status: 'ok', news: news('Apple') })
})

it('satu gagal, antrean tetap jalan', async () => {
  const { result } = renderHook(() => useNews(SYMBOLS, true))

  await act(async () => pending.AAPL[0].reject(new ApiError(429, 'Kuota habis')))
  expect(result.current.news.AAPL).toEqual({ status: 'error', message: 'Kuota habis' })
  expect(calls()).toEqual(['AAPL', 'AMD'])
})

it('load langsung, tanpa antre', () => {
  const { result } = renderHook(() => useNews(SYMBOLS, true))

  act(() => result.current.load('AMD'))
  expect(calls()).toEqual(['AAPL', 'AMD'])
  expect(result.current.news.AMD).toEqual({ status: 'loading' })
})

it('hasil disimpan per simbol', async () => {
  const { result } = renderHook(() => useNews(SYMBOLS, true))

  await act(async () => pending.AAPL[0].resolve(news('Apple')))
  await act(async () => pending.AMD[0].resolve(news('AMD')))
  expect(result.current.news.AAPL).toEqual({ status: 'ok', news: news('Apple') })
  expect(result.current.news.AMD).toEqual({ status: 'ok', news: news('AMD') })
})

it('watchlist berubah di tengah jalan nggak bikin macet', async () => {
  const { result, rerender } = renderHook(({ symbols }) => useNews(symbols, true), {
    initialProps: { symbols: SYMBOLS as readonly string[] },
  })
  expect(calls()).toEqual(['AAPL'])

  // Pemilik menghapus AMD selagi berita AAPL masih diambil: AAPL diambil ulang, bukan dilewati.
  rerender({ symbols: ['AAPL'] })
  expect(calls()).toEqual(['AAPL', 'AAPL'])
  await act(async () => pending.AAPL[1].resolve(news('Apple')))
  expect(result.current.news.AAPL).toEqual({ status: 'ok', news: news('Apple') })
})

it('refresh tiap 15 menit', async () => {
  vi.useFakeTimers()
  const { result } = renderHook(() => useNews(SYMBOLS, true))
  await act(async () => pending.AAPL[0].resolve(news('Apple')))
  await act(async () => pending.AMD[0].resolve(news('AMD')))

  await act(async () => vi.advanceTimersByTime(NEWS_REFRESH_MS))
  expect(calls()).toEqual(['AAPL', 'AMD', 'AAPL'])
  // Badge nggak kedip: hasil lama tetap tampil selama diambil ulang.
  expect(result.current.news.AAPL).toEqual({ status: 'ok', news: news('Apple') })
})
