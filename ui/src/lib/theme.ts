/**
 * Which palette the desk wears, remembered per viewer.
 *
 * `main.tsx` used to do `document.documentElement.classList.add('dark')`
 * unconditionally, so the light palette in index.css had never been rendered
 * once — it existed, was maintained, and nobody had ever looked at it. That is
 * why the light pass that came with this file found five contrast failures
 * nobody had reported: there was no way to report them.
 *
 * Three states and not two. `system` is a real choice, not the absence of one:
 * a person who wants the desk to follow their machine at dusk is asking for
 * something neither `light` nor `dark` can express, and collapsing it into a
 * boolean would make that preference unsayable.
 *
 * Both `.dark` and `data-theme` are stamped. The stylesheet keys off `.dark`
 * and always has; `data-theme` is stamped beside it so anything reading the
 * documented attribute — a future stylesheet, an embedded chart, a person
 * debugging in devtools — sees the same answer as the class does. One writer,
 * both marks, so they cannot disagree.
 */

export type Theme = 'light' | 'dark' | 'system'

const KEY = 'fd.desk.theme'

export function readTheme(): Theme {
  try {
    const raw = localStorage.getItem(KEY)
    return raw === 'light' || raw === 'dark' || raw === 'system' ? raw : 'dark'
  } catch {
    // Private mode, or storage blocked. Dark is the product's own ground, so
    // it is the honest fallback rather than "whatever the OS says".
    return 'dark'
  }
}

export function writeTheme(theme: Theme) {
  try {
    localStorage.setItem(KEY, theme)
  } catch {
    /* private mode: the choice lasts the page */
  }
}

/** What `system` currently resolves to. */
export function systemTheme(): 'light' | 'dark' {
  if (typeof window === 'undefined' || !window.matchMedia) return 'dark'
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark'
}

export function resolveTheme(theme: Theme): 'light' | 'dark' {
  return theme === 'system' ? systemTheme() : theme
}

/**
 * Put the choice on the document.
 *
 * Exported and called from `main.tsx` BEFORE React renders, not from an
 * effect: applying it after the first paint is a flash of the wrong palette on
 * every single load, and on a dark product that flash is a white screen.
 */
export function applyTheme(theme: Theme): 'light' | 'dark' {
  const resolved = resolveTheme(theme)
  const root = document.documentElement
  root.classList.toggle('dark', resolved === 'dark')
  root.setAttribute('data-theme', resolved)
  // Tells the browser which scrollbars, form controls and default UI to draw.
  // Without it a light page keeps dark native widgets and looks half-converted.
  root.style.colorScheme = resolved
  return resolved
}

/**
 * Follow the machine while the choice is `system`, and stop when it is not.
 *
 * Returns an unsubscribe. The listener is only meaningful for `system`; for an
 * explicit choice the OS changing is not an event this desk should react to.
 */
export function watchSystem(theme: Theme, onChange: () => void): () => void {
  if (theme !== 'system' || typeof window === 'undefined' || !window.matchMedia) return () => {}
  const mq = window.matchMedia('(prefers-color-scheme: light)')
  mq.addEventListener('change', onChange)
  return () => mq.removeEventListener('change', onChange)
}
