import { useCallback } from 'react'

import { errorMessage } from './api/client'
import { AddSymbolForm } from './components/AddSymbolForm'
import { Header } from './components/Header'
import { StockCard } from './components/StockCard'
import { Toasts } from './components/Toasts'
import { useLiveTicks } from './hooks/useLiveTicks'
import { useStockSnapshots } from './hooks/useStockSnapshots'
import { useToasts } from './hooks/useToasts'
import { useWatchlist } from './hooks/useWatchlist'

/** Dipakai selama watchlist belum termuat; konstanta supaya referensinya stabil. */
const NO_SYMBOLS: readonly string[] = []

export function App() {
  const { toasts, push, dismiss } = useToasts()
  const watchlist = useWatchlist()
  const symbols = watchlist.symbols ?? NO_SYMBOLS
  const snapshots = useStockSnapshots(symbols)
  const { ticks, connection, ticksPerSecond } = useLiveTicks(symbols, push)

  const { remove } = watchlist
  const handleRemove = useCallback(
    (symbol: string) => {
      remove(symbol).catch((err: unknown) => push(errorMessage(err)))
    },
    [remove, push],
  )

  return (
    <>
      <Header connection={connection} ticksPerSecond={ticksPerSecond} />
      <main>
        <div className="toolbar-row">
          <AddSymbolForm onAdd={watchlist.add} onError={push} />
        </div>

        {watchlist.loadError && (
          <p className="down">Gagal memuat watchlist: {watchlist.loadError}</p>
        )}
        {watchlist.symbols?.length === 0 && (
          <p className="muted">Watchlist kosong. Tambah ticker di atas.</p>
        )}

        <div className="grid">
          {symbols.map((symbol) => (
            <StockCard
              key={symbol}
              symbol={symbol}
              snapshot={snapshots[symbol]}
              live={ticks[symbol]}
              onRemove={handleRemove}
            />
          ))}
        </div>
      </main>
      <Toasts toasts={toasts} onDismiss={dismiss} />
    </>
  )
}
