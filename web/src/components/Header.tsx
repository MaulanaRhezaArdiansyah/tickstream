import { memo } from 'react'

import type { ConnectionState } from '../hooks/useLiveTicks'
import { Clock } from './Clock'
import { MarketStatusLabel } from './MarketStatusLabel'

interface HeaderProps {
  connection: ConnectionState
  ticksPerSecond: number
}

const CONNECTION_LABEL: Record<ConnectionState, string> = {
  connecting: 'Menghubungkan…',
  live: 'Live',
  reconnecting: 'Menyambung ulang…',
}

/** `memo`: props-nya berubah paling sering sekali per detik, padahal App render tiap frame. */
export const Header = memo(function Header({ connection, ticksPerSecond }: HeaderProps) {
  return (
    <header className="topbar">
      <div className="page">
        <div className="brand">US Stock Live</div>
        <div className="topbar-meta">
          <MarketStatusLabel />
          <Clock />
          <span className="conn">
            {/* Kelas dot = nama state, jadi warnanya diatur penuh dari CSS. */}
            <span className={`dot ${connection}`} aria-hidden="true" />
            {CONNECTION_LABEL[connection]}
            {connection === 'live' && <span className="label">{ticksPerSecond} tick/dtk</span>}
          </span>
        </div>
      </div>
    </header>
  )
})
