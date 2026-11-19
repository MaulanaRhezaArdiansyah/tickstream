import { useId, useState, type FormEvent } from 'react'

import { errorMessage } from '../api/client'
import { isUnauthorized, type Owner } from '../hooks/useOwner'

/**
 * Pintu masuk pemilik di bawah halaman. Pengunjung cukup melihat; yang punya ADMIN_TOKEN
 * server bisa membuka form tambah dan tombol hapus.
 */
export function OwnerAccess({ owner }: { owner: Owner }) {
  const [open, setOpen] = useState(false)
  const [value, setValue] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const inputId = useId()

  if (owner.token) {
    return (
      <p className="owner">
        Mode pemilik: kamu bisa menambah dan menghapus simbol.{' '}
        <button type="button" className="link-btn" onClick={owner.signOut}>
          Keluar
        </button>
      </p>
    )
  }

  if (!open) {
    return (
      <p className="owner">
        <button type="button" className="link-btn" onClick={() => setOpen(true)}>
          Masuk sebagai pemilik
        </button>
      </p>
    )
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault()
    if (!value.trim() || busy) return
    setBusy(true)
    setError(null)
    try {
      await owner.signIn(value)
      setValue('')
      setOpen(false)
    } catch (err) {
      setError(isUnauthorized(err) ? 'Token salah.' : errorMessage(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <form className="owner owner-form" onSubmit={handleSubmit}>
      <label className="label" htmlFor={inputId}>
        Token pemilik
      </label>
      <input
        id={inputId}
        className="field"
        type="password"
        autoComplete="current-password"
        autoFocus
        value={value}
        onChange={(event) => setValue(event.target.value)}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? `${inputId}-error` : undefined}
      />
      <button className="btn" type="submit" disabled={busy || !value.trim()}>
        {busy ? 'Mengecek…' : 'Masuk'}
      </button>
      <button type="button" className="link-btn" onClick={() => setOpen(false)}>
        Batal
      </button>
      {error && (
        <span id={`${inputId}-error`} className="owner-error" role="alert">
          {error}
        </span>
      )}
    </form>
  )
}
