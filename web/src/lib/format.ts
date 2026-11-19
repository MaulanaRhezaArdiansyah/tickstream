const twoDecimals = new Intl.NumberFormat('en-US', {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
})

/** 1234.5 → "1,234.50"; kosong → "–". */
export function fmt(n: number | null | undefined): string {
  return n == null || Number.isNaN(n) ? '–' : twoDecimals.format(n)
}

/** Tanda "+" untuk angka positif; angka negatif sudah membawa "-" sendiri. */
export function sign(n: number): string {
  return n >= 0 ? '+' : ''
}

const compact = new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 })

/** 1234567 → "1.2M". */
export function fmtVol(n: number): string {
  return compact.format(n)
}

/**
 * "BINANCE:BTCUSDT" → { ticker: "BTCUSDT", venue: "Binance" }. Saham US nggak punya
 * awalan bursa, jadi `venue`-nya null.
 */
export function splitSymbol(symbol: string): { ticker: string; venue: string | null } {
  const colon = symbol.indexOf(':')
  if (colon === -1) return { ticker: symbol, venue: null }
  const exchange = symbol.slice(0, colon)
  return {
    ticker: symbol.slice(colon + 1),
    venue: exchange.charAt(0) + exchange.slice(1).toLowerCase(),
  }
}
