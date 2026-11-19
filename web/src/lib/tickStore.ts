import type { Trade } from '../types'

/** Titik sparkline yang disimpan per simbol (sama dengan PoC). */
const MAX_POINTS = 300

/** Data live satu simbol, dalam bentuk yang dibaca React. Objeknya nggak pernah diubah;
 *  tiap perubahan menghasilkan objek baru, jadi React cukup membandingkan referensi. */
export interface LiveTick {
  price: number
  /** Unix ms trade terakhir. */
  timestamp: number
  /** Total volume sejak halaman dibuka. */
  volume: number
  high: number
  low: number
  points: readonly number[]
  /** Arah perubahan harga terakhir; `null` sampai harganya pertama kali berubah. Trade dengan
   *  harga yang sama nggak mengubahnya, jadi yang menandai perubahan baru adalah `flashSeq`. */
  flash: 'up' | 'down' | null
  /** Naik tiap kali `flash` diisi; dipakai sebagai `key` untuk memutar ulang animasi. */
  flashSeq: number
}

export type LiveTicks = Readonly<Record<string, LiveTick>>

type Draft = Omit<LiveTick, 'points'> & { points: number[] }

/**
 * Penyimpan tick di LUAR React.
 *
 * Trade bisa datang ratusan kali per detik. Kalau tiap trade memanggil `setState`, React
 * render ulang sebanyak itu. Di sini trade cuma mengubah `drafts` (objek biasa, mutable),
 * lalu paling banyak sekali per frame layar (`requestAnimationFrame`) dibuat snapshot baru
 * dan pendengar dikabari. Bentuk subscribe/getSnapshot ini yang dipakai `useSyncExternalStore`.
 */
export class TickStore {
  private drafts = new Map<string, Draft>()
  private dirty = new Set<string>()
  private snapshot: LiveTicks = {}
  private listeners = new Set<() => void>()
  private frame = 0
  /** Jumlah trade sejak terakhir dibaca `takeTradeCount`. */
  private tradeCount = 0

  apply(trades: readonly Trade[]) {
    for (const trade of trades) {
      this.tradeCount++
      const draft = this.drafts.get(trade.symbol)
      if (!draft) {
        this.drafts.set(trade.symbol, {
          price: trade.price,
          timestamp: trade.timestamp,
          volume: trade.volume,
          high: trade.price,
          low: trade.price,
          points: [trade.price],
          flash: null,
          flashSeq: 0,
        })
      } else {
        if (trade.price !== draft.price) {
          draft.flash = trade.price > draft.price ? 'up' : 'down'
          draft.flashSeq++
        }
        draft.price = trade.price
        draft.timestamp = trade.timestamp
        draft.volume += trade.volume
        draft.high = Math.max(draft.high, trade.price)
        draft.low = Math.min(draft.low, trade.price)
        draft.points.push(trade.price)
        if (draft.points.length > MAX_POINTS) draft.points.shift()
      }
      this.dirty.add(trade.symbol)
    }
    if (this.dirty.size > 0 && this.frame === 0) {
      this.frame = requestAnimationFrame(this.flush)
    }
  }

  /** Arrow function supaya `this` tetap benar saat dioper ke `requestAnimationFrame`. */
  private flush = () => {
    this.frame = 0
    const next: Record<string, LiveTick> = { ...this.snapshot }
    for (const symbol of this.dirty) {
      const draft = this.drafts.get(symbol)!
      next[symbol] = { ...draft, points: draft.points.slice() }
    }
    this.dirty.clear()
    this.snapshot = next
    for (const listener of this.listeners) listener()
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  getSnapshot = () => this.snapshot

  takeTradeCount(): number {
    const count = this.tradeCount
    this.tradeCount = 0
    return count
  }

  dispose() {
    cancelAnimationFrame(this.frame)
    this.frame = 0
  }
}
