import type { Snapshot } from '../hooks/useStockSnapshots'
import { fmt, sign } from '../lib/format'

interface StockCardProps {
  symbol: string
  snapshot: Snapshot | undefined
}

export function StockCard({ symbol, snapshot }: StockCardProps) {
  const quote = snapshot?.quote
  const change = quote && quote.prevClose ? quote.price - quote.prevClose : null
  const direction = change == null ? '' : change >= 0 ? 'up' : 'down'

  let changeText = '–'
  if (snapshot?.error) {
    changeText = snapshot.error
  } else if (quote && change != null) {
    const pct = (change / quote.prevClose) * 100
    changeText = `${sign(change)}${fmt(change)} (${sign(pct)}${pct.toFixed(2)}%)`
  }

  return (
    <div className="card">
      <div className="top">
        <div>
          <div className="sym">{symbol}</div>
          <div className="name">{snapshot?.name ?? ' '}</div>
        </div>
      </div>

      <div className="price">{quote ? `$${fmt(quote.price)}` : '–'}</div>
      <div className={`chg ${snapshot?.error ? 'down' : direction}`}>{changeText}</div>

      <div className="meta">
        <Stat label="Open" value={quote?.open} />
        <Stat label="High" value={quote?.high} />
        <Stat label="Low" value={quote?.low} />
        <Stat label="Prev" value={quote?.prevClose} />
      </div>

      <div className="foot">
        <span>Quote</span>
        <span>{quote ? new Date(quote.timestamp * 1000).toLocaleTimeString('id-ID') : '–'}</span>
      </div>
    </div>
  )
}

function Stat({ label, value }: { label: string; value: number | undefined }) {
  return (
    <div>
      {label}
      <b>{fmt(value)}</b>
    </div>
  )
}
