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
