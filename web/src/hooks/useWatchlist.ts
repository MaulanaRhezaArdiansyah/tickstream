import { useCallback, useEffect, useState } from 'react'

import { addToWatchlist, errorMessage, fetchWatchlist, removeFromWatchlist } from '../api/client'

export interface Watchlist {
  /** `null` selama daftar pertama belum termuat. */
  symbols: readonly string[] | null
  loadError: string | null
  /** Melempar `ApiError` kalau ditolak server (simbol nggak dikenal, duplikat, penuh). */
  add: (symbol: string) => Promise<void>
  remove: (symbol: string) => Promise<void>
}

/** Watchlist yang tersimpan di server. Daftar lokal baru diubah setelah server setuju. */
export function useWatchlist(): Watchlist {
  const [symbols, setSymbols] = useState<readonly string[] | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)

  useEffect(() => {
    const controller = new AbortController()
    fetchWatchlist(controller.signal).then(setSymbols, (err: unknown) => {
      if (!controller.signal.aborted) setLoadError(errorMessage(err))
    })
    return () => controller.abort()
  }, [])

  // useCallback: fungsi yang sama dikembalikan di setiap render. Tanpa ini, tiap render
  // bikin fungsi baru dan komponen `memo` yang menerimanya (StockCard) ikut render ulang.
  const add = useCallback(async (raw: string) => {
    const symbol = await addToWatchlist(raw)
    setSymbols((prev) => (prev?.includes(symbol) ? prev : [...(prev ?? []), symbol]))
  }, [])

  const remove = useCallback(async (symbol: string) => {
    await removeFromWatchlist(symbol)
    setSymbols((prev) => prev?.filter((s) => s !== symbol) ?? null)
  }, [])

  return { symbols, loadError, add, remove }
}
