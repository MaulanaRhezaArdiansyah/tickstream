import { memo } from 'react'

import type { Snapshot } from '../hooks/useStockSnapshots'
import { fmt, fmtVol, sign, splitSymbol } from '../lib/format'
import type { LiveTick } from '../lib/tickStore'
import { Sparkline } from './Sparkline'
import { XIcon } from './XIcon'

// Canvas nggak bisa membaca variabel CSS, jadi warnanya disalin dari index.css.
const UP = '#52c48c'
const DOWN = '#f2705f'
const NEUTRAL = '#9ba5a0'

interface QuoteRowProps {
  symbol: string
  snapshot: Snapshot | undefined
  live: LiveTick | undefined
  /**
   * Harus fungsi yang stabil (useCallback), kalau nggak `memo` jadi percuma.
   * Nggak diisi = tanpa tombol hapus (pengunjung).
   */
  onRemove?: (symbol: string) => void
}

/**
 * Satu baris di papan kutipan.
 *
 * `memo`: baris cuma dirender ulang kalau props-nya berubah (dibandingkan per referensi).
 * Tick BTC 60×/detik nggak ikut membuat baris AAPL dirender ulang.
 */
export const QuoteRow = memo(function QuoteRow({
  symbol,
  snapshot,
  live,
  onRemove,
}: QuoteRowProps) {
  const quote = snapshot?.quote
  // Harga live menimpa harga quote; perubahan tetap dihitung dari prev close harian.
  const price = live?.price ?? quote?.price
  // Quote kosong (simbol tanpa data) punya prevClose 0; jangan dibagi nol.
  const prevClose = quote?.prevClose || null
  const change = price != null && prevClose ? price - prevClose : null
  const direction = change == null ? null : change >= 0 ? 'up' : 'down'
  const loading = price == null && !snapshot?.error

  const high = live && quote ? Math.max(quote.high, live.high) : (quote?.high ?? live?.high)
  // Quote kosong punya low 0; jangan sampai 0 dianggap harga terendah.
  const low = live && quote?.low ? Math.min(quote.low, live.low) : quote?.low || live?.low

  const { ticker, venue } = splitSymbol(symbol)
  const name = snapshot?.name || venue || (loading ? 'Mengambil quote…' : ' ')

  return (
    <li className="row cols">
      <div className="min0">
        <div className="ticker">{ticker}</div>
        <div className="sub">{name}</div>
      </div>

      {price == null && snapshot?.error ? (
        <div className="row-error">{snapshot.error}</div>
      ) : (
        <>
          <div className="num">
            {price == null ? (
              <span className="skel wide" />
            ) : (
              // `key` berubah tiap harga berubah → React membuat <span> baru → animasi kedip
              // CSS diputar dari awal. Cara "React" memicu ulang animasi tanpa timer.
              <span
                key={live?.flashSeq ?? 0}
                className={live?.flash ? `price flash-${live.flash}` : 'price'}
              >
                {fmt(price)}
              </span>
            )}
          </div>
          <div className={`num chg ${direction ?? ''}`}>
            {change != null && prevClose ? (
              <Change change={change} prevClose={prevClose} />
            ) : loading ? (
              <span className="skel" />
            ) : (
              '–'
            )}
          </div>
          <div className="range hide-narrow">
            {price != null && high != null && low != null && (
              <DayRange price={price} low={low} high={high} />
            )}
          </div>
          <div className="hide-narrow">
            <Sparkline
              points={live?.points ?? NO_POINTS}
              color={direction === 'down' ? DOWN : direction === 'up' ? UP : NEUTRAL}
            />
          </div>
          <div className="num vol hide-narrow">{live ? fmtVol(live.volume) : '–'}</div>
        </>
      )}

      <div className="num">
        {onRemove && (
          <button
            className="icon-btn"
            type="button"
            title={`Hapus ${symbol}`}
            aria-label={`Hapus ${symbol} dari watchlist`}
            onClick={() => onRemove(symbol)}
          >
            <XIcon />
          </button>
        )}
      </div>
    </li>
  )
})

/** Array kosong yang referensinya tetap, supaya efek gambar Sparkline nggak jalan sia-sia. */
const NO_POINTS: readonly number[] = []

function Change({ change, prevClose }: { change: number; prevClose: number }) {
  const pct = (change / prevClose) * 100
  return (
    <>
      {/* Panah + tanda: arah tetap terbaca tanpa harus membedakan hijau dan merah. */}
      <b>
        <span aria-hidden="true">{change >= 0 ? '▲' : '▼'}</span> {sign(pct)}
        {pct.toFixed(2)}%
      </b>
      <small>
        {sign(change)}
        {fmt(change)}
      </small>
    </>
  )
}

/** Posisi harga sekarang di antara terendah dan tertinggi hari ini. */
function DayRange({ price, low, high }: { price: number; low: number; high: number }) {
  const position = high > low ? Math.min(1, Math.max(0, (price - low) / (high - low))) : 0.5
  return (
    <>
      <div className="range-track">
        <span className="range-mark" style={{ left: `${position * 100}%` }} />
      </div>
      <div className="range-ends">
        <span>{fmt(low)}</span>
        <span>{fmt(high)}</span>
      </div>
    </>
  )
}
