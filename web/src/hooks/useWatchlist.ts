import { useCallback, useEffect, useState } from 'react'

import { addToWatchlist, errorMessage, fetchWatchlist, removeFromWatchlist } from '../api/client'

export interface Watchlist {
  /** `null` selama daftar pertama belum termuat. */
  symbols: readonly string[] | null
  loadError: string | null
  /** Coba muat lagi setelah gagal. */
  reload: () => void
  /** Melempar `ApiError` kalau ditolak server (simbol nggak dikenal, duplikat, penuh). */
  add: (symbol: string) => Promise<void>
  remove: (symbol: string) => Promise<void>
}

/** Watchlist yang tersimpan di server. Daftar lokal baru diubah setelah server setuju. */
export function useWatchlist(): Watchlist {
  const [symbols, setSymbols] = useState<readonly string[] | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  // Dinaikkan oleh `reload`. Karena jadi dependency effect di bawah, menaikkannya
  // membuat React menjalankan ulang effect itu = fetch ulang.
  const [attempt, setAttempt] = useState(0)

  useEffect(() => {
    const controller = new AbortController()
    fetchWatchlist(controller.signal).then(setSymbols, (err: unknown) => {
      if (!controller.signal.aborted) setLoadError(errorMessage(err))
    })
    return () => controller.abort()
  }, [attempt])

  const reload = useCallback(() => {
    setLoadError(null)
    setAttempt((n) => n + 1)
  }, [])

  // useCallback: fungsi yang sama dikembalikan di setiap render. Tanpa ini, tiap render
  // bikin fungsi baru dan komponen `memo` yang menerimanya (QuoteRow) ikut render ulang.
  const add = useCallback(async (raw: string) => {
    const symbol = await addToWatchlist(raw)
    setSymbols((prev) => (prev?.includes(symbol) ? prev : [...(prev ?? []), symbol]))
  }, [])

  const remove = useCallback(async (symbol: string) => {
    await removeFromWatchlist(symbol)
    setSymbols((prev) => prev?.filter((s) => s !== symbol) ?? null)
  }, [])

  return { symbols, loadError, reload, add, remove }
}
