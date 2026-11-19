import { MAX_POINTS } from './tickStore'

/**
 * Posisi x titik ke-`index` dari `count` titik di kanvas selebar `width`.
 *
 * Sumbunya waktu, bukan urutan: titik terakhir selalu di kanan dan satu titik = 10 detik dari
 * 30 menit. Baris yang riwayatnya baru sedikit (misalnya setelah deploy) menempel di kanan
 * dan nggak direntang selebar kolom, jadi semua baris tetap bisa dibandingkan.
 */
export function sparkX(index: number, count: number, width: number): number {
  return ((index + MAX_POINTS - count) / (MAX_POINTS - 1)) * width
}
