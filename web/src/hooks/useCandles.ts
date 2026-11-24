import { useCallback, useEffect, useRef, useState } from 'react'

import { errorMessage, fetchCandles } from '../api/client'
import type { CandleRange, CandleSeries } from '../types'
import type { ConnectionState } from './useLiveTicks'

export type CandlesState =
  | { status: 'loading' }
  | { status: 'ok'; series: CandleSeries }
  | { status: 'error'; message: string }

// Satu objek tetap: objek baru tiap render bikin chart di-reset tiap ada tick.
const LOADING: CandlesState = { status: 'loading' }

/**
 * Ambil data candle historis dari server dan perbarui ulang saat WebSocket tersambung
 * kembali setelah putus (`reconnecting` → `live`), supaya lubang selama offline tertutup.
 */
export function useCandles(
  symbol: string,
  range: CandleRange,
  connection: ConnectionState,
): { state: CandlesState; retry: () => void } {
  const key = `${symbol}\0${range}`
  const [settled, setSettled] = useState<{ key: string; state: CandlesState }>({
    key,
    state: LOADING,
  })
  // Naik tiap kali user klik "Coba lagi" atau koneksi pulih dari `reconnecting`.
  const [attempt, setAttempt] = useState(0)
  const prevConnection = useRef(connection)

  useEffect(() => {
    const wasReconnecting = prevConnection.current === 'reconnecting'
    prevConnection.current = connection
    if (wasReconnecting && connection === 'live') {
      setAttempt((n) => n + 1)
    }
  }, [connection])

  useEffect(() => {
    const controller = new AbortController()
    const { signal } = controller

    void (async () => {
      try {
        const series = await fetchCandles(symbol, range, signal)
        if (!signal.aborted) {
          setSettled({ key, state: { status: 'ok', series } })
        }
      } catch (err) {
        if (signal.aborted) return
        setSettled((prev) =>
          prev.key === key && prev.state.status === 'ok'
            ? prev
            : { key, state: { status: 'error', message: errorMessage(err) } },
        )
      }
    })()

    return () => controller.abort()
  }, [key, symbol, range, attempt])

  const retry = useCallback(() => {
    setSettled((prev) =>
      prev.key === key && prev.state.status === 'ok'
        ? prev
        : { key, state: LOADING },
    )
    setAttempt((n) => n + 1)
  }, [key])

  // Saat simbol/range berganti, langsung kembali ke `loading` di render yang sama tanpa
  // perlu memanggil `setState` sinkron di dalam `useEffect`.
  const state: CandlesState = settled.key === key ? settled.state : LOADING

  return { state, retry }
}
