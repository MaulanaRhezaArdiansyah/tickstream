/** Watchlist belum berisi apa pun: pemilik diarahkan ke form tambah di atasnya. */
export function EmptyWatchlist({ canEdit }: { canEdit: boolean }) {
  return (
    <section className="notice">
      <h2>Belum ada simbol</h2>
      {canEdit ? (
        <p>
          Ketik ticker saham US (AAPL, NVDA), nama perusahaan, atau pasangan kripto
          (BINANCE:BTCUSDT) di kolom Tambah simbol untuk mulai memantau harganya.
        </p>
      ) : (
        <p>Pemilik watchlist ini belum menambahkan simbol apa pun.</p>
      )}
    </section>
  )
}

interface LoadErrorProps {
  message: string
  onRetry: () => void
}

export function WatchlistLoadError({ message, onRetry }: LoadErrorProps) {
  return (
    <section className="notice" role="alert">
      <h2>Watchlist nggak bisa dimuat</h2>
      <p>Server menjawab: {message}. Pastikan server dan Postgres jalan, lalu coba lagi.</p>
      <button className="btn" type="button" onClick={onRetry}>
        Muat ulang
      </button>
    </section>
  )
}
