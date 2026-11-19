// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'

import type { Snapshot } from '../hooks/useStockSnapshots'
import type { Quote } from '../types'
import { QuoteRow } from './QuoteRow'

const QUOTE: Quote = { price: 110, open: 100, high: 112, low: 99, prevClose: 100, timestamp: 0 }

function renderRow(symbol: string, snapshot: Snapshot | undefined, onRemove?: (s: string) => void) {
  return render(
    <ul>
      <QuoteRow symbol={symbol} snapshot={snapshot} live={undefined} onRemove={onRemove} />
    </ul>,
  )
}

// Tanpa test globals, Testing Library nggak membersihkan DOM sendiri di antara tes.
afterEach(cleanup)

it('pengunjung nggak melihat tombol hapus', () => {
  renderRow('AAPL', { quote: QUOTE, name: 'Apple Inc.' })
  expect(screen.queryByRole('button', { name: 'Hapus AAPL dari watchlist' })).toBeNull()
})

it('pemilik bisa menghapus simbol', () => {
  const onRemove = vi.fn()
  renderRow('AAPL', { quote: QUOTE, name: 'Apple Inc.' }, onRemove)

  fireEvent.click(screen.getByRole('button', { name: 'Hapus AAPL dari watchlist' }))
  expect(onRemove).toHaveBeenCalledWith('AAPL')
})

it('harga dan perubahan dari penutupan kemarin', () => {
  renderRow('AAPL', { quote: QUOTE, name: 'Apple Inc.' })
  expect(screen.getByText('110.00')).not.toBeNull()
  expect(screen.getByText(/\+10\.00%/)).not.toBeNull()
})

it('logo yang gagal dimuat diganti huruf pertama ticker', () => {
  const { container } = renderRow('AAPL', { quote: QUOTE, logo: 'https://example.com/a.png' })
  const img = container.querySelector('img.logo')
  expect(img).not.toBeNull()

  fireEvent.error(img!)
  expect(container.querySelector('img.logo')).toBeNull()
  expect(container.querySelector('.logo-letter')?.textContent).toBe('A')
})

it('crypto tanpa logo menampilkan huruf dan nama bursa', () => {
  const { container } = renderRow('BINANCE:BTCUSDT', { quote: QUOTE })
  expect(container.querySelector('.logo-letter')?.textContent).toBe('B')
  expect(screen.getByText('BTCUSDT')).not.toBeNull()
  expect(screen.getByText('Binance')).not.toBeNull()
})

it('error ditampilkan kalau harganya nggak ada', () => {
  renderRow('XYZ', { error: 'simbol XYZ tidak ditemukan' })
  expect(screen.getByText('simbol XYZ tidak ditemukan')).not.toBeNull()
})
