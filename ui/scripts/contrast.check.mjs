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

  // The decider chip's vendor marks. Graphics, not text — the model's NAME
  // beside them is ordinary foreground, already checked above, which is the
  // whole point of the chip's design: the hue only has to be seen, so it
  // never has to be lifted until it stops looking like the brand.
  //
  // Checked on every ground because these rows appear on cards and on the
  // page, and because a published brand colour is chosen against one ground
  // and this desk has three. #D97757 measures 5.92 on the dark card and 2.74
  // on the light page; OpenAI's white is not a colour on a white page at all.
  // The level ladder's four families. 4.5 and not the 3.0 a line needs,
  // because the tags carry TEXT — and on the page as well as the card,
  // because the toggles sit in the chart header.
  //
  // The SEPARATION assertion is the one that would have caught the mistake:
  // the first four measured 1.00–1.03 against each other, four colours
  // distinguished by hue alone on a chart that shows all four at once. A
  // contrast check that only looked at each against its ground would have
  // passed them all.
  // `structure` joined them on 2026-09-19 and is IN THIS LIST rather than
  // exempted from it, which is the whole point of adding it: there was no
  // free hue left, so it is separated by lightness at the end of the ladder,
  // and a claim like that is worth nothing unless the same assertion that
  // measured the other four measures it too. Its marks carry text (`BOS`,
  // `CHoCH`) on the chart, so it needs the 4.5 the others need and not the
  // 3.0 a bare line would.
  console.log(`\n-- ${theme.trim()}: level ladder families --`)
  const families = ['profile', 'gaps', 'liquidity', 'blocks', 'structure']
  for (const family of families) {
    for (const [name, bg] of grounds.filter(([n]) => n !== 'sidebar')) {
      check(theme, `${family} on ${name}`, t[`level-${family}`], bg, TEXT)
    }
  }
  for (let i = 0; i < families.length; i += 1) {
    for (let j = i + 1; j < families.length; j += 1) {
      const [a, b] = [t[`level-${families[i]}`], t[`level-${families[j]}`]]
      const r = ratio(a, b)
      const ok = r >= 1.15
      if (!ok) failures += 1
      console.log(
        `${ok ? 'ok  ' : 'FAIL'}  ${r.toFixed(2)} / 1.15  ${theme}  ${families[i]} vs ${families[j]} separate without colour`,
      )
    }
  }

  console.log(`\n-- ${theme.trim()}: decider chip, house marks --`)
  for (const [name, bg] of grounds) {
    for (const house of ['claude', 'deepseek', 'openai']) {
      check(theme, `${house} mark on ${name}`, t[`house-${house}`], bg, GRAPHIC)
    }
  }

  // THE BIAS WASH CHANGES THE GROUND, so the text has to be measured over the
  // composite and not over the card. This is the whole reason the wash alphas
  // are tokens rather than numbers in a component: a tint nobody can measure
  // is a tint that silently eats contrast.
  //
  // The worst case is the TOP BAND of the card, where the linear fade is at
  // full strength — which is exactly where the header and the bias word sit.
  // A radial from the corner could not be checked this way at all, because
  // its intensity under any given word depends on the card's size.
  console.log(`\n-- ${theme.trim()}: text over the bias wash --`)
  const pct = (token) => {
    const raw = t[token]
    if (!raw) throw new Error(`missing --${token}`)
    const value = Number.parseFloat(raw)
    if (!Number.isFinite(value)) throw new Error(`--${token} is not a percentage: ${raw}`)
    return value / 100
  }
  /** What the eye actually receives: `hue` at `alpha` over `bg`. */
  const over = (hue, bg, alpha) => {
    const [f, b] = [rgb(hue), rgb(bg)]
    return (
      '#' +
      [0, 1, 2]
        .map((i) => Math.round(alpha * f[i] + (1 - alpha) * b[i]).toString(16).padStart(2, '0'))
        .join('')
    )
  }

  for (const [strength, token] of [
    ['majority', 'bias-wash-weak'],
    ['unanimous', 'bias-wash-strong'],
  ]) {
    const alpha = pct(token)
    for (const [word, key] of [
      ['bullish', 'lc'],
      ['bearish', 'lp'],
    ]) {
      const ground = over(t[key], t.card, alpha)
      check(theme, `foreground on ${word} wash (${strength})`, t.foreground, ground, TEXT)
      check(theme, `muted on ${word} wash (${strength})`, t['muted-foreground'], ground, TEXT)
      if (t['muted-subtle']) {
        check(theme, `muted-subtle on ${word} wash (${strength})`, t['muted-subtle'], ground, TEXT)
      }
      // THE BINDING PAIR. The bias word is drawn in the same hue as the wash
      // beneath it, so the two converge as the alpha rises — the thing that
      // caps the wash is the thing the wash exists to highlight.
      check(theme, `the ${word} WORD on its own wash (${strength})`, t[key], ground, TEXT)
    }
  }

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
