import { useEffect, useState } from 'react'

import { ApiError, fetchProfile, fetchQuote } from '../api/client'
import { runPool } from '../lib/pool'
import type { Quote } from '../types'

export interface Snapshot {
  quote?: Quote
  name?: string
  error?: string
}

// Setelah burst habis, limiter server melepas ±1 call/detik. Tiga request paralel cukup
// supaya kartu terisi cepat tanpa bikin antrean server kepenuhan (429).
const CONCURRENCY = 3

/**
 * Ambil quote + nama perusahaan untuk tiap simbol dari REST.
 *
 * `symbols` harus referensi yang stabil (konstanta atau hasil `useMemo`): effect ini
 * jalan ulang tiap kali referensi array-nya berubah, bukan isinya.
 */
export function useStockSnapshots(symbols: readonly string[]): Record<string, Snapshot> {
  const [snapshots, setSnapshots] = useState<Record<string, Snapshot>>({})

  useEffect(() => {
    const controller = new AbortController()
    const { signal } = controller

    // Functional update: selalu berangkat dari state terbaru, karena banyak request
    // selesai bergantian dan masing-masing cuma mengubah satu simbol.
    const patch = (symbol: string, data: Partial<Snapshot>) =>
      setSnapshots((prev) => ({ ...prev, [symbol]: { ...prev[symbol], ...data } }))

    async function load() {
      // Harga dulu untuk semua simbol, baru nama: angka di kartu muncul lebih cepat.
      await runPool(symbols, CONCURRENCY, async (symbol) => {
        if (signal.aborted) return
        try {
          const quote = await fetchQuote(symbol, signal)
          patch(symbol, { quote, error: undefined })
        } catch (err) {
          if (!signal.aborted) patch(symbol, { error: errorMessage(err) })
        }
      })

      await runPool(symbols, CONCURRENCY, async (symbol) => {
        if (signal.aborted) return
        try {
          const profile = await fetchProfile(symbol, signal)
          patch(symbol, { name: profile.name })
        } catch {
          // Nama cuma pelengkap; kartu tetap jalan tanpa nama.
        }
      })
    }

    void load()
    // Cleanup: dipanggil saat komponen unmount atau sebelum effect jalan ulang.
    return () => controller.abort()
  }, [symbols])

  return snapshots
}

function errorMessage(err: unknown): string {
  return err instanceof ApiError ? err.message : 'gagal memuat'
}
