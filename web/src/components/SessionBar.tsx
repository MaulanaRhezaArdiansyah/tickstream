import { useMarketStatus } from '../hooks/useMarketStatus'
import { useNow } from '../hooks/useNow'
import { fmtDuration, SEGMENTS, usSession, type Phase, type Session } from '../lib/session'

const SEGMENT_LABELS = [
  { phase: 'pre', label: 'Pra-pasar' },
  { phase: 'regular', label: 'Reguler' },
  { phase: 'post', label: 'Pasca' },
] as const satisfies readonly { phase: Phase; label: string }[]

/**
 * Sesi bursa US hari ini dalam waktu Jakarta, dengan penanda jam sekarang.
 * Komponen sendiri karena ia render ulang tiap detik (lewat `useNow`).
 */
export function SessionBar() {
  const now = useNow()
  const status = useMarketStatus()
  const session = usSession(now)
  // Jadwal lokal nggak tahu hari libur bursa; server tahu.
  const holiday = session.phase === 'regular' && status?.isOpen === false
  const [title, detail] = holiday
    ? ['Bursa US libur hari ini', ', kripto tetap jalan 24 jam']
    : describe(session)
  const active = holiday ? null : session.phase

  return (
    <section className="session page" aria-label="Sesi bursa US dalam waktu Jakarta">
      <div className="session-head">
        <p>
          <b>{title}</b>
          <span className="label">{detail}</span>
        </p>
        <p className="label session-note">Jam bursa New York dalam waktu Jakarta</p>
      </div>
      <div className="session-bar" aria-hidden="true">
        {SEGMENT_LABELS.map(({ phase, label }, i) => (
          <div
            key={phase}
            className={phase === active ? 'session-seg on' : 'session-seg'}
            style={{ flexGrow: SEGMENTS[i] }}
          >
            <span className="session-track" />
            <span className="session-label">
              <span>
                <span className="word">{label} </span>
                {session.wib[i]}
              </span>
              {i === SEGMENT_LABELS.length - 1 && <span>{session.wib[3]}</span>}
            </span>
          </div>
        ))}
        {session.position != null && !holiday && (
          <span className="session-now" style={{ left: `${session.position * 100}%` }} />
        )}
      </div>
    </section>
  )
}

function describe({ phase, wib, minutesToNext }: Session): [string, string] {
  const left = fmtDuration(minutesToNext)
  switch (phase) {
    case 'pre':
      return ['Pra-pasar', `, sesi reguler buka ${wib[1]} WIB (${left} lagi)`]
    case 'regular':
      return ['Sesi reguler', `, tutup ${wib[2]} WIB (${left} lagi)`]
    case 'post':
      return ['Pasca-pasar', `, selesai ${wib[3]} WIB (${left} lagi)`]
    case 'closed':
      return ['Bursa US tutup', `, pra-pasar buka ${wib[0]} WIB (${left} lagi)`]
  }
}
