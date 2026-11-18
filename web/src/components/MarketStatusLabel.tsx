import { useMarketStatus } from '../hooks/useMarketStatus'

export function MarketStatusLabel() {
  const status = useMarketStatus()
  if (!status) return <span>Market: –</span>

  const label = status.isOpen
    ? `Open${status.session ? ` (${status.session})` : ''}`
    : (status.session ?? 'Closed')

  return (
    <span>
      Market: <b className={status.isOpen ? 'up' : 'down'}>{label}</b>
    </span>
  )
}
