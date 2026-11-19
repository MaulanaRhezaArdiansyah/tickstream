import { expect, it } from 'vitest'

import { relativeTime } from './relativeTime'

const now = 1_000_000_000
const MIN = 60_000
const HOUR = 60 * MIN

it('waktu relatif berita', () => {
  expect(relativeTime(now - 30_000, now)).toBe('baru saja')
  expect(relativeTime(now - 5 * MIN, now)).toBe('5 menit lalu')
  expect(relativeTime(now - 2 * HOUR, now)).toBe('2 jam lalu')
  expect(relativeTime(now - 30 * HOUR, now)).toBe('kemarin')
  expect(relativeTime(now - 3 * 24 * HOUR, now)).toBe('3 hari lalu')
})

it('jam server yang sedikit di depan tetap "baru saja"', () => {
  expect(relativeTime(now + 5_000, now)).toBe('baru saja')
})
