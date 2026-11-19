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
