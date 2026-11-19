import { useCallback } from 'react'

import { errorMessage } from './api/client'
import { AddSymbolForm } from './components/AddSymbolForm'
import { Header } from './components/Header'
import { OwnerAccess } from './components/OwnerAccess'
import { QuoteBoard } from './components/QuoteBoard'
import { SessionBar } from './components/SessionBar'
import { Toasts } from './components/Toasts'
import { EmptyWatchlist, WatchlistLoadError } from './components/WatchlistNotice'
import { useLiveTicks } from './hooks/useLiveTicks'
import { isUnauthorized, useOwner } from './hooks/useOwner'
import { useStockSnapshots } from './hooks/useStockSnapshots'
import { useToasts } from './hooks/useToasts'
import { useWatchlist } from './hooks/useWatchlist'

/** Dipakai selama watchlist belum termuat; konstanta supaya referensinya stabil. */
const NO_SYMBOLS: readonly string[] = []

/** Sama dengan `MAX_SYMBOLS` di server/src/hub.rs. */
const MAX_SYMBOLS = 50

export function App() {
  const { toasts, push, dismiss } = useToasts()
  const owner = useOwner()
  const watchlist = useWatchlist(owner.token)
  const symbols = watchlist.symbols ?? NO_SYMBOLS
  const snapshots = useStockSnapshots(symbols)
  const { ticks, connection, ticksPerSecond } = useLiveTicks(symbols, push)

  // Server menolak token (mis. sudah diganti): keluar dari mode pemilik, lalu tampilkan pesannya.
  const { signOut } = owner
  const reportWriteError = useCallback(
    (err: unknown) => {
      if (isUnauthorized(err)) signOut()
      push(errorMessage(err))
    },
    [signOut, push],
  )

  const { add, remove } = watchlist
  const handleAdd = useCallback(
    async (symbol: string) => {
      try {
        await add(symbol)
      } catch (err) {
        if (isUnauthorized(err)) signOut()
        throw err
      }
    },
    [add, signOut],
  )
  const handleRemove = useCallback(
    (symbol: string) => {
      remove(symbol).catch(reportWriteError)
    },
    [remove, reportWriteError],
  )
  const canEdit = owner.token !== null

  let content
  if (watchlist.loadError) {
    content = <WatchlistLoadError message={watchlist.loadError} onRetry={watchlist.reload} />
  } else if (watchlist.symbols === null) {
    content = <p className="label">Memuat watchlist…</p>
  } else if (symbols.length === 0) {
    content = <EmptyWatchlist canEdit={canEdit} />
  } else {
    content = (
      <QuoteBoard
        symbols={symbols}
        snapshots={snapshots}
        ticks={ticks}
        onRemove={canEdit ? handleRemove : undefined}
      />
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
          {canEdit && <AddSymbolForm onAdd={handleAdd} onError={push} watched={symbols} />}
        </div>
        {content}
        <p className="footnote">
          Data dari Finnhub. Persentase dihitung dari harga penutupan kemarin.
        </p>
        <OwnerAccess owner={owner} />
      </main>
      <Toasts toasts={toasts} onDismiss={dismiss} />
    </>
  )
}
