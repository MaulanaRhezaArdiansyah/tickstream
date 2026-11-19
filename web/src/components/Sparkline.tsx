import { useEffect, useRef } from 'react'

interface SparklineProps {
  points: readonly number[]
  color: string
}

/** Grafik garis kecil di `<canvas>`. Diport dari `drawSpark` di PoC. */
export function Sparkline({ points, color }: SparklineProps) {
  // Ref ke elemen DOM: React yang membuat <canvas>, kita yang menggambar di dalamnya.
  const canvasRef = useRef<HTMLCanvasElement>(null)

  // Menggambar itu efek samping ke DOM, jadi tempatnya di useEffect (jalan setelah render).
  useEffect(() => {
    const canvas = canvasRef.current
    if (canvas) draw(canvas, points, color)
  }, [points, color])

  return <canvas ref={canvasRef} />
}

function draw(canvas: HTMLCanvasElement, points: readonly number[], color: string) {
  const dpr = window.devicePixelRatio || 1
  const w = canvas.clientWidth
  const h = canvas.clientHeight
  // Resolusi internal = ukuran tampil × dpr, supaya tajam di layar HiDPI.
  if (canvas.width !== w * dpr) {
    canvas.width = w * dpr
    canvas.height = h * dpr
  }
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, w, h)
  if (points.length < 2) return

  let min = Math.min(...points)
  let max = Math.max(...points)
  if (max === min) {
    max += 0.01
    min -= 0.01
  }
  const x = (i: number) => (i / (points.length - 1)) * w
  const y = (v: number) => h - 3 - ((v - min) / (max - min)) * (h - 6)

  ctx.beginPath()
  points.forEach((p, i) => (i === 0 ? ctx.moveTo(x(i), y(p)) : ctx.lineTo(x(i), y(p))))
  ctx.strokeStyle = color
  ctx.lineWidth = 1.6
  ctx.stroke()

  ctx.lineTo(w, h)
  ctx.lineTo(0, h)
  ctx.closePath()
  const gradient = ctx.createLinearGradient(0, 0, 0, h)
  gradient.addColorStop(0, color + '55')
  gradient.addColorStop(1, color + '00')
  ctx.fillStyle = gradient
  ctx.fill()
}
