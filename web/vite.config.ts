import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// Dev: request /api dan /ws diteruskan ke server Rust, jadi browser cuma kenal satu origin
// (nggak perlu CORS). API_TARGET bisa di-override kalau server jalan di port lain.
const apiTarget = process.env.API_TARGET ?? 'http://127.0.0.1:8080'

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/api': apiTarget,
      '/ws': { target: apiTarget, ws: true },
    },
  },
})
