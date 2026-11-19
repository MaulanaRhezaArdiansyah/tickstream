import { useCallback, useState } from 'react'

export interface Toast {
  id: number
  message: string
}

const LIFETIME_MS = 5000
/** Lebih dari ini, yang paling lama dibuang duluan supaya layar nggak penuh. */
const MAX_VISIBLE = 4

let nextId = 1

/**
 * Notifikasi sementara. `push` dan `dismiss` stabil (useCallback tanpa dependensi),
 * jadi aman dioper ke komponen `memo` atau dipakai di dependency array.
 */
export function useToasts() {
  const [toasts, setToasts] = useState<readonly Toast[]>([])

  const dismiss = useCallback((id: number) => {
    setToasts((current) => current.filter((toast) => toast.id !== id))
  }, [])

  const push = useCallback(
    (message: string) => {
      const id = nextId++
      setToasts((current) => [...current.slice(-(MAX_VISIBLE - 1)), { id, message }])
      // Timer nggak dibatalkan saat unmount: setState di komponen yang sudah hilang cuma no-op.
      setTimeout(() => dismiss(id), LIFETIME_MS)
    },
    [dismiss],
  )

  return { toasts, push, dismiss }
}
