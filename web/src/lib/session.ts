/**
 * Jadwal sesi bursa saham US (NYSE/Nasdaq) dan konversinya ke WIB.
 *
 * Jam bursa selalu tetap dalam waktu New York. Yang berubah saat musim panas US (DST)
 * adalah selisihnya dengan WIB: 12 jam di musim dingin, 11 jam di musim panas. Offset
 * New York dibaca dari `Intl`, jadi aturan DST nggak perlu ditulis sendiri.
 *
 * Hari libur bursa nggak diketahui di sini; itu urusan /api/market-status.
 */

export type Phase = 'pre' | 'regular' | 'post' | 'closed'

// Menit sejak tengah malam, waktu New York.
const PRE_OPEN = 4 * 60
const REGULAR_OPEN = 9 * 60 + 30
const REGULAR_CLOSE = 16 * 60
const POST_CLOSE = 20 * 60
const DAY = 24 * 60
const WIB_OFFSET = 7 * 60

/** Panjang segmen bar dalam menit: pra-pasar, reguler, pasca-pasar. */
export const SEGMENTS = [
  REGULAR_OPEN - PRE_OPEN,
  REGULAR_CLOSE - REGULAR_OPEN,
  POST_CLOSE - REGULAR_CLOSE,
] as const

export interface Session {
  phase: Phase
  /** Posisi jam sekarang di bar (0..1); `null` di luar 04:00–20:00 New York. */
  position: number | null
  /** Menit sampai fase berikutnya mulai. Saat tutup: sampai pra-pasar berikutnya. */
  minutesToNext: number
  /** Jam WIB tiap batas bar: pra-pasar, reguler buka, reguler tutup, pasca-pasar selesai. */
  wib: readonly [string, string, string, string]
}

const nyFormat = new Intl.DateTimeFormat('en-US', {
  timeZone: 'America/New_York',
  weekday: 'short',
  hour: '2-digit',
  minute: '2-digit',
  hourCycle: 'h23',
  // "GMT-5" di musim dingin, "GMT-4" di musim panas.
  timeZoneName: 'shortOffset',
})
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']

export function usSession(now: Date): Session {
  const parts: Record<string, string> = {}
  for (const part of nyFormat.formatToParts(now)) parts[part.type] = part.value

  const weekday = WEEKDAYS.indexOf(parts.weekday)
  const minute = Number(parts.hour) * 60 + Number(parts.minute)
  const offset = Number(/GMT([+-]\d+)/.exec(parts.timeZoneName)?.[1] ?? -5) * 60

  // Label pakai offset HARI INI. Hitung mundur yang melewati pergantian DST bisa meleset
  // satu jam, dan itu cuma terjadi dua akhir pekan setahun.
  const toWib = (nyMinute: number) => {
    const m = (((nyMinute - offset + WIB_OFFSET) % DAY) + DAY) % DAY
    return `${pad(Math.floor(m / 60))}:${pad(m % 60)}`
  }
  const wib = [
    toWib(PRE_OPEN),
    toWib(REGULAR_OPEN),
    toWib(REGULAR_CLOSE),
    toWib(POST_CLOSE),
  ] as const

  const isTradingDay = (day: number) => day >= 1 && day <= 5

  if (isTradingDay(weekday) && minute >= PRE_OPEN && minute < POST_CLOSE) {
    let phase: Phase
    let next: number
    if (minute < REGULAR_OPEN) [phase, next] = ['pre', REGULAR_OPEN]
    else if (minute < REGULAR_CLOSE) [phase, next] = ['regular', REGULAR_CLOSE]
    else [phase, next] = ['post', POST_CLOSE]
    return {
      phase,
      position: (minute - PRE_OPEN) / (POST_CLOSE - PRE_OPEN),
      minutesToNext: next - minute,
      wib,
    }
  }

  // Tutup. Pra-pasar berikutnya hari ini (kalau masih dini hari di hari bursa) atau di hari
  // bursa berikutnya: Jumat malam → Senin.
  let days = isTradingDay(weekday) && minute < PRE_OPEN ? 0 : 1
  while (!isTradingDay((weekday + days) % 7)) days++
  return { phase: 'closed', position: null, minutesToNext: days * DAY + PRE_OPEN - minute, wib }
}

/** 260 → "4 j 20 m"; 3000 → "2 hari 2 j". */
export function fmtDuration(minutes: number): string {
  const days = Math.floor(minutes / DAY)
  const hours = Math.floor((minutes % DAY) / 60)
  const mins = minutes % 60
  if (days > 0) return `${days} hari ${hours} j`
  if (hours > 0) return `${hours} j ${mins} m`
  return `${mins} m`
}

function pad(n: number): string {
  return String(n).padStart(2, '0')
}
