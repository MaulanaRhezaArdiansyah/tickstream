import type { SymbolInfo } from '../types'

export interface SymbolMatches {
  items: SymbolInfo[]
  /** Jumlah semua yang cocok, sebelum dipotong `limit`. */
  total: number
}

/**
 * Cari di daftar simbol, urut dari yang paling mungkin dimaksud:
 * ticker persis, ticker berawalan query, nama yang salah satu katanya berawalan query,
 * lalu nama yang sekadar memuat query.
 *
 * Daftar dari server sudah urut simbol, jadi isi tiap kelompok ikut alfabetis
 * ("BRK" → BRK.A, BRK.B, lalu BRKR). Query kosong = seluruh daftar.
 */
export function searchSymbols(
  list: readonly SymbolInfo[],
  query: string,
  limit: number,
): SymbolMatches {
  const q = query.trim().toUpperCase()
  if (!q) return { items: list.slice(0, limit), total: list.length }

  const groups: SymbolInfo[][] = [[], [], [], []]
  for (const info of list) {
    const rank = rankOf(info, q)
    if (rank !== null) groups[rank].push(info)
  }

  const all = groups.flat()
  return { items: all.slice(0, limit), total: all.length }
}

function rankOf({ symbol, name }: SymbolInfo, q: string): number | null {
  if (symbol === q) return 0
  if (symbol.startsWith(q)) return 1
  const upper = name.toUpperCase()
  if (upper.startsWith(q) || upper.includes(` ${q}`)) return 2
  if (upper.includes(q)) return 3
  return null
}
