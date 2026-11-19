import { useCallback, useEffect, useRef, useState } from 'react'

import { errorMessage } from './api/client'
import { AddSymbolForm } from './components/AddSymbolForm'
import { Header } from './components/Header'
import { NewsPanel } from './components/NewsPanel'
import { OwnerAccess } from './components/OwnerAccess'
import { QuoteBoard } from './components/QuoteBoard'
import { SessionBar } from './components/SessionBar'
import { Toasts } from './components/Toasts'
import { EmptyWatchlist, WatchlistLoadError } from './components/WatchlistNotice'
import { useLiveTicks } from './hooks/useLiveTicks'
import { useNews } from './hooks/useNews'
import { isUnauthorized, useOwner } from './hooks/useOwner'
import { useStockSnapshots } from './hooks/useStockSnapshots'
import { useToasts } from './hooks/useToasts'
import { useWatchlist } from './hooks/useWatchlist'
import { newsButtonId, visibleSelection } from './lib/selection'

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
  const { ticks, sparks, connection, ticksPerSecond } = useLiveTicks(symbols, push)

  // Berita baru diambil setelah semua quote selesai (berhasil atau gagal), supaya nggak
  // berebut jatah Finnhub dengan harga di papan.
  const ready =
    symbols.length > 0 && symbols.every((s) => snapshots[s]?.quote || snapshots[s]?.error)
  const { news, load } = useNews(symbols, ready)

  const [selected, setSelected] = useState<string | null>(null)
  // Simbol yang dihapus dari watchlist otomatis menutup panelnya.
  const open = visibleSelection(selected, symbols)
  const handleSelect = useCallback(
    (symbol: string) => setSelected((prev) => (prev === symbol ? null : symbol)),
    [],
  )
  const handleClose = useCallback(() => {
    setSelected(null)
    // Kembalikan fokus ke baris yang tadi dibuka.
    if (open) document.getElementById(newsButtonId(open))?.focus()
  }, [open])

  // Panel yang baru dibuka langsung mengambil beritanya, tanpa menunggu antrean.
  // Ref: effect ini cuma boleh jalan saat panel berganti, bukan tiap berita lain masuk.
  const newsRef = useRef(news)
  useEffect(() => {
    newsRef.current = news
  })
  useEffect(() => {
    const status = open ? newsRef.current[open]?.status : undefined
    if (open && status !== 'ok' && status !== 'loading') load(open)
  }, [open, load])

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
        sparks={sparks}
        onRemove={canEdit ? handleRemove : undefined}
        news={news}
        selected={open}
        onSelect={handleSelect}
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
        <div className={open ? 'workspace open' : 'workspace'}>
          {content}
          {open && (
            <NewsPanel
              symbol={open}
              name={snapshots[open]?.name}
              price={ticks[open]?.price ?? snapshots[open]?.quote?.price ?? null}
              prevClose={snapshots[open]?.quote?.prevClose || null}
              state={news[open]}
              onClose={handleClose}
              onRetry={() => load(open)}
            />
          )}
        </div>
        <p className="footnote">
          Data dari Finnhub. Persentase dihitung dari harga penutupan kemarin.
        </p>
        <OwnerAccess owner={owner} />
      </main>
      <Toasts toasts={toasts} onDismiss={dismiss} />
    </>
  )
}
