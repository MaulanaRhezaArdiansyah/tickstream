import { useNow } from '../hooks/useNow'

// en-GB: format 24 jam "23:40". id-ID memakai titik ("23.40"), kurang lazim untuk jam bursa.
const nyTime = new Intl.DateTimeFormat('en-GB', {
  timeZone: 'America/New_York',
  hour: '2-digit',
  minute: '2-digit',
})
const wibTime = new Intl.DateTimeFormat('en-GB', {
  timeZone: 'Asia/Jakarta',
  hour: '2-digit',
  minute: '2-digit',
})

/**
 * Jam New York dan Jakarta. Sengaja jadi komponen sendiri: state yang berubah tiap detik
 * cuma me-render ulang komponen kecil ini, bukan seluruh dashboard.
 */
export function Clock() {
  const now = useNow()
  return (
    <>
      <span>
        <span className="label">New York</span> {nyTime.format(now)}
      </span>
      <span>
        <span className="label">Jakarta</span> {wibTime.format(now)}
      </span>
    </>
  )
}
