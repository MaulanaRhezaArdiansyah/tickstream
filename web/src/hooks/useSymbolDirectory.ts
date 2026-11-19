import { useCallback, useEffect, useState } from 'react'

import { errorMessage, fetchSymbols } from '../api/client'
import type { SymbolInfo } from '../types'

export type SymbolDirectory =
  | { status: 'idle' | 'loading' }
  | { status: 'ready'; symbols: SymbolInfo[] }
  | { status: 'error'; message: string }

type Result = { attempt: number } & (
  { symbols: SymbolInfo[]; message?: never } | { message: string; symbols?: never }
)

/**
 * Daftar semua saham & ETF yang bisa ditambahkan (±11 ribu, ±140 KB). Baru diunduh setelah
 * `load()` dipanggil, jadi yang nggak pernah membuka form tambah nggak ikut mengunduhnya.
 * Kalau gagal, `load()` berikutnya mencoba lagi.
 */
export function useSymbolDirectory() {
  // Tiap kenaikan `attempt` menjalankan ulang effect = satu percobaan unduh.
  const [attempt, setAttempt] = useState(0)
  const [result, setResult] = useState<Result | null>(null)

  useEffect(() => {
    if (attempt === 0) return
    const controller = new AbortController()
    fetchSymbols(controller.signal).then(
      (symbols) => setResult({ attempt, symbols }),
      (err: unknown) => {
        if (!controller.signal.aborted) setResult({ attempt, message: errorMessage(err) })
      },
    )
    return () => controller.abort()
  }, [attempt])

  const failed = result?.message !== undefined
  const load = useCallback(() => {
    setAttempt((current) => (current === 0 || failed ? current + 1 : current))
  }, [failed])

  // Status diturunkan dari state, bukan disimpan sendiri: nggak mungkin nggak sinkron.
  let directory: SymbolDirectory
  if (attempt === 0) directory = { status: 'idle' }
  else if (result?.attempt !== attempt) directory = { status: 'loading' }
  else if (result.symbols) directory = { status: 'ready', symbols: result.symbols }
  else directory = { status: 'error', message: result.message ?? '' }

  return { directory, load }
}
