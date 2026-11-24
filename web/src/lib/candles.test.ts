import { expect, it } from 'vitest'

import { applyTick, formatWibDay, formatWibTime, toBars } from './candles'

it('mengubah baris server jadi bar', () => {
  expect(toBars([[60, 1, 3, 0, 2]])).toEqual([{ time: 60, open: 1, high: 3, low: 0, close: 2 }])
})

it('tick di bucket yang sama memperbarui bar', () => {
  const last = { time: 60, open: 10, high: 12, low: 9, close: 11 }
  expect(applyTick(last, 13, 61_000, 60)).toEqual({
    time: 60,
    open: 10,
    high: 13,
    low: 9,
    close: 13,
  })
})

it('tick di bucket baru membuat bar baru', () => {
  const last = { time: 60, open: 10, high: 12, low: 9, close: 11 }
  expect(applyTick(last, 13, 120_000, 60)).toEqual({
    time: 120,
    open: 13,
    high: 13,
    low: 13,
    close: 13,
  })
})

it('bar pertama tanpa riwayat', () => {
  expect(applyTick(undefined, 5, 61_000, 60)).toEqual({
    time: 60,
    open: 5,
    high: 5,
    low: 5,
    close: 5,
  })
})

it('tick lebih lama dari bar terakhir diabaikan', () => {
  const last = { time: 120, open: 10, high: 12, low: 9, close: 11 }
  expect(applyTick(last, 15, 119_999, 60)).toBeNull()
})

it('bucket 5 menit dibulatkan ke bawah', () => {
  expect(applyTick(undefined, 10, 899_000, 300)).toEqual({
    time: 600,
    open: 10,
    high: 10,
    low: 10,
    close: 10,
  })
})

it('format waktu WIB', () => {
  expect(formatWibTime(1_700_000_000)).toBe('05:13')
  expect(formatWibDay(1_700_000_000)).toBe('15 Nov')
})
