/** Ikon silang untuk tombol hapus/tutup. Garis SVG, bukan karakter "✕", supaya
 *  ukurannya sama di semua font. */
export function XIcon({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
      <path d="M3 3l8 8M11 3l-8 8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  )
}
