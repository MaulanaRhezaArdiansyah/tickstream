// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'

import { ApiError, fetchProfile, fetchQuote } from '../api/client'
import type { Profile, Quote } from '../types'
import { useStockSnapshots } from './useStockSnapshots'

vi.mock('../api/client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api/client')>()),
  fetchQuote: vi.fn(),
  fetchProfile: vi.fn(),
}))

afterEach(cleanup)

const QUOTE: Quote = { price: 1, open: 1, high: 1, low: 1, prevClose: 1, timestamp: 0 }

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (err: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

const SYMBOLS = ['AAPL'] as const

it('baru dianggap selesai setelah profil dicoba, berhasil atau gagal', async () => {
  const profile = deferred<Profile>()
  vi.mocked(fetchQuote).mockResolvedValue(QUOTE)
  vi.mocked(fetchProfile).mockReturnValue(profile.promise)

  const { result } = renderHook(() => useStockSnapshots(SYMBOLS))
  await act(async () => {})
  expect(result.current.AAPL?.quote).toEqual(QUOTE)
  expect(result.current.AAPL?.settled).toBeFalsy()

  await act(async () => profile.reject(new ApiError(429, 'Kuota habis')))
  expect(result.current.AAPL?.settled).toBe(true)
})
