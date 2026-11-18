import { Header } from './components/Header'
import { StockCard } from './components/StockCard'
import { useStockSnapshots } from './hooks/useStockSnapshots'

// Sementara hardcoded; di M8 pindah ke watchlist di PostgreSQL.
// Didefinisikan di luar komponen supaya referensinya stabil antar-render.
const SYMBOLS: readonly string[] = ['AAPL', 'NVDA', 'MSFT', 'TSLA', 'AMZN', 'GOOGL', 'META', 'SPY']

export function App() {
  const snapshots = useStockSnapshots(SYMBOLS)

  return (
    <>
      <Header />
      <main>
        <div className="grid">
          {SYMBOLS.map((symbol) => (
            <StockCard key={symbol} symbol={symbol} snapshot={snapshots[symbol]} />
          ))}
        </div>
      </main>
    </>
  )
}
