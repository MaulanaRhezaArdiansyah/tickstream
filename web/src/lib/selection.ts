/** Simbol yang panelnya terbuka, atau null kalau simbol itu sudah dihapus dari watchlist. */
export function visibleSelection(
  selected: string | null,
  symbols: readonly string[],
): string | null {
  return selected !== null && symbols.includes(selected) ? selected : null
}

/** `id` tombol simbol di baris, supaya fokus bisa dikembalikan waktu panel ditutup. */
export const newsButtonId = (symbol: string) => `news-btn-${symbol}`

/** Pilihan setelah `removed` dihapus dari watchlist; simbol itu dilupakan, bukan cuma disembunyikan. */
export function selectionAfterRemove(selected: string | null, removed: string): string | null {
  return selected === removed ? null : selected
}
