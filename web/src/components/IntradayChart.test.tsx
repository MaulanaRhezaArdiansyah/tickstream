// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { useCandles, type CandlesState } from '../hooks/useCandles'
import type { LiveTick } from '../lib/tickStore'
import type { CandleSeries } from '../types'
import IntradayChart from './IntradayChart'

const seriesMock = {
  setData: vi.fn(),
  update: vi.fn(),
}

const fitContentMock = vi.fn()

const chartMock = {
  addSeries: vi.fn(() => seriesMock),
  remove: vi.fn(),
  applyOptions: vi.fn(),
  timeScale: vi.fn(() => ({ fitContent: fitContentMock })),
}

const createChartMock = vi.fn((..._args: unknown[]) => chartMock)

vi.mock('lightweight-charts', () => ({
  createChart: (...args: unknown[]) => createChartMock(...args),
  CandlestickSeries: Symbol('CandlestickSeries'),
  ColorType: { Solid: 'solid' },
  TickMarkType: {
    Year: 0,
    Month: 1,
    DayOfMonth: 2,
    Time: 3,
    TimeWithSeconds: 4,
  },
}))

vi.mock('../hooks/useCandles', () => ({
  useCandles: vi.fn(),
}))

const mockUseCandles = vi.mocked(useCandles)

let currentState: CandlesState = { status: 'loading' }
const retryMock = vi.fn()

const sampleSeries = (symbol = 'NVDA', candles: CandleSeries['candles'] = [[1_700_000_040, 100, 102, 99, 101]]): CandleSeries => ({
  symbol,
  range: '1d',
  bucketSeconds: 60,
  candles,
})

const tick = (price: number, timestamp: number): LiveTick => ({
  price,
  timestamp,
  volume: 10,
  high: price,
  low: price,
  flash: null,
  flashSeq: 0,
})

beforeEach(() => {
  currentState = { status: 'loading' }
  retryMock.mockReset()
  seriesMock.setData.mockReset()
  seriesMock.update.mockReset()
  fitContentMock.mockReset()
  chartMock.addSeries.mockClear()
  chartMock.remove.mockReset()
  chartMock.applyOptions.mockReset()
  chartMock.timeScale.mockClear()
  createChartMock.mockClear()

  mockUseCandles.mockImplementation(() => ({
    state: currentState,
    retry: retryMock,
  }))
})

afterEach(cleanup)

it('menampilkan toggle 1H dan 5H, default 1H', () => {
  render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  const btn1h = screen.getByRole('button', { name: '1H' })
  const btn5h = screen.getByRole('button', { name: '5H' })
  expect(btn1h.getAttribute('aria-pressed')).toBe('true')
  expect(btn5h.getAttribute('aria-pressed')).toBe('false')
})

it('klik 5H meminta range 5d', () => {
  render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  fireEvent.click(screen.getByRole('button', { name: '5H' }))
  expect(mockUseCandles).toHaveBeenLastCalledWith('NVDA', '5d', 'live')
  expect(screen.getByRole('button', { name: '5H' }).getAttribute('aria-pressed')).toBe('true')
})

it('mengisi chart saat data datang', () => {
  currentState = { status: 'ok', series: sampleSeries('NVDA') }
  render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  expect(seriesMock.setData).toHaveBeenCalledWith([
    { time: 1_700_000_040, open: 100, high: 102, low: 99, close: 101 },
  ])
})

it('kosong menampilkan pesan', () => {
  currentState = { status: 'ok', series: sampleSeries('NVDA', []) }
  render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  expect(
    screen.getByText('Belum ada data grafik. Data mulai terkumpul saat ada transaksi.'),
  ).not.toBeNull()
})

it('pesan kosong hilang setelah tick live pertama tergambar', () => {
  const empty = 'Belum ada data grafik. Data mulai terkumpul saat ada transaksi.'
  currentState = { status: 'ok', series: sampleSeries('NVDA', []) }
  const { rerender } = render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  expect(screen.queryByText(empty)).not.toBeNull()

  rerender(<IntradayChart symbol="NVDA" live={tick(103, 1_700_000_045_000)} connection="live" />)
  expect(seriesMock.update).toHaveBeenCalledTimes(1)
  expect(screen.queryByText(empty)).toBeNull()
})

it('tick rusak nggak menghilangkan pesan kosong', () => {
  const empty = 'Belum ada data grafik. Data mulai terkumpul saat ada transaksi.'
  currentState = { status: 'ok', series: sampleSeries('NVDA', []) }
  render(<IntradayChart symbol="NVDA" live={tick(-1, 1_700_000_045_000)} connection="live" />)
  expect(seriesMock.update).not.toHaveBeenCalled()
  expect(screen.queryByText(empty)).not.toBeNull()
})

it('pesan kosong tetap muncul untuk simbol lain yang belum ada tick', () => {
  const empty = 'Belum ada data grafik. Data mulai terkumpul saat ada transaksi.'
  currentState = { status: 'ok', series: sampleSeries('NVDA', []) }
  const { rerender } = render(
    <IntradayChart symbol="NVDA" live={tick(103, 1_700_000_045_000)} connection="live" />,
  )
  expect(screen.queryByText(empty)).toBeNull()

  currentState = { status: 'ok', series: sampleSeries('AAPL', []) }
  rerender(<IntradayChart symbol="AAPL" live={undefined} connection="live" />)
  expect(screen.queryByText(empty)).not.toBeNull()
})

it('error menampilkan pesan dan Coba lagi memanggil retry', () => {
  currentState = { status: 'error', message: 'timeout' }
  render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  expect(screen.getByText('Grafik nggak bisa dimuat.')).not.toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Coba lagi' }))
  expect(retryMock).toHaveBeenCalledTimes(1)
})

it('catatan feed selalu tampil', () => {
  const note = 'Dari transaksi yang diterima feed gratis, bisa beda dari angka resmi.'
  const states: CandlesState[] = [
    { status: 'loading' },
    { status: 'ok', series: sampleSeries('NVDA', []) },
    { status: 'error', message: 'err' },
    { status: 'ok', series: sampleSeries('NVDA') },
  ]

  const { rerender } = render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  for (const state of states) {
    currentState = state
    rerender(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
    expect(screen.getByText(note)).not.toBeNull()
  }
})

it('punya label untuk pembaca layar', () => {
  render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  expect(screen.getByRole('img', { name: 'Grafik harga NVDA, 1 hari' })).not.toBeNull()
})

it('tick baru memanggil update', () => {
  currentState = { status: 'ok', series: sampleSeries('NVDA') }
  const { rerender } = render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  expect(seriesMock.update).not.toHaveBeenCalled()

  rerender(
    <IntradayChart
      symbol="NVDA"
      live={tick(103, 1_700_000_045_000)}
      connection="live"
    />,
  )
  expect(seriesMock.update).toHaveBeenCalledWith({
    time: 1_700_000_040,
    open: 100,
    high: 103,
    low: 99,
    close: 103,
  })
})

it('tick lama nggak memanggil update', () => {
  currentState = { status: 'ok', series: sampleSeries('NVDA') }
  render(
    <IntradayChart
      symbol="NVDA"
      live={tick(95, 1_700_000_039_999)}
      connection="live"
    />,
  )
  expect(seriesMock.update).not.toHaveBeenCalled()
})

it('tutup panel menghapus chart sekali dan tick berikutnya nggak dipakai', () => {
  currentState = { status: 'ok', series: sampleSeries('NVDA') }
  const { unmount } = render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  expect(chartMock.remove).not.toHaveBeenCalled()

  unmount()
  expect(chartMock.remove).toHaveBeenCalledTimes(1)
  expect(seriesMock.update).not.toHaveBeenCalled()
})

it('ganti simbol memuat data baru tanpa membuat chart kedua', () => {
  currentState = { status: 'ok', series: sampleSeries('NVDA') }
  const { rerender } = render(<IntradayChart symbol="NVDA" live={undefined} connection="live" />)
  expect(createChartMock).toHaveBeenCalledTimes(1)

  currentState = { status: 'ok', series: sampleSeries('AAPL', [[1_700_000_100, 200, 205, 198, 202]]) }
  rerender(<IntradayChart symbol="AAPL" live={undefined} connection="live" />)
  expect(createChartMock).toHaveBeenCalledTimes(1)
  expect(seriesMock.setData).toHaveBeenLastCalledWith([
    { time: 1_700_000_100, open: 200, high: 205, low: 198, close: 202 },
  ])
})
