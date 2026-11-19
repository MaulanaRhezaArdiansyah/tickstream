import { useState, type FormEvent } from 'react'

import { errorMessage } from '../api/client'

interface AddSymbolFormProps {
  onAdd: (symbol: string) => Promise<void>
  onError: (message: string) => void
}

export function AddSymbolForm({ onAdd, onError }: AddSymbolFormProps) {
  // Controlled input: nilai input = state React (padanan `v-model` di Vue).
  const [value, setValue] = useState('')
  const [busy, setBusy] = useState(false)

  async function handleSubmit(event: FormEvent) {
    event.preventDefault() // jangan biarkan browser reload halaman
    const symbol = value.trim()
    if (!symbol || busy) return
    setBusy(true)
    try {
      await onAdd(symbol)
      setValue('')
    } catch (err) {
      onError(errorMessage(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <form className="toolbar" onSubmit={handleSubmit}>
      <input
        className="symbol-input"
        placeholder="Ticker, mis. NVDA"
        maxLength={20}
        value={value}
        onChange={(event) => setValue(event.target.value)}
        disabled={busy}
      />
      <button className="primary" type="submit" disabled={busy || !value.trim()}>
        {busy ? 'Menambah…' : '+ Tambah'}
      </button>
    </form>
  )
}
