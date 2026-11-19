import { useCallback, useEffect, useRef, useState } from 'react'

import { errorMessage, fetchNews } from '../api/client'
import type { CompanyNews } from '../types'

export type NewsState =
  { status: 'loading' } | { status: 'ok'; news: CompanyNews } | { status: 'error'; message: string }

/** Sama dengan umur cache berita di server; lebih sering nggak memberi berita baru. */
export const NEWS_REFRESH_MS = 15 * 60_000

/**
 * Berita tiap simbol watchlist, untuk badge dan panel.
 *
 * Diambil SATU simbol per satu, dan baru mulai setelah `ready` (quote papan sudah termuat),
 * supaya berita nggak berebut jatah Finnhub dengan harga. `load` melompati antrean, untuk
 * panel yang baru dibuka atau tombol "Coba lagi". `symbols` harus referensi yang stabil.
 */
export function useNews(symbols: readonly string[], ready: boolean) {
  const [news, setNews] = useState<Readonly<Record<string, NewsState>>>({})
  // Putaran refresh; naik tiap NEWS_REFRESH_MS dan memicu antrean jalan lagi.
  const [round, setRound] = useState(0)
  // Simbol → putaran terakhir ia diambil. Ref: mengubahnya nggak perlu render.
  const fetchedIn = useRef(new Map<string, number>())
  const roundRef = useRef(round)
  // Untuk `load`, yang hidup di luar effect antrean: dibatalkan waktu unmount.
  const unmount = useRef<AbortController | null>(null)

  const fetchInto = useCallback(async (symbol: string, signal: AbortSignal) => {
    try {
      const result = await fetchNews(symbol, signal)
      if (!signal.aborted)
        setNews((prev) => ({ ...prev, [symbol]: { status: 'ok', news: result } }))
    } catch (err) {
      if (signal.aborted) return
      // Refresh yang gagal nggak menghapus berita yang sudah tampil.
      setNews((prev) =>
        prev[symbol]?.status === 'ok'
          ? prev
          : { ...prev, [symbol]: { status: 'error', message: errorMessage(err) } },
      )
    }
  }, [])

  useEffect(() => {
    const controller = new AbortController()
    unmount.current = controller
    return () => controller.abort()
  }, [])

  useEffect(() => {
    if (!ready) return
    const id = setInterval(() => setRound((r) => r + 1), NEWS_REFRESH_MS)
    return () => clearInterval(id)
  }, [ready])

  useEffect(() => {
    if (!ready) return
    roundRef.current = round
    const controller = new AbortController()
    const { signal } = controller

    async function drain() {
      for (const symbol of symbols) {
        if (signal.aborted) return
        if (fetchedIn.current.get(symbol) === round) continue
        fetchedIn.current.set(symbol, round)
        // Berita lama tetap tampil selama diambil ulang, supaya badge nggak kedip.
        setNews((prev) => (prev[symbol] ? prev : { ...prev, [symbol]: { status: 'loading' } }))
        await fetchInto(symbol, signal)
        // Terpotong di tengah jalan: lepas lagi supaya putaran effect berikutnya mengambilnya.
        if (signal.aborted) fetchedIn.current.delete(symbol)
      }
    }

    void drain()
    return () => controller.abort()
  }, [symbols, ready, round, fetchInto])

  const load = useCallback(
    (symbol: string) => {
      const signal = unmount.current?.signal
      if (!signal) return
      fetchedIn.current.set(symbol, roundRef.current)
      setNews((prev) => ({ ...prev, [symbol]: { status: 'loading' } }))
      void fetchInto(symbol, signal)
    },
    [fetchInto],
  )

  return { news, load }
}
