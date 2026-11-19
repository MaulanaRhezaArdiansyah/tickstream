import { memo } from 'react'

import type { ConnectionState } from '../hooks/useLiveTicks'
import { Clock } from './Clock'
import { MarketStatusLabel } from './MarketStatusLabel'

interface HeaderProps {
  connection: ConnectionState
  ticksPerSecond: number
}

const CONNECTION_LABEL: Record<ConnectionState, string> = {
  connecting: 'Connecting…',
  live: 'Live',
  reconnecting: 'Reconnecting…',
}

/** `memo`: props-nya berubah paling sering sekali per detik, padahal App render tiap frame. */
export const Header = memo(function Header({ connection, ticksPerSecond }: HeaderProps) {
  const dotClass = connection === 'live' ? 'on' : connection === 'reconnecting' ? 'off' : ''
  return (
    <header>
      <h1>📈 US Stock Live</h1>
      <div className="status">
        <span>
          <span className={`dot ${dotClass}`} />
          {CONNECTION_LABEL[connection]}
        </span>
        <MarketStatusLabel />
        <Clock />
        <span>{ticksPerSecond} ticks/s</span>
      </div>
    </header>
  )
})
