import { expect, it } from 'vitest'

import { selectionAfterRemove, visibleSelection } from './selection'

it('panel tertutup kalau simbolnya dihapus', () => {
  expect(visibleSelection('AMD', ['AAPL'])).toBeNull()
  expect(visibleSelection('AMD', ['AMD'])).toBe('AMD')
  expect(visibleSelection(null, ['AMD'])).toBeNull()
})

it('simbol yang dihapus dilupakan, supaya menambahkannya lagi nggak membuka panelnya', () => {
  expect(selectionAfterRemove('AMD', 'AMD')).toBeNull()
  expect(selectionAfterRemove('AMD', 'NVDA')).toBe('AMD')
  expect(selectionAfterRemove(null, 'AMD')).toBeNull()
})
