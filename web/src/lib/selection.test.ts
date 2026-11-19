import { expect, it } from 'vitest'

import { visibleSelection } from './selection'

it('panel tertutup kalau simbolnya dihapus', () => {
  expect(visibleSelection('AMD', ['AAPL'])).toBeNull()
  expect(visibleSelection('AMD', ['AMD'])).toBe('AMD')
  expect(visibleSelection(null, ['AMD'])).toBeNull()
})
