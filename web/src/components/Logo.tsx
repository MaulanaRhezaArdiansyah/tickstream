/**
 * Logo Tickstream: harga yang naik per transaksi, berakhir di titik kuning (harga terakhir).
 * Kuningnya sama dengan penanda jam sekarang di bar sesi. Favicon (public/favicon.svg)
 * memakai bentuk yang sama dengan garis lebih tebal untuk ukuran 16px.
 */
export function Logo({ size = 28 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
      <rect x="0.5" y="0.5" width="63" height="63" rx="14" fill="#0e1412" stroke="#2d3a36" />
      <path d="M11 47h12V36h11V25h10" fill="none" stroke="#ebe5d8" strokeWidth="7" />
      <circle cx="49" cy="25" r="6" fill="#e8a53a" />
    </svg>
  )
}
