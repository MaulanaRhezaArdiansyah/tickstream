import { useCallback, useEffect, useState } from 'react'

import {
  ApiError,
  addToWatchlist,
  errorMessage,
  fetchWatchlist,
  removeFromWatchlist,
} from '../api/client'

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

/**
 * Watchlist yang tersimpan di server. Daftar lokal baru diubah setelah server setuju.
 * Mengubahnya butuh `ownerToken`; tanpa token, `add`/`remove` langsung ditolak.
 */
export function useWatchlist(ownerToken: string | null): Watchlist {
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
  const add = useCallback(
    async (raw: string) => {
      const symbol = await addToWatchlist(raw, requireToken(ownerToken))
      setSymbols((prev) => (prev?.includes(symbol) ? prev : [...(prev ?? []), symbol]))
    },
    [ownerToken],
  )

  const remove = useCallback(
    async (symbol: string) => {
      await removeFromWatchlist(symbol, requireToken(ownerToken))
      setSymbols((prev) => prev?.filter((s) => s !== symbol) ?? null)
    },
    [ownerToken],
  )

  return { symbols, loadError, reload, add, remove }
}

/** Gagal dengan bentuk yang sama seperti tolakan server, supaya penanganannya satu jalur. */
function requireToken(token: string | null): string {
  if (!token) throw new ApiError(401, 'cuma pemilik yang bisa mengubah watchlist')
  return token
}
