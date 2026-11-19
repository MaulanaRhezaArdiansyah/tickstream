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
  /** URL logo (https), atau null kalau Finnhub nggak punya. */
  logo: string | null
}

/** Satu simbol dari `/api/symbols` (lihat `SymbolInfo` di server). */
export interface SymbolInfo {
  symbol: string
  /** Nama versi bursa, huruf besar semua (mis. "APPLE INC"). */
  name: string
  kind: 'stock' | 'etf'
}

export interface MarketStatus {
  isOpen: boolean
  /** "pre-market" | "regular" | "post-market", atau null saat tutup. */
  session: string | null
}

/** Satu berita dari `/api/news` (lihat `NewsItem` di server/src/market/news.rs). */
export interface NewsItem {
  id: number
  /** Unix timestamp dalam milidetik. */
  datetime: number
  headline: string
  source: string
  summary: string
  /** Selalu http(s); server membuang link lain. */
  url: string
}

export interface CompanyNews {
  /** Jumlah berita 24 jam terakhir (sebelum daftar dipotong ke 20). */
  count24h: number
  /** Terbaru di depan, paling banyak 20. */
  items: NewsItem[]
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
  /** Riwayat sparkline 30 menit: `[awal kelompok 10 detik (Unix ms), harga]`, urut dari lama. */
  | { type: 'history'; symbol: string; points: [number, number][] }
  | { type: 'error'; symbol: string | null; message: string }
