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
