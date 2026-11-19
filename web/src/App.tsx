import { useCallback } from 'react'

import { errorMessage } from './api/client'
import { AddSymbolForm } from './components/AddSymbolForm'
import { Header } from './components/Header'
import { QuoteBoard } from './components/QuoteBoard'
import { SessionBar } from './components/SessionBar'
import { Toasts } from './components/Toasts'
import { EmptyWatchlist, WatchlistLoadError } from './components/WatchlistNotice'
import { useLiveTicks } from './hooks/useLiveTicks'
import { useStockSnapshots } from './hooks/useStockSnapshots'
import { useToasts } from './hooks/useToasts'
import { useWatchlist } from './hooks/useWatchlist'

/** Dipakai selama watchlist belum termuat; konstanta supaya referensinya stabil. */
const NO_SYMBOLS: readonly string[] = []

/** Sama dengan `MAX_SYMBOLS` di server/src/hub.rs. */
const MAX_SYMBOLS = 50

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

  let content
  if (watchlist.loadError) {
    content = <WatchlistLoadError message={watchlist.loadError} onRetry={watchlist.reload} />
  } else if (watchlist.symbols === null) {
    content = <p className="label">Memuat watchlist…</p>
  } else if (symbols.length === 0) {
    content = <EmptyWatchlist />
  } else {
    content = (
      <QuoteBoard symbols={symbols} snapshots={snapshots} ticks={ticks} onRemove={handleRemove} />
    )
  }

  return (
    <>
      <Header connection={connection} ticksPerSecond={ticksPerSecond} />
      <SessionBar />
      <main className="page">
        <div className="watchlist-head">
          <h1>
            Watchlist
            {watchlist.symbols && (
              <span>
                {symbols.length} dari maks {MAX_SYMBOLS}
              </span>
            )}
          </h1>
          <AddSymbolForm onAdd={watchlist.add} onError={push} />
        </div>
        {content}
        <p className="footnote">
          Data dari Finnhub. Persentase dihitung dari harga penutupan kemarin.
        </p>
      </main>
      <Toasts toasts={toasts} onDismiss={dismiss} />
    </>
  )
}
