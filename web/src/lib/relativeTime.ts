const MINUTE = 60_000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/** "2 jam lalu", "kemarin", dst. untuk waktu berita (Unix ms) dibanding `now`. */
export function relativeTime(ms: number, now: number): string {
  const diff = now - ms
  if (diff < MINUTE) return 'baru saja'
  if (diff < HOUR) return `${Math.floor(diff / MINUTE)} menit lalu`
  if (diff < DAY) return `${Math.floor(diff / HOUR)} jam lalu`
  if (diff < 2 * DAY) return 'kemarin'
  return `${Math.floor(diff / DAY)} hari lalu`
}
