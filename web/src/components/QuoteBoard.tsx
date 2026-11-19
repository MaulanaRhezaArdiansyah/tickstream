import type { NewsState } from '../hooks/useNews'
import type { Snapshot } from '../hooks/useStockSnapshots'
import type { LiveTicks, Sparks } from '../lib/tickStore'
import { QuoteRow } from './QuoteRow'

interface QuoteBoardProps {
  symbols: readonly string[]
  snapshots: Record<string, Snapshot>
  ticks: LiveTicks
  sparks: Sparks
  /** Nggak diisi = papan tanpa tombol hapus (pengunjung). */
  onRemove?: (symbol: string) => void
  news: Readonly<Record<string, NewsState>>
  selected: string | null
  /** Harus fungsi yang stabil. */
  onSelect: (symbol: string) => void
}

function newsCount(state: NewsState | undefined): number | undefined {
  return state?.status === 'ok' ? state.news.count24h : undefined
}

export function QuoteBoard({
  symbols,
  snapshots,
  ticks,
  sparks,
  onRemove,
  news,
  selected,
  onSelect,
}: QuoteBoardProps) {
  return (
    <div className="board">
      {/* Judul kolom cuma visual; isi tiap baris sudah terbaca tanpa judulnya. */}
      <div className="cols board-head" aria-hidden="true">
        <span>Simbol</span>
        <span className="num">Harga</span>
        <span className="num">Sejak tutup kemarin</span>
        <span className="range hide-narrow">Rentang hari ini</span>
        <span className="hide-narrow">30 menit terakhir</span>
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
            points={sparks[symbol]}
            onRemove={onRemove}
            newsCount={newsCount(news[symbol])}
            selected={symbol === selected}
            onSelect={onSelect}
          />
        ))}
      </ul>
    </div>
  )
}
