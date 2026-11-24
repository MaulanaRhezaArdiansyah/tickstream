// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { ApiError, fetchCandles } from '../api/client'
import type { ConnectionState } from './useLiveTicks'
import type { CandleRange, CandleSeries } from '../types'
import { useCandles } from './useCandles'

vi.mock('../api/client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api/client')>()),
  fetchCandles: vi.fn(),
}))

const mockFetch = vi.mocked(fetchCandles)

interface PendingCall {
  symbol: string
  range: CandleRange
  signal: AbortSignal | undefined
  resolve: (series: CandleSeries) => void
  reject: (err: unknown) => void
}

let pending: PendingCall[] = []

const series = (symbol: string, range: CandleRange = '1d', close = 100): CandleSeries => ({
  symbol,
  range,
  bucketSeconds: range === '1d' ? 60 : 300,
  candles: [[1_700_000_040, close, close + 1, close - 1, close]],
})

beforeEach(() => {
  pending = []
  mockFetch.mockReset()
  mockFetch.mockImplementation((symbol, range, signal) => {
    return new Promise<CandleSeries>((resolve, reject) => {
      pending.push({ symbol, range, signal, resolve, reject })
    })
  })
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

it('memuat lalu ok', async () => {
  const { result } = renderHook(() => useCandles('NVDA', '1d', 'live'))
  expect(result.current.state).toEqual({ status: 'loading' })

  const data = series('NVDA')
  await act(async () => pending[0].resolve(data))
  expect(result.current.state).toEqual({ status: 'ok', series: data })
})

it('error lalu retry berhasil', async () => {
  const { result } = renderHook(() => useCandles('NVDA', '1d', 'live'))

  await act(async () => pending[0].reject(new ApiError(500, 'kesalahan server')))
  expect(result.current.state).toEqual({ status: 'error', message: 'kesalahan server' })

  act(() => result.current.retry())
  expect(mockFetch).toHaveBeenCalledTimes(2)
  const data = series('NVDA')
  await act(async () => pending[1].resolve(data))
  expect(result.current.state).toEqual({ status: 'ok', series: data })
})

it('respons lama nggak menimpa simbol yang baru dipilih', async () => {
  const { result, rerender } = renderHook(
    ({ symbol }: { symbol: string }) => useCandles(symbol, '1d', 'live'),
    { initialProps: { symbol: 'AMD' } },
  )
  expect(pending[0].symbol).toBe('AMD')

  rerender({ symbol: 'NVDA' })
  expect(pending[0].signal?.aborted).toBe(true)
  expect(pending[1].symbol).toBe('NVDA')

  const nvda = series('NVDA', '1d', 130)
  await act(async () => pending[1].resolve(nvda))
  // Respons AMD datang belakangan: nggak boleh menimpa NVDA.
  await act(async () => pending[0].resolve(series('AMD', '1d', 90)))

  expect(result.current.state).toEqual({ status: 'ok', series: nvda })
})

it('ganti range kembali ke loading dan mengambil range baru', async () => {
  const { result, rerender } = renderHook(
    ({ range }: { range: CandleRange }) => useCandles('NVDA', range, 'live'),
    { initialProps: { range: '1d' as CandleRange } },
  )
  await act(async () => pending[0].resolve(series('NVDA', '1d')))
  expect(result.current.state.status).toBe('ok')

  rerender({ range: '5d' })
  expect(result.current.state).toEqual({ status: 'loading' })
  expect(pending[1].range).toBe('5d')

  const five = series('NVDA', '5d')
  await act(async () => pending[1].resolve(five))
  expect(result.current.state).toEqual({ status: 'ok', series: five })
})

it('state loading stabil antar render saat menunggu data baru', async () => {
  const { result, rerender } = renderHook(
    ({ range }: { range: CandleRange }) => useCandles('NVDA', range, 'live'),
    { initialProps: { range: '1d' as CandleRange } },
  )
  await act(async () => pending[0].resolve(series('NVDA', '1d')))

  rerender({ range: '5d' })
  const first = result.current.state
  rerender({ range: '5d' })
  expect(result.current.state).toBe(first)
})

it('reconnect mengambil ulang tanpa mengosongkan data', async () => {
  const { result, rerender } = renderHook(
    ({ connection }: { connection: ConnectionState }) => useCandles('NVDA', '1d', connection),
    { initialProps: { connection: 'live' as ConnectionState } },
  )
  const initial = series('NVDA', '1d', 100)
  await act(async () => pending[0].resolve(initial))

  rerender({ connection: 'reconnecting' })
  expect(mockFetch).toHaveBeenCalledTimes(1)

  rerender({ connection: 'live' })
  expect(mockFetch).toHaveBeenCalledTimes(2)
  // Data lama tetap tampil selama refresh berlangsung.
  expect(result.current.state).toEqual({ status: 'ok', series: initial })

  const updated = series('NVDA', '1d', 105)
  await act(async () => pending[1].resolve(updated))
  expect(result.current.state).toEqual({ status: 'ok', series: updated })
})

it('koneksi pertama connecting ke live nggak mengambil dua kali', async () => {
  const { rerender } = renderHook(
    ({ connection }: { connection: ConnectionState }) => useCandles('NVDA', '1d', connection),
    { initialProps: { connection: 'connecting' as ConnectionState } },
  )
  await act(async () => pending[0].resolve(series('NVDA')))

  rerender({ connection: 'live' })
  expect(mockFetch).toHaveBeenCalledTimes(1)
})

it('refresh yang gagal nggak menghapus data yang sudah tampil', async () => {
  const { result, rerender } = renderHook(
    ({ connection }: { connection: ConnectionState }) => useCandles('NVDA', '1d', connection),
    { initialProps: { connection: 'live' as ConnectionState } },
  )
  const initial = series('NVDA', '1d', 100)
  await act(async () => pending[0].resolve(initial))

  rerender({ connection: 'reconnecting' })
  rerender({ connection: 'live' })
  await act(async () => pending[1].reject(new Error('network')))

  expect(result.current.state).toEqual({ status: 'ok', series: initial })
})

it('simbol crypto dikodekan di URL', async () => {
  const actual = await vi.importActual<typeof import('../api/client')>('../api/client')
  const fetchSpy = vi.fn().mockResolvedValue({
    ok: true,
    status: 200,
    json: async () => series('BINANCE:BTCUSDT', '5d'),
  })
  vi.stubGlobal('fetch', fetchSpy)

  await actual.fetchCandles('BINANCE:BTCUSDT', '5d')
  expect(fetchSpy).toHaveBeenCalledTimes(1)
  const [url] = fetchSpy.mock.calls[0] as [string]
  expect(url).toContain('symbol=BINANCE%3ABTCUSDT&range=5d')
})
