import {
  useId,
  useMemo,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type MouseEvent,
} from 'react'

import { errorMessage } from '../api/client'
import { useSymbolDirectory, type SymbolDirectory } from '../hooks/useSymbolDirectory'
import { searchSymbols, type SymbolMatches } from '../lib/symbolSearch'
import type { SymbolInfo } from '../types'

/** Saran yang dirender sekaligus. Sisanya disaring dengan mengetik. */
const LIMIT = 50

interface AddSymbolFormProps {
  onAdd: (symbol: string) => Promise<void>
  onError: (message: string) => void
  /** Simbol yang sudah ada di watchlist; ditandai di daftar saran. */
  watched: readonly string[]
}

/**
 * Input tambah simbol dengan daftar saran (pola ARIA "combobox"). Fokus tetap di input;
 * opsi yang aktif ditunjuk lewat `aria-activedescendant`, jadi screen reader membacakannya
 * tanpa fokus pindah. Enter menambah opsi aktif, atau teks apa adanya kalau nggak ada saran
 * (mis. simbol kripto `BINANCE:ETHUSDT`).
 */
export function AddSymbolForm({ onAdd, onError, watched }: AddSymbolFormProps) {
  const [value, setValue] = useState('')
  const [busy, setBusy] = useState(false)
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const { directory, load } = useSymbolDirectory()
  const inputId = useId()
  const listId = useId()

  const symbols = directory.status === 'ready' ? directory.symbols : null
  // Menyaring 11 ribu baris butuh ±2 ms; useMemo supaya nggak diulang di render yang
  // bukan karena ketikan (mis. hover mengganti opsi aktif).
  const matches = useMemo(
    () => (symbols ? searchSymbols(symbols, value, LIMIT) : null),
    [symbols, value],
  )
  const watchedSet = useMemo(() => new Set(watched), [watched])

  const options = matches?.items ?? []
  const activeOption = open ? options[active] : undefined
  const optionId = (index: number) => `${listId}-${index}`

  /** Pindah opsi aktif lewat keyboard, dan scroll daftar supaya opsinya kelihatan. */
  function moveTo(index: number) {
    if (options.length === 0) return
    const next = Math.max(0, Math.min(index, options.length - 1))
    setActive(next)
    document.getElementById(optionId(next))?.scrollIntoView({ block: 'nearest' })
  }

  async function add(symbol: string) {
    if (!symbol || busy) return
    setBusy(true)
    try {
      await onAdd(symbol)
      setValue('')
      setOpen(false)
    } catch (err) {
      onError(errorMessage(err))
    } finally {
      setBusy(false)
    }
  }

  function choose(info: SymbolInfo) {
    if (!watchedSet.has(info.symbol)) void add(info.symbol)
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault()
    if (activeOption) choose(activeOption)
    else void add(value.trim())
  }

  function handleKeyDown(event: KeyboardEvent) {
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        if (!open) setOpen(true)
        else moveTo(active + 1)
        break
      case 'ArrowUp':
        event.preventDefault()
        moveTo(active - 1)
        break
      case 'Escape':
        // Escape ini milik kolom ini; panel berita nggak ikut tertutup.
        event.preventDefault()
        if (open) setOpen(false)
        else setValue('')
        break
    }
  }

  const expanded = open && directory.status !== 'idle'

  return (
    <form className="add-form" onSubmit={handleSubmit}>
      <label className="label" htmlFor={inputId}>
        Tambah simbol
      </label>
      <div className="combo">
        <input
          id={inputId}
          className="field"
          placeholder="AMD atau Palantir"
          maxLength={40}
          autoComplete="off"
          spellCheck={false}
          role="combobox"
          aria-expanded={expanded}
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={activeOption ? optionId(active) : undefined}
          value={value}
          onChange={(event) => {
            setValue(event.target.value)
            setActive(0)
            setOpen(true)
          }}
          onFocus={() => {
            load()
            setOpen(true)
          }}
          onBlur={() => setOpen(false)}
          onKeyDown={handleKeyDown}
          // readOnly, bukan disabled: input disabled kehilangan fokus, padahal habis
          // menambah satu simbol biasanya user langsung mengetik simbol berikutnya.
          readOnly={busy}
        />
        <div className="combo-pop" hidden={!expanded}>
          <ul id={listId} role="listbox" aria-label="Saran simbol" className="combo-list">
            {options.map((info, index) => (
              <SymbolOption
                key={info.symbol}
                id={optionId(index)}
                info={info}
                active={index === active}
                watched={watchedSet.has(info.symbol)}
                // mousedown, bukan click: kalau nunggu click, input sudah blur dan daftar
                // keburu tertutup. preventDefault menjaga fokus tetap di input.
                onMouseDown={(event) => {
                  event.preventDefault()
                  choose(info)
                }}
                onMouseEnter={() => setActive(index)}
              />
            ))}
          </ul>
          <ComboNote text={note(directory, matches, value)} />
        </div>
      </div>
      <button className="btn primary" type="submit" disabled={busy || !value.trim()}>
        {busy ? 'Menambah…' : 'Tambah'}
      </button>
    </form>
  )
}

interface SymbolOptionProps {
  id: string
  info: SymbolInfo
  active: boolean
  watched: boolean
  onMouseDown: (event: MouseEvent) => void
  onMouseEnter: () => void
}

function SymbolOption({ id, info, active, watched, onMouseDown, onMouseEnter }: SymbolOptionProps) {
  return (
    <li
      id={id}
      role="option"
      aria-selected={active}
      aria-disabled={watched || undefined}
      className="combo-opt"
      onMouseDown={onMouseDown}
      onMouseEnter={onMouseEnter}
    >
      <span className="combo-symbol">{info.symbol}</span>
      <span className="combo-name">{info.name}</span>
      {watched ? (
        <span className="combo-tag">di watchlist</span>
      ) : (
        info.kind === 'etf' && <span className="combo-tag">ETF</span>
      )}
    </li>
  )
}

function ComboNote({ text }: { text: string }) {
  return text ? <p className="combo-note">{text}</p> : null
}

const count = new Intl.NumberFormat('id-ID')

/** Baris keterangan di bawah daftar: status unduhan, atau berapa yang nggak ditampilkan. */
function note(directory: SymbolDirectory, matches: SymbolMatches | null, value: string): string {
  if (directory.status === 'loading') return 'Memuat daftar saham…'
  if (directory.status === 'error') {
    return `Daftar saham nggak bisa dimuat (${directory.message}). Ketik ticker lalu Enter.`
  }
  if (!matches) return ''
  const query = value.trim()
  if (matches.total === 0) {
    return `Nggak ada di daftar saham US. Enter untuk tetap menambah ${query.toUpperCase()}, mis. simbol kripto.`
  }
  if (!query) {
    return `${count.format(matches.total)} saham & ETF bursa US. Ketik ticker atau nama untuk menyaring.`
  }
  const hidden = matches.total - matches.items.length
  return hidden > 0
    ? `${count.format(hidden)} lainnya cocok. Ketik lebih lengkap untuk menyaring.`
    : ''
}
