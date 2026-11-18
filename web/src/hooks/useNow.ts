import { useEffect, useState } from 'react'

/** Waktu sekarang, diperbarui tiap detik. */
export function useNow(): Date {
  // Lazy initializer: `new Date()` cuma dipanggil sekali saat mount, bukan tiap render.
  const [now, setNow] = useState(() => new Date())

  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 1000)
    return () => clearInterval(timer)
  }, [])

  return now
}
