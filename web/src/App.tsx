import { Header } from './components/Header'
import { StockCard } from './components/StockCard'
import { useLiveTicks } from './hooks/useLiveTicks'
import { useStockSnapshots } from './hooks/useStockSnapshots'

// Sementara hardcoded; di M8 pindah ke watchlist di PostgreSQL.
// Didefinisikan di luar komponen supaya referensinya stabil antar-render.
// BTC diperdagangkan 24 jam, jadi tetap ada tick walau bursa US tutup.
const SYMBOLS: readonly string[] = [
  'AAPL',
  'NVDA',
  'MSFT',
  'TSLA',
  'AMZN',
  'GOOGL',
  'META',
  'SPY',
  'BINANCE:BTCUSDT',
]

export function App() {
  const snapshots = useStockSnapshots(SYMBOLS)
  const { ticks, connection, ticksPerSecond } = useLiveTicks(SYMBOLS)

  return (
    <>
      <Header connection={connection} ticksPerSecond={ticksPerSecond} />
      <main>
        <div className="grid">
          {SYMBOLS.map((symbol) => (
            <StockCard
              key={symbol}
              symbol={symbol}
              snapshot={snapshots[symbol]}
              live={ticks[symbol]}
            />
          ))}
        </div>
      </main>
    </>
  )
}
