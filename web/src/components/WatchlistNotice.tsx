/** Watchlist belum berisi apa pun: arahkan ke form tambah di atasnya. */
export function EmptyWatchlist() {
  return (
    <section className="notice">
      <h2>Belum ada simbol</h2>
      <p>
        Ketik ticker saham US (AAPL, NVDA) atau pasangan kripto (BINANCE:BTCUSDT) di kolom Tambah
        ticker untuk mulai memantau harganya.
      </p>
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
