import {
  CandlestickSeries,
  ColorType,
  createChart,
  TickMarkType,
  type CandlestickData,
  type IChartApi,
  type ISeriesApi,
  type Time,
  type UTCTimestamp,
} from 'lightweight-charts'
import { useEffect, useRef, useState } from 'react'

import { useCandles } from '../hooks/useCandles'
import type { ConnectionState } from '../hooks/useLiveTicks'
import { applyTick, formatWibDay, formatWibTime, toBars, type Bar } from '../lib/candles'
import type { LiveTick } from '../lib/tickStore'
import type { CandleRange } from '../types'

interface IntradayChartProps {
  symbol: string
  live: LiveTick | undefined
  connection: ConnectionState
}

/**
 * Grafik candlestick intraday di panel samping.
 *
 * Instance chart dibuat sekali seumur panel: ganti simbol atau rentang cuma mengganti
 * datanya lewat `setData`, dan tick live memperbarui candle terakhir lewat `update`.
 */
export default function IntradayChart({ symbol, live, connection }: IntradayChartProps) {
  const [range, setRange] = useState<CandleRange>('1d')
  const { state, retry } = useCandles(symbol, range, connection)

  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const seriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null)
  const lastBarRef = useRef<Bar | undefined>(undefined)
  // `fitContent` cuma dipanggil saat simbol/range berubah, supaya refresh reconnect nggak
  // mengacak posisi zoom yang sedang digeser user.
  const fittedKeyRef = useRef<string | null>(null)

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const chart = createChart(container, {
      autoSize: true,
      layout: {
        background: { type: ColorType.Solid, color: 'transparent' },
        textColor: '#9ba5a0',
      },
      grid: {
        vertLines: { color: '#222e2b' },
        horzLines: { color: '#222e2b' },
      },
      rightPriceScale: {
        borderColor: '#2d3a36',
      },
      timeScale: {
        timeVisible: true,
        secondsVisible: false,
        borderColor: '#2d3a36',
        tickMarkFormatter: (time: Time, tickMarkType: TickMarkType) => {
          if (typeof time !== 'number') return ''
          return tickMarkType === TickMarkType.Time || tickMarkType === TickMarkType.TimeWithSeconds
            ? formatWibTime(time)
            : formatWibDay(time)
        },
      },
      localization: {
        timeFormatter: (time: Time) =>
          typeof time === 'number' ? `${formatWibDay(time)} ${formatWibTime(time)}` : '',
      },
    })

    const series = chart.addSeries(CandlestickSeries, {
      upColor: '#52c48c',
      downColor: '#f2705f',
      borderUpColor: '#52c48c',
      borderDownColor: '#f2705f',
      wickUpColor: '#52c48c',
      wickDownColor: '#f2705f',
    })

    chartRef.current = chart
    seriesRef.current = series

    return () => {
      chart.remove()
      chartRef.current = null
      seriesRef.current = null
      lastBarRef.current = undefined
    }
  }, [])

  useEffect(() => {
    const chart = chartRef.current
    const series = seriesRef.current
    if (!chart || !series) return

    if (state.status !== 'ok') {
      lastBarRef.current = undefined
      series.setData([])
      return
    }

    const bars = toBars(state.series.candles)
    series.setData(bars as CandlestickData<Time>[])
    lastBarRef.current = bars.at(-1)

    const key = `${state.series.symbol}\0${state.series.range}`
    if (fittedKeyRef.current !== key) {
      fittedKeyRef.current = key
      chart.timeScale().fitContent()
    }
  }, [state])

  useEffect(() => {
    const series = seriesRef.current
    if (!live || !series || state.status !== 'ok' || state.series.symbol !== symbol) {
      return
    }

    const next = applyTick(
      lastBarRef.current,
      live.price,
      live.timestamp,
      state.series.bucketSeconds,
    )
    if (!next) return

    lastBarRef.current = next
    series.update({ ...next, time: next.time as UTCTimestamp })
  }, [live, state, symbol])

  const isEmpty = state.status === 'ok' && state.series.candles.length === 0
  const ariaLabel = `Grafik harga ${symbol}, ${range === '1d' ? '1 hari' : '5 hari'}`

  return (
    <section className="chart-section">
      <div className="chart-head">
        <div className="chart-ranges" role="group" aria-label="Rentang grafik">
          <button
            type="button"
            className="range-btn"
            aria-pressed={range === '1d'}
            onClick={() => setRange('1d')}
          >
            1H
          </button>
          <button
            type="button"
            className="range-btn"
            aria-pressed={range === '5d'}
            onClick={() => setRange('5d')}
          >
            5H
          </button>
        </div>
      </div>

      <div className="chart-stage">
        <div ref={containerRef} className="chart-canvas" role="img" aria-label={ariaLabel} />
        {state.status === 'loading' && (
          <div className="chart-overlay">
            <p>Memuat grafik…</p>
          </div>
        )}
        {isEmpty && (
          <div className="chart-overlay">
            <p>Belum ada data grafik. Data mulai terkumpul saat ada transaksi.</p>
          </div>
        )}
        {state.status === 'error' && (
          <div className="chart-overlay" role="alert">
            <p>Grafik nggak bisa dimuat.</p>
            <button className="btn" type="button" onClick={retry}>
              Coba lagi
            </button>
          </div>
        )}
      </div>

      <p className="chart-note">
        Dari transaksi yang diterima feed gratis, bisa beda dari angka resmi.
      </p>
    </section>
  )
}
