import type { Toast } from '../hooks/useToasts'

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
            className="x"
            type="button"
            aria-label="Tutup"
            onClick={() => onDismiss(toast.id)}
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  )
}
