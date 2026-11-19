import { expect, it } from 'vitest'

import type { SymbolInfo } from '../types'
import { searchSymbols } from './symbolSearch'

// Urut simbol, seperti daftar dari server.
const LIST: SymbolInfo[] = [
  { symbol: 'AAPL', name: 'APPLE INC', kind: 'stock' },
  { symbol: 'AMD', name: 'ADVANCED MICRO DEVICES INC', kind: 'stock' },
  { symbol: 'AMDX', name: 'AMDX CORP', kind: 'stock' },
  { symbol: 'BRK.A', name: 'BERKSHIRE HATHAWAY INC CL A', kind: 'stock' },
  { symbol: 'BRK.B', name: 'BERKSHIRE HATHAWAY INC CL B', kind: 'stock' },
  { symbol: 'BRKR', name: 'BRUKER CORP', kind: 'stock' },
  { symbol: 'PNPL', name: 'PINEAPPLE INC', kind: 'stock' },
  { symbol: 'SPY', name: 'SPDR S&P 500 ETF', kind: 'etf' },
]

const symbols = (query: string, limit = 10) =>
  searchSymbols(LIST, query, limit).items.map((info) => info.symbol)

it('ticker persis di urutan pertama', () => {
  expect(symbols('AMD')).toEqual(['AMD', 'AMDX'])
})

it('ticker berawalan query tetap alfabetis', () => {
  expect(symbols('BRK')).toEqual(['BRK.A', 'BRK.B', 'BRKR'])
})

it('awal kata di nama sebelum nama yang sekadar memuat query', () => {
  expect(symbols('apple')).toEqual(['AAPL', 'PNPL'])
})

it('spasi dan huruf kecil diabaikan', () => {
  expect(symbols('  brk  ')).toEqual(symbols('BRK'))
})

it('limit memotong hasil tapi total tetap semua yang cocok', () => {
  const result = searchSymbols(LIST, 'BERKSHIRE', 1)
  expect(result.items).toHaveLength(1)
  expect(result.total).toBe(2)
})

it('query kosong = awal daftar', () => {
  const result = searchSymbols(LIST, '', 3)
  expect(result.items.map((info) => info.symbol)).toEqual(['AAPL', 'AMD', 'AMDX'])
  expect(result.total).toBe(8)
})

it('nggak ada yang cocok', () => {
  expect(searchSymbols(LIST, 'ZZZ', 10)).toEqual({ items: [], total: 0 })
})
