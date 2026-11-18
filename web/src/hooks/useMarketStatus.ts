import { useEffect, useState } from 'react'

import { fetchMarketStatus } from '../api/client'
import type { MarketStatus } from '../types'

const REFRESH_MS = 5 * 60 * 1000

/** Status bursa US, di-refresh tiap 5 menit. `null` selama belum ada / gagal. */
export function useMarketStatus(): MarketStatus | null {
  const [status, setStatus] = useState<MarketStatus | null>(null)

  useEffect(() => {
    const controller = new AbortController()
    const load = () =>
      fetchMarketStatus(controller.signal)
        .then(setStatus)
        .catch(() => {
          if (!controller.signal.aborted) setStatus(null)
        })

    void load()
    const timer = setInterval(load, REFRESH_MS)
    return () => {
      controller.abort()
      clearInterval(timer)
    }
  }, [])

  return status
}
