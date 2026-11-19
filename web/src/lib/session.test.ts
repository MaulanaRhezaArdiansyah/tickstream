import { describe, expect, it } from 'vitest'

import { fmtDuration, usSession } from './session'

// Semua waktu ditulis dalam UTC, jadi hasilnya sama di mesin mana pun.
// November 2026: New York UTC-5. Juli 2026: New York UTC-4 (musim panas).
describe('usSession', () => {
  it.each([
    ['2026-11-19T09:00:00Z', 'Kamis 04:00, pra-pasar baru buka', 'pre', 330],
    ['2026-11-19T14:30:00Z', 'Kamis 09:30, reguler baru buka', 'regular', 390],
    ['2026-11-19T14:44:00Z', 'Kamis 09:44', 'regular', 376],
    ['2026-11-19T21:00:00Z', 'Kamis 16:00, pasca-pasar', 'post', 240],
    ['2026-11-20T01:00:00Z', 'Kamis 20:00, tutup sampai Jumat pagi', 'closed', 480],
    ['2026-11-21T01:00:00Z', 'Jumat 20:00, tutup sampai Senin', 'closed', 3360],
    ['2026-11-22T15:00:00Z', 'Minggu 10:00', 'closed', 1080],
    ['2026-11-23T08:00:00Z', 'Senin 03:00, sejam sebelum pra-pasar', 'closed', 60],
  ])('%s (%s): %s, %i menit lagi', (utc, _label, phase, minutesToNext) => {
    const session = usSession(new Date(utc))
    expect(session.phase).toBe(phase)
    expect(session.minutesToNext).toBe(minutesToNext)
  })

  it('posisi di bar: 0 saat pra-pasar buka, null saat tutup', () => {
    expect(usSession(new Date('2026-11-19T09:00:00Z')).position).toBe(0)
    expect(usSession(new Date('2026-11-19T14:44:00Z')).position).toBeCloseTo(344 / 960)
    expect(usSession(new Date('2026-11-20T01:00:00Z')).position).toBeNull()
  })

  it('jam WIB bergeser sejam saat musim panas US', () => {
    expect(usSession(new Date('2026-11-19T14:44:00Z')).wib).toEqual([
      '16:00',
      '21:30',
      '04:00',
      '08:00',
    ])

    const summer = usSession(new Date('2026-07-15T13:30:00Z'))
    expect(summer.phase).toBe('regular')
    expect(summer.wib).toEqual(['15:00', '20:30', '03:00', '07:00'])
  })
})

it('fmtDuration', () => {
  expect(fmtDuration(45)).toBe('45 m')
  expect(fmtDuration(260)).toBe('4 j 20 m')
  expect(fmtDuration(3000)).toBe('2 hari 2 j')
  expect(fmtDuration(3360)).toBe('2 hari 8 j')
})
