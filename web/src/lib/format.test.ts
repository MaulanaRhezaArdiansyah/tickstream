import { describe, expect, it } from 'vitest'

import { fmt, fmtVol, sign, splitSymbol } from './format'

describe('fmt', () => {
  it('dua desimal dengan pemisah ribuan', () => {
    expect(fmt(1234.5)).toBe('1,234.50')
  })

  it('nilai kosong jadi strip', () => {
    expect(fmt(null)).toBe('–')
    expect(fmt(undefined)).toBe('–')
    expect(fmt(Number.NaN)).toBe('–')
  })
})

it('sign: plus untuk nol dan positif, kosong untuk negatif', () => {
  expect(sign(0)).toBe('+')
  expect(sign(2)).toBe('+')
  expect(sign(-2)).toBe('')
})

it('fmtVol meringkas angka besar', () => {
  expect(fmtVol(250)).toBe('250')
  expect(fmtVol(49_600)).toBe('49.6K')
  expect(fmtVol(1_234_567)).toBe('1.2M')
})

it('splitSymbol memisahkan bursa crypto dari ticker', () => {
  expect(splitSymbol('AAPL')).toEqual({ ticker: 'AAPL', venue: null })
  expect(splitSymbol('BINANCE:BTCUSDT')).toEqual({ ticker: 'BTCUSDT', venue: 'Binance' })
})
