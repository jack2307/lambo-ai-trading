/**
 * WCAG contrast, measured against `src/index.css` itself.
 *
 * WHY THIS IS A CHECK AND NOT A ONE-OFF. The light palette has now been fixed
 * twice. The first pass measured thirteen pairs and fixed five. The second
 * pass darkened the page and the sidebar to give the desk three surfaces
 * instead of one — and in doing so SILENTLY BROKE three data colours that had
 * passed against pure white: amber fell to 3.63, teal to 4.34, win green to
 * 4.31. Nothing on screen would have said so. A palette is a web of pairs, and
 * changing a ground re-measures every colour that sits on it.
 *
 * It reads the real stylesheet rather than a copy of the values, so it cannot
 * drift from what ships. Both themes, every pair that carries meaning.
 *
 *     node scripts/contrast.check.mjs
 */

import { readFileSync } from 'node:fs'

const css = readFileSync(new URL('../src/index.css', import.meta.url), 'utf8')

/** Pull one `--token: value;` out of a block, so the check reads what ships. */
function block(selector) {
  const at = css.indexOf(selector)
  if (at < 0) throw new Error(`no ${selector} block in index.css`)
  const body = css.slice(at, css.indexOf('\n}', at))
  const out = {}
  for (const [, name, value] of body.matchAll(/--([\w-]+):\s*([^;]+);/g)) out[name] = value.trim()
  return out
}

const light = block(':root {')
const dark = block('.dark {')

const lin = (v) => (v / 255 <= 0.04045 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4)
const rgb = (hex) => {
  const h = hex.trim().replace('#', '')
  if (!/^[0-9a-f]{6}$/i.test(h)) throw new Error(`not a hex colour: ${hex}`)
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16))
}
const lum = ([r, g, b]) => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
const ratio = (a, b) => {
  const [x, y] = [lum(rgb(a)), lum(rgb(b))]
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)
}

let failures = 0
const check = (theme, label, fg, bg, need) => {
  const r = ratio(fg, bg)
  const ok = r >= need
  if (!ok) failures += 1
  console.log(
    `${ok ? 'ok  ' : 'FAIL'}  ${r.toFixed(2).padStart(5)} / ${need.toFixed(1)}  ${theme}  ${label}`,
  )
  if (!ok) console.log(`        ${fg} on ${bg}`)
}

/** Text needs 4.5. Graphics — candles, the position bands — need 3.0. */
const TEXT = 4.5
const GRAPHIC = 3.0

for (const [theme, t] of [
  ['light', light],
  ['dark ', dark],
]) {
  const grounds = [
    ['card', t.card],
    ['page', t.background],
    ['sidebar', t.sidebar],
  ]

  console.log(`\n-- ${theme.trim()}: text on every ground --`)
  for (const [name, bg] of grounds) {
    check(theme, `foreground on ${name}`, t.foreground, bg, TEXT)
    check(theme, `muted on ${name}`, t['muted-foreground'], bg, TEXT)
    // The rung below muted. Light defines it as a solid colour because an
    // opacity ladder cannot stay legible on a light ground; dark has no such
    // token and fades with opacity, which works there.
    if (t['muted-subtle']) check(theme, `muted-subtle on ${name}`, t['muted-subtle'], bg, TEXT)
  }

  console.log(`\n-- ${theme.trim()}: data colours, which are read as numbers --`)
  for (const [name, bg] of grounds) {
    for (const [label, key] of [
      ['win green', 'lc'],
      ['loss red', 'lp'],
      ['tape teal', 'sp'],
      ['amber', 'sc'],
      ['caution', 'caution'],
      ['primary', 'primary'],
    ]) {
      check(theme, `${label} on ${name}`, t[key], bg, TEXT)
    }
  }

  console.log(`\n-- ${theme.trim()}: text ON a fill --`)
  check(theme, 'primary-foreground on primary', t['primary-foreground'], t.primary, TEXT)
  check(theme, 'sidebar-primary-fg on sidebar-primary', t['sidebar-primary-foreground'], t['sidebar-primary'], TEXT)
  check(theme, 'card-foreground on card', t['card-foreground'], t.card, TEXT)
  check(theme, 'sidebar-foreground on sidebar', t['sidebar-foreground'], t.sidebar, TEXT)

  console.log(`\n-- ${theme.trim()}: the chart, where candles are graphics --`)
  check(theme, 'candle up on card', t['candle-up'], t.card, GRAPHIC)
  check(theme, 'candle down on card', t['candle-down'], t.card, GRAPHIC)

  console.log(`\n-- ${theme.trim()}: three surfaces, not one sheet --`)
  // Not a WCAG rule. A minimum separation, because the failure the owner
  // reported was that the rail, the page and the cards were the same colour:
  // sidebar against card measured 1.000 — literally identical.
  const apart = (a, b, label) => {
    const r = ratio(a, b)
    const ok = r >= 1.03
    if (!ok) failures += 1
    console.log(`${ok ? 'ok  ' : 'FAIL'}  ${r.toFixed(3)} / 1.030  ${theme}  ${label}`)
  }
  apart(t.background, t.card, 'page differs from card')
  apart(t.sidebar, t.background, 'sidebar differs from page')
  apart(t.sidebar, t.card, 'sidebar differs from card')

  // The grid sits under the data; the border holds the panel edge. One token
  // for both means the grid competes with the candles.
  //
  // ASSERTED FOR LIGHT ONLY, AND THE EXCEPTION IS PRINTED RATHER THAN
  // DELETED. Dark still uses one colour for both, which is what it has always
  // done and what the owner has said he likes; separating them there is a
  // visual change to a theme nobody asked to change. This line exists so that
  // stays a decision somebody made and can revisit, instead of a fact that
  // quietly left the file when it became inconvenient.
  if (theme.trim() === 'light') {
    apart(t['chart-grid'], t.border, 'chart grid differs from border')
  } else {
    const same = ratio(t['chart-grid'], t.border) < 1.03
    console.log(
      `note  ${same ? 'grid and border are one colour' : 'grid and border differ'} in dark` +
        ` — deliberate, not asserted (see comment)`,
    )
  }
}

console.log(failures === 0 ? '\nall contrast checks pass' : `\n${failures} FAILED`)
process.exit(failures === 0 ? 0 : 1)
