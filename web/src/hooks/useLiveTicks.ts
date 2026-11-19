import { useEffect, useRef, useState, useSyncExternalStore } from 'react'

import { TickStore, type LiveTicks } from '../lib/tickStore'
import type { ServerMessage } from '../types'

export type ConnectionState = 'connecting' | 'live' | 'reconnecting'

export interface LiveTicksState {
  ticks: LiveTicks
  connection: ConnectionState
  ticksPerSecond: number
}

const MIN_RETRY_MS = 1000
const MAX_RETRY_MS = 30_000

/**
 * Tick real-time dari `/ws` untuk `symbols`. Satu koneksi per pemanggil; render ulang
 * paling banyak sekali per frame berkat `TickStore`.
 *
 * `symbols` boleh berubah (M8: tambah/hapus watchlist): selisihnya dikirim sebagai
 * subscribe/unsubscribe tanpa membuka koneksi baru.
 *
 * `onError` menerima pesan error dari server (mis. simbol ditolak karena penuh).
 */
export function useLiveTicks(
  symbols: readonly string[],
  onError?: (message: string) => void,
): LiveTicksState {
  // Lazy initializer: store dibuat sekali seumur komponen, bukan tiap render.
  const [store] = useState(() => new TickStore())
  const ticks = useSyncExternalStore(store.subscribe, store.getSnapshot)
  const [connection, setConnection] = useState<ConnectionState>('connecting')
  const [ticksPerSecond, setTicksPerSecond] = useState(0)

  // Ref = kotak mutable yang selamat antar-render tapi nggak memicu render saat diubah.
  // Efek koneksi butuh daftar simbol TERBARU saat (re)connect, tanpa harus ikut
  // dijalankan ulang (dan memutus koneksi) setiap kali daftarnya berubah.
  const socketRef = useRef<WebSocket | null>(null)
  const symbolsRef = useRef(symbols)
  const sentRef = useRef(new Set<string>())
  // Callback terbaru disimpan di ref, supaya mengganti callback nggak memutus koneksi.
  const onErrorRef = useRef(onError)
  useEffect(() => {
    onErrorRef.current = onError
  })

  // Koneksi: dibuka sekali saat mount, dibuka ulang dengan backoff kalau putus.
  useEffect(() => {
    let retryMs = MIN_RETRY_MS
    let retryTimer: ReturnType<typeof setTimeout> | undefined
    let disposed = false

    const connect = () => {
      const protocol = location.protocol === 'https:' ? 'wss' : 'ws'
      const socket = new WebSocket(`${protocol}://${location.host}/ws`)
      socketRef.current = socket

      socket.onopen = () => {
        retryMs = MIN_RETRY_MS
        setConnection('live')
        // Koneksi baru = langganan di server kosong lagi; kirim ulang semuanya.
        sentRef.current = new Set(symbolsRef.current)
        for (const symbol of sentRef.current) send(socket, 'subscribe', symbol)
      }

      socket.onmessage = (event) => {
        const message = JSON.parse(event.data as string) as ServerMessage
        if (message.type === 'trades') store.apply(message.data)
        else
          onErrorRef.current?.(
            message.symbol ? `${message.symbol}: ${message.message}` : message.message,
          )
      }

      socket.onclose = () => {
        // Cek dulu: di StrictMode, close socket lama bisa datang SETELAH socket baru dibuat.
        if (socketRef.current === socket) socketRef.current = null
        if (disposed) return
        setConnection('reconnecting')
        retryTimer = setTimeout(connect, retryMs)
        retryMs = Math.min(retryMs * 2, MAX_RETRY_MS)
      }
    }

    connect()
    const rateTimer = setInterval(() => setTicksPerSecond(store.takeTradeCount()), 1000)

    return () => {
      disposed = true
      clearTimeout(retryTimer)
      clearInterval(rateTimer)
      socketRef.current?.close()
      store.dispose()
    }
  }, [store])

  // Daftar simbol berubah: kirim selisihnya saja lewat koneksi yang sudah ada.
  useEffect(() => {
    symbolsRef.current = symbols
    const socket = socketRef.current
    if (!socket || socket.readyState !== WebSocket.OPEN) return // onopen yang akan mengirim

    const wanted = new Set(symbols)
    const sent = sentRef.current
    for (const symbol of wanted) if (!sent.has(symbol)) send(socket, 'subscribe', symbol)
    for (const symbol of sent) if (!wanted.has(symbol)) send(socket, 'unsubscribe', symbol)
    sentRef.current = wanted
  }, [symbols])

  return { ticks, connection, ticksPerSecond }
}

function send(socket: WebSocket, type: 'subscribe' | 'unsubscribe', symbol: string) {
  socket.send(JSON.stringify({ type, symbol }))
}
