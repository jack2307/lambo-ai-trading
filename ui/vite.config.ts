import path from 'node:path'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'

// The API lives in a separate process. It is now the Rust `fd-api` binary on
// :8138; the JavaScript prototype on :8137 still answers the same routes and
// `scripts/api-parity.py` compares the two, so pointing back at it is how a
// suspected regression gets bisected.
const API_TARGET = process.env.FLOWDESK_API ?? 'http://127.0.0.1:8138'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    // 5173 is Vite's default and collides with other projects on this machine;
    // a dedicated port keeps the dev URL predictable across restarts.
    port: 5180,
    strictPort: true,
    proxy: {
      '/api': { target: API_TARGET, changeOrigin: true },
    },
  },
})
