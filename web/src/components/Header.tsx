import { Clock } from './Clock'
import { MarketStatusLabel } from './MarketStatusLabel'

export function Header() {
  return (
    <header>
      <h1>📈 US Stock Live</h1>
      <div className="status">
        <MarketStatusLabel />
        <Clock />
      </div>
    </header>
  )
}
