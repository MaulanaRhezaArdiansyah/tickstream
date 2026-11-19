import { expect, it } from 'vitest'

import { sparkX } from './sparkline'

it('sparkline penuh membentang dari kiri ke kanan', () => {
  expect(sparkX(0, 180, 179)).toBe(0)
  expect(sparkX(179, 180, 179)).toBe(179)
})

it('sparkline pendek menempel di kanan, skala waktunya sama dengan baris lain', () => {
  // 2 titik = 10 detik terakhir dari 30 menit, bukan direntang selebar kolom.
  expect(sparkX(1, 2, 179)).toBe(179)
  expect(sparkX(0, 2, 179)).toBe(178)
})
