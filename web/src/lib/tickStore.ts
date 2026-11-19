import type { Trade } from '../types'

/** Lebar satu kelompok sparkline (sama dengan `BUCKET_MS` di server/src/history.rs). */
export const BUCKET_MS = 10_000
/** 180 × 10 detik = 30 menit, sama dengan riwayat yang disimpan server. */
export const MAX_POINTS = 180

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
  /** Arah perubahan harga terakhir; `null` sampai harganya pertama kali berubah. Trade dengan
   *  harga yang sama nggak mengubahnya, jadi yang menandai perubahan baru adalah `flashSeq`. */
  flash: 'up' | 'down' | null
  /** Naik tiap kali `flash` diisi; dipakai sebagai `key` untuk memutar ulang animasi. */
  flashSeq: number
}

export type LiveTicks = Readonly<Record<string, LiveTick>>

/** Titik sparkline per simbol: harga terakhir tiap kelompok 10 detik, urut dari yang lama. */
export type Sparks = Readonly<Record<string, readonly number[]>>

/** Sparkline yang sedang dibangun: `times[i]` = awal kelompok (Unix ms) untuk `prices[i]`. */
interface SparkDraft {
  times: number[]
  prices: number[]
}

/**
 * Penyimpan tick di LUAR React.
 *
 * Trade bisa datang ratusan kali per detik. Kalau tiap trade memanggil `setState`, React
 * render ulang sebanyak itu. Di sini trade cuma mengubah `drafts` (objek biasa, mutable),
 * lalu paling banyak sekali per frame layar (`requestAnimationFrame`) dibuat snapshot baru
 * dan pendengar dikabari. Bentuk subscribe/getSnapshot ini yang dipakai `useSyncExternalStore`.
 *
 * Sparkline disimpan terpisah dari data live: riwayat dari server (`seed`) bisa ada sebelum
 * trade pertama datang, misalnya malam hari, dan saat itu belum ada harga live yang benar.
 */
export class TickStore {
  private drafts = new Map<string, LiveTick>()
  private dirty = new Set<string>()
  private snapshot: LiveTicks = {}
  private sparks = new Map<string, SparkDraft>()
  private sparksDirty = new Set<string>()
  private sparksSnapshot: Sparks = {}
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
      }
      this.dirty.add(trade.symbol)
      this.addPoint(trade.symbol, trade.timestamp, trade.price)
    }
    this.schedule()
  }

  /**
   * Isi sparkline dari riwayat server (`[awalKelompok, harga][]`).
   *
   * Riwayat server jadi dasarnya; titik lokal yang LEBIH BARU dari riwayat itu disambung di
   * belakang. Setelah reconnect biasa, riwayat server sudah memuat semuanya, jadi ekornya
   * nggak dobel. Setelah server restart (deploy), riwayatnya kosong, jadi garis yang sudah
   * ada di tab ini tetap utuh.
   */
  seed(symbol: string, points: readonly (readonly [number, number])[]) {
    const times = points.map(([time]) => time)
    const prices = points.map(([, price]) => price)
    const local = this.sparks.get(symbol)
    const newest = times.at(-1) ?? -Infinity
    local?.times.forEach((time, i) => {
      if (time > newest) {
        times.push(time)
        prices.push(local.prices[i])
      }
    })
    this.sparks.set(symbol, {
      times: times.slice(-MAX_POINTS),
      prices: prices.slice(-MAX_POINTS),
    })
    this.sparksDirty.add(symbol)
    this.schedule()
  }

  /** Aturan yang sama dengan `History::record` di server. */
  private addPoint(symbol: string, timestamp: number, price: number) {
    const start = Math.floor(timestamp / BUCKET_MS) * BUCKET_MS
    const spark = this.sparks.get(symbol)
    // Riwayat kosong: trade pertama mana pun dianggap kelompok baru.
    const newest = spark?.times.at(-1) ?? -Infinity
    if (!spark) {
      this.sparks.set(symbol, { times: [start], prices: [price] })
    } else if (start === newest) {
      spark.prices[spark.prices.length - 1] = price
    } else if (start > newest) {
      spark.times.push(start)
      spark.prices.push(price)
      if (spark.prices.length > MAX_POINTS) {
        spark.times.shift()
        spark.prices.shift()
      }
    } else {
      // Trade telat (lebih lama dari kelompok terbaru) nggak boleh mengacak garis.
      return
    }
    this.sparksDirty.add(symbol)
  }

  private schedule() {
    if ((this.dirty.size > 0 || this.sparksDirty.size > 0) && this.frame === 0) {
      this.frame = requestAnimationFrame(this.flush)
    }
  }

  /** Arrow function supaya `this` tetap benar saat dioper ke `requestAnimationFrame`. */
  private flush = () => {
    this.frame = 0
    // Snapshot cuma diganti kalau isinya berubah, supaya komponen yang membaca snapshot
    // lainnya nggak ikut render ulang.
    if (this.dirty.size > 0) {
      const next: Record<string, LiveTick> = { ...this.snapshot }
      for (const symbol of this.dirty) next[symbol] = { ...this.drafts.get(symbol)! }
      this.dirty.clear()
      this.snapshot = next
    }
    if (this.sparksDirty.size > 0) {
      const next: Record<string, readonly number[]> = { ...this.sparksSnapshot }
      for (const symbol of this.sparksDirty) next[symbol] = this.sparks.get(symbol)!.prices.slice()
      this.sparksDirty.clear()
      this.sparksSnapshot = next
    }
    for (const listener of this.listeners) listener()
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  getSnapshot = () => this.snapshot

  getSparks = () => this.sparksSnapshot

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
