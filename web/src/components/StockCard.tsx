import { memo } from 'react'

import type { Snapshot } from '../hooks/useStockSnapshots'
import { fmt, fmtVol, sign } from '../lib/format'
import type { LiveTick } from '../lib/tickStore'
import { Sparkline } from './Sparkline'

const UP = '#22c55e'
const DOWN = '#ef4444'

interface StockCardProps {
  symbol: string
  snapshot: Snapshot | undefined
  live: LiveTick | undefined
}

/**
 * `memo`: kartu cuma dirender ulang kalau props-nya berubah (dibandingkan per referensi).
 * Tick BTC 60×/detik nggak ikut membuat kartu AAPL dirender ulang.
 */
export const StockCard = memo(function StockCard({ symbol, snapshot, live }: StockCardProps) {
  const quote = snapshot?.quote
  // Harga live menimpa harga quote; perubahan tetap dihitung dari prev close harian.
  const price = live?.price ?? quote?.price
  const change = price != null && quote?.prevClose ? price - quote.prevClose : null
  const direction = change == null ? '' : change >= 0 ? 'up' : 'down'

  let changeText = '–'
  if (snapshot?.error) {
    changeText = snapshot.error
  } else if (quote && change != null) {
    const pct = (change / quote.prevClose) * 100
    changeText = `${sign(change)}${fmt(change)} (${sign(pct)}${pct.toFixed(2)}%)`
  }

  const high = live && quote ? Math.max(quote.high, live.high) : (quote?.high ?? live?.high)
  // Quote yang kosong punya low 0; jangan sampai 0 dianggap harga terendah.
  const low = live && quote?.low ? Math.min(quote.low, live.low) : (quote?.low ?? live?.low)
  const timestamp = live ? live.timestamp : quote ? quote.timestamp * 1000 : null

  return (
    <div className="card">
      {live?.flash && (
        // `key` berubah → React membuang elemen lama dan membuat yang baru → animasi CSS
        // diputar dari awal. Cara "React" untuk memicu ulang animasi tanpa timer.
        <span key={live.flashSeq} className={`flash flash-${live.flash}`} aria-hidden />
      )}
      <div className="top">
        <div>
          <div className="sym">{symbol}</div>
          <div className="name">{snapshot?.name ?? ' '}</div>
        </div>
      </div>

      <div className="price">{price != null ? `$${fmt(price)}` : '–'}</div>
      <div className={`chg ${snapshot?.error ? 'down' : direction}`}>{changeText}</div>

      <Sparkline points={live?.points ?? NO_POINTS} color={direction === 'down' ? DOWN : UP} />

      <div className="meta">
        <Stat label="Open" value={quote?.open} />
        <Stat label="High" value={high} />
        <Stat label="Low" value={low} />
        <Stat label="Prev" value={quote?.prevClose} />
      </div>

      <div className="foot">
        <span>{live ? `Vol (sesi ini): ${fmtVol(live.volume)}` : 'Quote'}</span>
        <span>{timestamp != null ? new Date(timestamp).toLocaleTimeString('id-ID') : '–'}</span>
      </div>
    </div>
  )
})

/** Array kosong yang referensinya tetap, supaya efek gambar Sparkline nggak jalan sia-sia. */
const NO_POINTS: readonly number[] = []

function Stat({ label, value }: { label: string; value: number | undefined }) {
  return (
    <div>
      {label}
      <b>{fmt(value)}</b>
    </div>
  )
}
