// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'

import type { NewsState } from '../hooks/useNews'
import { NewsPanel } from './NewsPanel'

afterEach(cleanup)

const ok = (headline: string): NewsState => ({
  status: 'ok',
  news: {
    count24h: 1,
    items: [
      {
        id: 1,
        datetime: Date.now() - 2 * 3_600_000,
        headline,
        source: 'Reuters',
        summary: 'Ringkasan',
        url: 'https://example.com/a',
      },
    ],
  },
})

function renderPanel(state: NewsState | undefined, symbol = 'AMD') {
  const onClose = vi.fn()
  const onRetry = vi.fn()
  const view = render(
    <NewsPanel
      symbol={symbol}
      name="Advanced Micro Devices"
      price={100}
      prevClose={105}
      state={state}
      onClose={onClose}
      onRetry={onRetry}
    />,
  )
  return { ...view, onClose, onRetry }
}

it('memuat', () => {
  renderPanel({ status: 'loading' })
  expect(screen.getByText('Memuat berita…')).not.toBeNull()
})

it('daftar berita', () => {
  renderPanel(ok('AMD turun'))
  const link = screen.getByRole('link', { name: 'AMD turun' })
  expect(link.getAttribute('href')).toBe('https://example.com/a')
  expect(link.getAttribute('target')).toBe('_blank')
  expect(link.getAttribute('rel')).toBe('noopener noreferrer')
  expect(screen.getByText(/Reuters/)).not.toBeNull()
  expect(screen.getByText(/2 jam lalu/)).not.toBeNull()
})

it('kosong', () => {
  renderPanel({ status: 'ok', news: { count24h: 0, items: [] } })
  expect(screen.getByText('Belum ada berita 7 hari terakhir')).not.toBeNull()
})

it('gagal', () => {
  const { onRetry } = renderPanel({ status: 'error', message: 'Kuota habis' })
  expect(screen.getByText(/Kuota habis/)).not.toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Coba lagi' }))
  expect(onRetry).toHaveBeenCalledTimes(1)
})

it('tutup', () => {
  const { onClose } = renderPanel(ok('x'))
  fireEvent.click(screen.getByRole('button', { name: 'Tutup berita' }))
  expect(onClose).toHaveBeenCalledTimes(1)
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(onClose).toHaveBeenCalledTimes(2)
})

it('Escape yang sudah dipakai elemen lain nggak menutup panel', () => {
  const { onClose } = renderPanel(ok('x'))
  // Mis. kolom "Tambah simbol" menutup daftar sarannya dengan Escape.
  const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
  event.preventDefault()
  document.dispatchEvent(event)
  expect(onClose).not.toHaveBeenCalled()
})

function mockNarrowScreen(narrow: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({ matches: narrow })),
  )
}

it('di layar penuh (HP) Tab berputar di dalam panel', () => {
  mockNarrowScreen(true)
  renderPanel(ok('Judul'))
  const close = screen.getByRole('button', { name: 'Tutup berita' })
  const link = screen.getByRole('link', { name: 'Judul' })

  link.focus()
  fireEvent.keyDown(link, { key: 'Tab' })
  expect(document.activeElement).toBe(close)

  fireEvent.keyDown(close, { key: 'Tab', shiftKey: true })
  expect(document.activeElement).toBe(link)
  vi.unstubAllGlobals()
})

it('di desktop Tab nggak dibelokkan', () => {
  mockNarrowScreen(false)
  renderPanel(ok('Judul'))
  const link = screen.getByRole('link', { name: 'Judul' })

  link.focus()
  fireEvent.keyDown(link, { key: 'Tab' })
  expect(document.activeElement).toBe(link)
  vi.unstubAllGlobals()
})

it('panel menampilkan berita simbolnya sendiri', () => {
  const { rerender } = renderPanel(ok('AMD x'))
  expect(screen.getByText('AMD x')).not.toBeNull()
  rerender(
    <NewsPanel
      symbol="NVDA"
      name="NVIDIA"
      price={100}
      prevClose={99}
      state={{ status: 'loading' }}
      onClose={() => {}}
      onRetry={() => {}}
    />,
  )
  expect(screen.queryByText('AMD x')).toBeNull()
  expect(screen.getByRole('complementary', { name: 'Berita NVDA' })).not.toBeNull()
})
