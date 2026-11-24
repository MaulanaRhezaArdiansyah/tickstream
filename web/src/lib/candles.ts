import type { CandleRow } from '../types'

export interface Bar {
  time: number
  open: number
  high: number
  low: number
  close: number
}

export function toBars(rows: readonly CandleRow[]): Bar[] {
  return rows.map(([time, open, high, low, close]) => ({ time, open, high, low, close }))
}

/** Sama dengan batas di server (`MAX_FUTURE_MS`). */
const MAX_FUTURE_MS = 60_000

/**
 * `null` = tick nggak boleh digambar: harga/timestamp rusak, terlalu jauh di masa depan, atau
 * lebih lama dari bar terakhir (`update()` chart bakal throw kalau dikasih itu).
 */
export function applyTick(
  last: Bar | undefined,
  price: number,
  timestampMs: number,
  bucketSeconds: number,
  nowMs: number = Date.now(),
): Bar | null {
  if (!Number.isFinite(price) || price <= 0) return null
  if (!Number.isFinite(timestampMs) || timestampMs > nowMs + MAX_FUTURE_MS) return null

  const time = Math.floor(timestampMs / 1000 / bucketSeconds) * bucketSeconds
  if (!last || time > last.time) {
    return { time, open: price, high: price, low: price, close: price }
  }
  if (time < last.time) {
    return null
  }
  return {
    time: last.time,
    open: last.open,
    high: Math.max(last.high, price),
    low: Math.min(last.low, price),
    close: price,
  }
}

const wibTimeFormatter = new Intl.DateTimeFormat('en-GB', {
  timeZone: 'Asia/Jakarta',
  hour: '2-digit',
  minute: '2-digit',
  hourCycle: 'h23',
})

const wibDayFormatter = new Intl.DateTimeFormat('id-ID', {
  timeZone: 'Asia/Jakarta',
  day: 'numeric',
  month: 'short',
})

export function formatWibTime(unixSeconds: number): string {
  return wibTimeFormatter.format(unixSeconds * 1000)
}

export function formatWibDay(unixSeconds: number): string {
  return wibDayFormatter.format(unixSeconds * 1000)
}
