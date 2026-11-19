import { useState } from 'react'

interface CompanyLogoProps {
  src: string | null | undefined
  ticker: string
}

/**
 * Logo perusahaan dari Finnhub, atau huruf pertama ticker kalau logonya nggak ada
 * (crypto, ETF tertentu) atau gagal dimuat. Kotaknya selalu sama besar, supaya
 * kolom nama tetap sejajar.
 */
export function CompanyLogo({ src, ticker }: CompanyLogoProps) {
  // Simpan URL yang gagal, bukan boolean: kalau URL-nya berganti, coba lagi.
  const [failed, setFailed] = useState<string | null>(null)

  if (!src || failed === src) {
    return (
      <span className="logo logo-letter" aria-hidden="true">
        {ticker.charAt(0)}
      </span>
    )
  }
  return (
    <img
      className="logo"
      src={src}
      alt=""
      width={28}
      height={28}
      loading="lazy"
      // Finnhub nggak perlu tahu halaman mana yang memuat logonya.
      referrerPolicy="no-referrer"
      onError={() => setFailed(src)}
    />
  )
}
