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

it('naik, turun, lalu harga sama', () => {
  const store = new TickStore()
  store.apply([trade('AAPL', 10, 1)])
  store.apply([trade('AAPL', 11, 2)])
  store.apply([trade('AAPL', 9, 3)])
  store.apply([trade('AAPL', 9, 4)])
  nextFrame()

  const tick = store.getSnapshot().AAPL
  expect(tick.flash).toBe('down')
  // Harga sama nggak menaikkan flashSeq, jadi baris nggak berkedip tanpa perubahan.
  expect(tick.flashSeq).toBe(2)
  expect(tick.high).toBe(11)
  expect(tick.low).toBe(9)
  expect(tick.volume).toBe(10)
  expect(tick.points).toEqual([10, 11, 9, 9])
})

it('simbol yang nggak berubah tetap objek yang sama', () => {
  const store = new TickStore()
  store.apply([trade('AAPL', 10), trade('MSFT', 20)])
  nextFrame()
  const msft = store.getSnapshot().MSFT

  store.apply([trade('AAPL', 11)])
  nextFrame()
  // Referensi sama = baris MSFT yang di-memo nggak dirender ulang.
  expect(store.getSnapshot().MSFT).toBe(msft)
})

it('sparkline menyimpan paling banyak 300 titik', () => {
  const store = new TickStore()
  store.apply(Array.from({ length: 301 }, (_, i) => trade('AAPL', i)))
  nextFrame()

  const { points } = store.getSnapshot().AAPL
  expect(points).toHaveLength(300)
  expect(points[0]).toBe(1)
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
