// Bentuk JSON dari server (lihat server/src/market/mod.rs).
// Kalau struct di sana berubah, ubah juga di sini.

export interface Quote {
  price: number
  open: number
  high: number
  low: number
  prevClose: number
  /** Unix timestamp dalam detik. */
  timestamp: number
}

export interface Profile {
  name: string
}

export interface MarketStatus {
  isOpen: boolean
  /** "pre-market" | "regular" | "post-market", atau null saat tutup. */
  session: string | null
}

/** Satu transaksi dari `/ws` (lihat `Trade` di server). */
export interface Trade {
  symbol: string
  price: number
  /** Unix timestamp dalam milidetik. */
  timestamp: number
  volume: number
}

/** Pesan dari `/ws` (lihat `ServerMessage` di server/src/routes/ws.rs). */
export type ServerMessage =
  | { type: 'trades'; data: Trade[] }
  | { type: 'error'; symbol: string | null; message: string }
