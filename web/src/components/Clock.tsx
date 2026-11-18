import { useNow } from '../hooks/useNow'

const nyTime = new Intl.DateTimeFormat('en-US', {
  timeZone: 'America/New_York',
  hour: '2-digit',
  minute: '2-digit',
})
const wibTime = new Intl.DateTimeFormat('id-ID', {
  timeZone: 'Asia/Jakarta',
  hour: '2-digit',
  minute: '2-digit',
})

/**
 * Jam NY dan WIB. Sengaja jadi komponen sendiri: state yang berubah tiap detik cuma
 * me-render ulang komponen kecil ini, bukan seluruh dashboard.
 */
export function Clock() {
  const now = useNow()
  return (
    <span>
      NY {nyTime.format(now)} · WIB {wibTime.format(now)}
    </span>
  )
}
