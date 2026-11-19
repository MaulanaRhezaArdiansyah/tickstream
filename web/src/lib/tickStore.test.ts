import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import type { Trade } from '../types'
import { TickStore } from './tickStore'

// Node nggak punya requestAnimationFrame. Callback-nya ditampung di sini dan baru dijalankan
// saat tes memanggil nextFrame(), jadi tes yang menentukan kapan "frame layar" terjadi.
let frames: FrameRequestCallback[] = []
const cancelFrame = vi.fn()

function nextFrame() {
  const queued = frames
  frames = []
  for (const callback of queued) callback(0)
}

const trade = (symbol: string, price: number, volume = 1): Trade => ({
  symbol,
  price,
  volume,
  timestamp: 0,
})

/** Trade dengan waktu (Unix ms), untuk tes sparkline yang dikelompokkan per 10 detik. */
const tradeAt = (symbol: string, timestamp: number, price: number): Trade => ({
  symbol,
  price,
  volume: 1,
  timestamp,
})

beforeEach(() => {
  frames = []
  cancelFrame.mockClear()
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    frames.push(callback)
    return frames.length
  })
  vi.stubGlobal('cancelAnimationFrame', cancelFrame)
})

afterEach(() => {
  vi.unstubAllGlobals()
})

it('belum ada snapshot sebelum frame berikutnya', () => {
  const store = new TickStore()
  store.apply([trade('AAPL', 10)])
  expect(store.getSnapshot()).toEqual({})

  nextFrame()
  expect(store.getSnapshot().AAPL.price).toBe(10)
})

it('banyak batch dalam satu frame cuma memicu satu render', () => {
  const store = new TickStore()
  const listener = vi.fn()
  store.subscribe(listener)

  store.apply([trade('AAPL', 10)])
  store.apply([trade('AAPL', 11)])
  expect(frames).toHaveLength(1)

  nextFrame()
  expect(listener).toHaveBeenCalledTimes(1)
})

it('turun, naik, lalu harga sama', () => {
  const store = new TickStore()
  store.apply([trade('AAPL', 10, 1)])
  store.apply([trade('AAPL', 9, 2)])
  store.apply([trade('AAPL', 11, 3)])
  store.apply([trade('AAPL', 11, 4)])
  nextFrame()

  const tick = store.getSnapshot().AAPL
  // Harga sama nggak mengubah arah dan nggak menaikkan flashSeq, jadi baris nggak berkedip.
  expect(tick.flash).toBe('up')
  expect(tick.flashSeq).toBe(2)
  expect(tick.high).toBe(11)
  expect(tick.low).toBe(9)
  expect(tick.volume).toBe(10)
})

it('flash masih null sampai harganya pertama kali berubah', () => {
  const store = new TickStore()
  store.apply([trade('AAPL', 10), trade('AAPL', 10)])
  nextFrame()

  const tick = store.getSnapshot().AAPL
  expect(tick.flash).toBeNull()
  expect(tick.flashSeq).toBe(0)
})

it('simbol yang nggak berubah tetap objek yang sama', () => {
  const store = new TickStore()
  store.apply([trade('AAPL', 10), trade('MSFT', 20)])
  nextFrame()
  const msft = store.getSnapshot().MSFT

  store.apply([trade('AAPL', 11)])
  nextFrame()
  expect(store.getSnapshot().AAPL.price).toBe(11)
  // Referensi sama = baris MSFT yang di-memo nggak dirender ulang.
  expect(store.getSnapshot().MSFT).toBe(msft)
})

it('sparkline yang nggak berubah tetap array yang sama', () => {
  const store = new TickStore()
  store.apply([tradeAt('AAPL', 0, 10), tradeAt('MSFT', 0, 20)])
  nextFrame()
  const msft = store.getSparks().MSFT

  store.apply([tradeAt('AAPL', 10_000, 11)])
  nextFrame()
  expect(store.getSparks().AAPL).toEqual([10, 11])
  expect(store.getSparks().MSFT).toBe(msft)
})

it('sparkline per 10 detik: kelompok yang sama menimpa harga', () => {
  const store = new TickStore()
  store.apply([tradeAt('AAPL', 0, 10), tradeAt('AAPL', 9_999, 11), tradeAt('AAPL', 10_000, 12)])
  nextFrame()
  expect(store.getSparks().AAPL).toEqual([11, 12])
})

it('trade yang lebih lama dari kelompok terbaru diabaikan', () => {
  const store = new TickStore()
  store.apply([tradeAt('AAPL', 20_000, 5), tradeAt('AAPL', 10_000, 9)])
  nextFrame()
  expect(store.getSparks().AAPL).toEqual([5])
})

it('sparkline menyimpan paling banyak 180 titik', () => {
  const store = new TickStore()
  store.apply(Array.from({ length: 181 }, (_, i) => tradeAt('AAPL', i * 10_000, i)))
  nextFrame()

  const points = store.getSparks().AAPL
  expect(points).toHaveLength(180)
  expect(points[0]).toBe(1)
})

it('seed mengisi sparkline tanpa membuat harga live', () => {
  const store = new TickStore()
  store.seed('AAPL', [
    [0, 5],
    [10_000, 6],
  ])
  nextFrame()
  expect(store.getSparks().AAPL).toEqual([5, 6])
  // Malam hari belum ada trade: harga, volume dan high/low tetap dari quote.
  expect(store.getSnapshot().AAPL).toBeUndefined()
})

it('trade live menyambung dari riwayat', () => {
  const store = new TickStore()
  store.seed('AAPL', [
    [0, 5],
    [10_000, 6],
  ])
  store.apply([tradeAt('AAPL', 15_000, 7)])
  nextFrame()
  expect(store.getSparks().AAPL).toEqual([5, 7])

  store.apply([tradeAt('AAPL', 20_000, 8)])
  nextFrame()
  expect(store.getSparks().AAPL).toEqual([5, 7, 8])
})

it('riwayat kosong setelah server restart nggak menghapus garis', () => {
  const store = new TickStore()
  store.apply([tradeAt('AAPL', 0, 3), tradeAt('AAPL', 10_000, 4)])
  store.seed('AAPL', [])
  nextFrame()
  expect(store.getSparks().AAPL).toEqual([3, 4])
})

it('titik lokal yang lebih baru dari riwayat disambung di belakang', () => {
  const store = new TickStore()
  store.apply([tradeAt('AAPL', 0, 3), tradeAt('AAPL', 10_000, 4)])
  store.seed('AAPL', [[0, 1]])
  nextFrame()
  expect(store.getSparks().AAPL).toEqual([1, 4])
})

it('seed kedua nggak menggandakan ekor', () => {
  const store = new TickStore()
  store.apply([tradeAt('AAPL', 0, 3), tradeAt('AAPL', 10_000, 4)])
  // Reconnect: riwayat server sudah memuat kelompok yang tadi diterima live.
  store.seed('AAPL', [
    [0, 1],
    [10_000, 2],
  ])
  nextFrame()
  expect(store.getSparks().AAPL).toEqual([1, 2])
})

it('takeTradeCount mengembalikan jumlah lalu mereset', () => {
  const store = new TickStore()
  store.apply([trade('AAPL', 1), trade('AAPL', 2), trade('MSFT', 3)])
  expect(store.takeTradeCount()).toBe(3)
  expect(store.takeTradeCount()).toBe(0)
})

it('dispose membatalkan frame yang sudah dijadwalkan', () => {
  const store = new TickStore()
  store.apply([trade('AAPL', 10)])
  store.dispose()
  expect(cancelFrame).toHaveBeenCalledWith(1)
})
