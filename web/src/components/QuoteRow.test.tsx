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
      <QuoteRow
        symbol={symbol}
        snapshot={snapshot}
        live={undefined}
        points={[100, 110]}
        onRemove={onRemove}
      />
    </ul>,
  )
}

// Tanpa test globals, Testing Library nggak membersihkan DOM sendiri di antara tes.
afterEach(cleanup)

// jsdom nggak punya canvas dan mencetak peringatan tiap getContext dipanggil. Sparkline
// sudah menangani context null, jadi null dikembalikan langsung tanpa peringatan.
vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)

it('pengunjung nggak melihat tombol hapus', () => {
  renderRow('AAPL', { quote: QUOTE, name: 'Apple Inc.' })
  // Baris ini nggak punya tombol lain, jadi tombol apa pun = tombol hapus bocor ke pengunjung.
  expect(screen.queryByRole('button')).toBeNull()
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

it('badge berita', () => {
  const { rerender } = render(
    <ul>
      <QuoteRow
        symbol="AAPL"
        snapshot={undefined}
        live={undefined}
        points={undefined}
        newsCount={6}
      />
    </ul>,
  )
  expect(screen.getByText('6 berita')).not.toBeNull()

  rerender(
    <ul>
      <QuoteRow
        symbol="AAPL"
        snapshot={undefined}
        live={undefined}
        points={undefined}
        newsCount={0}
      />
    </ul>,
  )
  expect(screen.queryByText(/berita/)).toBeNull()
})

it('klik baris memilih simbol', () => {
  const onSelect = vi.fn()
  render(
    <ul>
      <QuoteRow
        symbol="AAPL"
        snapshot={undefined}
        live={undefined}
        points={undefined}
        onSelect={onSelect}
      />
    </ul>,
  )
  fireEvent.click(screen.getByRole('button', { name: 'Berita AAPL' }))
  expect(onSelect).toHaveBeenCalledTimes(1)
  expect(onSelect).toHaveBeenCalledWith('AAPL')
})

it('hapus nggak ikut memilih', () => {
  const onSelect = vi.fn()
  const onRemove = vi.fn()
  render(
    <ul>
      <QuoteRow
        symbol="AAPL"
        snapshot={undefined}
        live={undefined}
        points={undefined}
        onSelect={onSelect}
        onRemove={onRemove}
      />
    </ul>,
  )
  fireEvent.click(screen.getByRole('button', { name: 'Hapus AAPL dari watchlist' }))
  expect(onRemove).toHaveBeenCalledWith('AAPL')
  expect(onSelect).not.toHaveBeenCalled()
})
