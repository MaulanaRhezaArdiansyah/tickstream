import { useEffect, useRef, useState } from 'react'

import { errorMessage, fetchProfile, fetchQuote } from '../api/client'
import { runPool } from '../lib/pool'
import type { Quote } from '../types'

export interface Snapshot {
  quote?: Quote
  name?: string
  logo?: string | null
  error?: string
}

// Setelah burst habis, limiter server melepas ±1 call/detik. Tiga request paralel cukup
// supaya kartu terisi cepat tanpa bikin antrean server kepenuhan (429).
const CONCURRENCY = 3

/**
 * Ambil quote + nama perusahaan untuk tiap simbol dari REST.
 *
 * Saat `symbols` berubah, cuma simbol yang BELUM pernah dimuat yang diambil; menambah satu
 * simbol nggak memuat ulang semuanya. `symbols` harus referensi yang stabil.
 */
export function useStockSnapshots(symbols: readonly string[]): Record<string, Snapshot> {
  const [snapshots, setSnapshots] = useState<Record<string, Snapshot>>({})
  // Simbol yang sedang/sudah dimuat. Ref, bukan state: mengubahnya nggak perlu render.
  const requested = useRef(new Set<string>())

  useEffect(() => {
    const controller = new AbortController()
    const { signal } = controller
    const inFlight = requested.current // Set yang sama seumur komponen
    const fresh = symbols.filter((symbol) => !inFlight.has(symbol))
    const done = new Set<string>()
    for (const symbol of fresh) inFlight.add(symbol)

    // Functional update: selalu berangkat dari state terbaru, karena banyak request
    // selesai bergantian dan masing-masing cuma mengubah satu simbol.
    const patch = (symbol: string, data: Partial<Snapshot>) =>
      setSnapshots((prev) => ({
        ...prev,
        [symbol]: { ...prev[symbol], ...data },
      }))

    async function load() {
      // Harga dulu untuk semua simbol, baru nama: angka di kartu muncul lebih cepat.
      await runPool(fresh, CONCURRENCY, async (symbol) => {
        if (signal.aborted) return
        try {
          const quote = await fetchQuote(symbol, signal)
          patch(symbol, { quote, error: undefined })
        } catch (err) {
          if (!signal.aborted) patch(symbol, { error: errorMessage(err) })
        }
      })

      await runPool(fresh, CONCURRENCY, async (symbol) => {
        if (signal.aborted) return
        try {
          const profile = await fetchProfile(symbol, signal)
          patch(symbol, { name: profile.name, logo: profile.logo })
        } catch {
          // Nama cuma pelengkap; kartu tetap jalan tanpa nama.
        }
        if (!signal.aborted) done.add(symbol)
      })
    }

    void load()
    return () => {
      controller.abort()
      // Yang terpotong di tengah jalan dilepas lagi, supaya putaran effect berikutnya
      // (atau mount kedua di StrictMode) memuatnya ulang.
      for (const symbol of fresh) if (!done.has(symbol)) inFlight.delete(symbol)
    }
  }, [symbols])

  return snapshots
}
