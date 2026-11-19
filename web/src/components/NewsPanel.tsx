import { useEffect, useRef } from 'react'

import type { NewsState } from '../hooks/useNews'
import { useNow } from '../hooks/useNow'
import { fmt, splitSymbol } from '../lib/format'
import { relativeTime } from '../lib/relativeTime'
import { Change } from './QuoteRow'
import { XIcon } from './XIcon'

interface NewsPanelProps {
  symbol: string
  name: string | undefined
  /** Harga live yang sama dengan barisnya. */
  price: number | null
  prevClose: number | null
  /** `undefined` = belum pernah diambil; tampil sebagai memuat. */
  state: NewsState | undefined
  onClose: () => void
  onRetry: () => void
}

/** Panel samping: harga simbol yang dipilih dan berita 7 hari terakhirnya. */
export function NewsPanel({
  symbol,
  name,
  price,
  prevClose,
  state,
  onClose,
  onRetry,
}: NewsPanelProps) {
  const closeButton = useRef<HTMLButtonElement>(null)
  // Diperbarui tiap detik, jadi "baru saja" berubah sendiri jadi "1 menit lalu".
  const now = useNow().getTime()

  // Fokus pindah ke panel waktu dibuka, supaya pengguna keyboard langsung ada di dalamnya.
  useEffect(() => closeButton.current?.focus(), [])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [onClose])

  const { ticker } = splitSymbol(symbol)
  const change = price != null && prevClose ? price - prevClose : null

  return (
    <aside className="news-panel" aria-label={`Berita ${symbol}`}>
      <header className="news-head">
        <div className="min0">
          <h2>{ticker}</h2>
          <div className="sub">{name ?? symbol}</div>
        </div>
        <div className="news-price">
          <span className="price">{fmt(price)}</span>
          {change != null && prevClose && (
            <span className={`chg ${change >= 0 ? 'up' : 'down'}`}>
              <Change change={change} prevClose={prevClose} />
            </span>
          )}
        </div>
        <button
          ref={closeButton}
          className="icon-btn news-close"
          type="button"
          aria-label="Tutup berita"
          onClick={onClose}
        >
          <XIcon />
          <span className="close-text">Tutup</span>
        </button>
      </header>

      {/* Tempat grafik intraday (fitur berikutnya). */}
      <div className="chart-slot" />

      <NewsBody state={state} now={now} onRetry={onRetry} />
    </aside>
  )
}

interface NewsBodyProps {
  state: NewsState | undefined
  now: number
  onRetry: () => void
}

function NewsBody({ state, now, onRetry }: NewsBodyProps) {
  if (!state || state.status === 'loading') return <p className="news-state">Memuat berita…</p>
  if (state.status === 'error') {
    return (
      <div className="news-state" role="alert">
        <p>Berita nggak bisa dimuat: {state.message}.</p>
        <button className="btn" type="button" onClick={onRetry}>
          Coba lagi
        </button>
      </div>
    )
  }
  if (state.news.items.length === 0) {
    return <p className="news-state">Belum ada berita 7 hari terakhir</p>
  }

  return (
    <ul className="news-list">
      {state.news.items.map((item) => (
        <li key={`${item.id}-${item.url}`} className="news-item">
          <div className="news-meta">
            {relativeTime(item.datetime, now)}
            {item.source && ` · ${item.source}`}
          </div>
          <a href={item.url} target="_blank" rel="noopener noreferrer">
            {item.headline}
          </a>
          {item.summary && <p className="clamp-3">{item.summary}</p>}
        </li>
      ))}
    </ul>
  )
}
