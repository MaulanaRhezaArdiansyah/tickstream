import type { Toast } from '../hooks/useToasts'
import { XIcon } from './XIcon'

interface ToastsProps {
  toasts: readonly Toast[]
  onDismiss: (id: number) => void
}

export function Toasts({ toasts, onDismiss }: ToastsProps) {
  return (
    // aria-live: screen reader membacakan toast yang muncul tanpa memindah fokus.
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div key={toast.id} className="toast">
          <span>{toast.message}</span>
          <button
            className="icon-btn"
            type="button"
            aria-label="Tutup notifikasi"
            onClick={() => onDismiss(toast.id)}
          >
            <XIcon size={12} />
          </button>
        </div>
      ))}
    </div>
  )
}
