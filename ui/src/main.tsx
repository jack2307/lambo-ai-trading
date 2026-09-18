import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import { applyTheme, readTheme } from '@/lib/theme'
import App from './App.tsx'

// Applied BEFORE React renders. Doing it in an effect would flash the wrong
// palette on every load, and on a dark product that flash is a white screen.
//
// The product is a dark room with a tape in it and dark is the default, but
// the light palette is now reachable rather than theoretical — it had never
// once been rendered before 2026-09-18, which is why the pass that came with
// this line found contrast failures nobody had been able to report.
applyTheme(readTheme())

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
