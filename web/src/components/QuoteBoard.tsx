import type { Snapshot } from '../hooks/useStockSnapshots'
import type { LiveTicks } from '../lib/tickStore'
import { QuoteRow } from './QuoteRow'

interface QuoteBoardProps {
  symbols: readonly string[]
  snapshots: Record<string, Snapshot>
  ticks: LiveTicks
  onRemove: (symbol: string) => void
}

export function QuoteBoard({ symbols, snapshots, ticks, onRemove }: QuoteBoardProps) {
  return (
    <div className="board">
      {/* Judul kolom cuma visual; isi tiap baris sudah terbaca tanpa judulnya. */}
      <div className="cols board-head" aria-hidden="true">
        <span>Simbol</span>
        <span className="num">Harga</span>
        <span className="num">Sejak tutup kemarin</span>
        <span className="range hide-narrow">Rentang hari ini</span>
        <span className="hide-narrow">Sejak halaman dibuka</span>
        <span className="num hide-narrow">Volume</span>
        <span />
      </div>
      <ul className="rows" aria-label="Watchlist">
        {symbols.map((symbol) => (
          <QuoteRow
            key={symbol}
            symbol={symbol}
            snapshot={snapshots[symbol]}
            live={ticks[symbol]}
            onRemove={onRemove}
          />
        ))}
      </ul>
    </div>
  )
}
