import { useCallback, useState } from 'react'

import { errorMessage } from './api/client'
import { AddSymbolForm } from './components/AddSymbolForm'
import { Header } from './components/Header'
import { StockCard } from './components/StockCard'
import { useLiveTicks } from './hooks/useLiveTicks'
import { useStockSnapshots } from './hooks/useStockSnapshots'
import { useWatchlist } from './hooks/useWatchlist'

/** Dipakai selama watchlist belum termuat; konstanta supaya referensinya stabil. */
const NO_SYMBOLS: readonly string[] = []

export function App() {
  const watchlist = useWatchlist()
  const symbols = watchlist.symbols ?? NO_SYMBOLS
  const snapshots = useStockSnapshots(symbols)
  const { ticks, connection, ticksPerSecond } = useLiveTicks(symbols)
  // Pesan error terakhir (sementara; jadi toast di M9).
  const [notice, setNotice] = useState<string | null>(null)

  const { add, remove } = watchlist
  const handleAdd = useCallback(
    async (symbol: string) => {
      await add(symbol)
      setNotice(null)
    },
    [add],
  )
  const handleRemove = useCallback(
    (symbol: string) => {
      remove(symbol).catch((err: unknown) => setNotice(errorMessage(err)))
    },
    [remove],
  )

  return (
    <>
      <Header connection={connection} ticksPerSecond={ticksPerSecond} />
      <main>
        <div className="toolbar-row">
          <AddSymbolForm onAdd={handleAdd} onError={setNotice} />
          {notice && <span className="notice down">{notice}</span>}
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
    </>
  )
}
