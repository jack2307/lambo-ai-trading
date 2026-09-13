# flowdesk design language

Two surfaces, one palette. Read the first section before adding a screen,
because most mistakes here are a screen built in the wrong mode.

## The two modes

**Overview mode** — the Dashboard. Dark, elevated cards on a near-black ground,
generous radius, generous space, a single bright accent. Its job is to let
someone arriving cold see the state of the system in one screen: what is
running, what passed, what is blocked. Cards carry meaning through grouping and
rank rather than through density.

**Terminal mode** — Workbench and Tape. Dense, flat, small radius, tight
leading, a table that fills the viewport. Its job is to let someone already
working read a lot of numbers without scrolling. Cards and padding are the enemy
here.

The Workbench is the model for terminal mode. Three regions, fixed to the
viewport, no cards: a **rail** on the left for what you *set* (strategy,
parameters, run, result), the **chart** filling the middle for what you *see*,
and a **dock** along the bottom for what you *read* — full-width tables with
headers, tabbed (leaderboard, trades, fill model). A number that belongs in a
table goes in the dock, not in a sidebar card that can hold four columns.

**Do not merge them.** An airy card layout over a tape reader wastes the screen
someone is reading; a dense grid on an overview screen is a wall nobody parses.
When adding a screen, decide which mode it is in first, then follow that mode's
rules all the way. A screen that is half one and half the other reads as unfinished.

## Palette

This is a Backcom product and wears Backcom's colours: **lime on near-black**,
lifted from backcom.io's own CSS. Tokens live in `src/index.css` and are the
only source of colour. Never write a hex value in a component; if a colour is
missing, add a token.

| token | dark | role |
|---|---|---|
| `--background` | `#0a0a0a` | the ground. Brand near-black. **Neutral**, not blue-black — a blue ground tints lime toward teal |
| `--card` | `#131413` | a raised surface |
| `--elevated` | `#1a1c1a` | a surface raised above a card (hero, active row) |
| `--border` | `#242724` | hairlines, 1px, never heavier |
| `--foreground` | `#ffffff` | primary text |
| `--muted-foreground` | `#9aa39a` | labels, units, secondary text (brand grey) |
| `--primary` | `#8dff08` | **the one accent.** Brand lime. Buttons, links, focus, active nav |
| `--primary-foreground` | `#071400` | text on lime. **Never white** — white on lime fails contrast and is not the brand |
| `--brand-mint` | `#08ffb5` | Brand secondary. **Gradients and glows only.** Never a control |

### The rule that matters most

**One chrome accent, and it is lime.** Everything a person can press, focus or
navigate with is `--primary`. Nothing else is.

Green, teal, red and amber remain *data encodings* — the four flow classes and
up/down on a number — and no control of any kind uses them. Lime (`#8dff08`,
yellow-green) and the bull green (`--lc`, `#46c98a`, sea-green) are far enough
apart to coexist; a reader does not confuse a lime button with a green delta.

**Mint is the exception that proves the rule.** It is the brand's second
colour and it is close to the tape's teal (`--sp`). So it is confined to the
places the tape never appears: the hero gradient, the arbiter's aura in the
pantheon. If mint ever sits next to an SP figure, one of them is in the wrong
place.

### Gradients

The hero card uses one: **mint into lime at 324°**, backcom.io's own pairing
and angle, laid as radial washes under the content. That is the budget — one
gradient, on one card, per screen. A gradient on every card is the house style
of every AI product shipped since 2023 and says nothing about this one.

A gradient must sit under content rather than compete with it. The brand angle
is already off-axis; do not straighten it to 45°.

## Elevation

Three levels, no more:

```
background  →  card (border, no shadow)  →  elevated (border + tinted shadow)
```

Shadows are **tinted with the background hue**, never pure black at low opacity:
`0 8px 32px -12px rgb(4 6 4 / 0.75)` — the ground is neutral with a breath of
green, and so is its shadow.

## Radius

- Cards and panels: `--radius-xl` (16px)
- Buttons, chips, inputs: `--radius-md`
- Pills and avatars: `9999px`
- Inner elements are always *tighter* than their container. A 16px radius inside
  a 16px container reads as a mistake.

## Type

- Prose and labels: Geist Variable.
- **All numbers are monospace with tabular figures.** Use the `.tnum` utility.
  A column of proportional digits does not line up and cannot be scanned, which
  is the only reason the column exists.
- Sizes: `11px` labels (uppercase, `tracking-wide`), `13px` body, `15px` card
  titles, `28px`+ hero figures. Nothing between 15 and 28 — the gap is
  deliberate and keeps hierarchy legible.
- Weight carries hierarchy, size carries importance. Do not use both at once for
  the same jump.

## Motion

- Transitions 150–200ms on `transform` and `opacity` only. Never on `width`,
  `height`, `top` or `left`.
- Hover on an interactive card: `translateY(-1px)` plus a border lightening.
  Nothing scales — a card that grows shifts everything around it.
- Press: `scale(0.985)`.
- Every interactive element needs a visible focus ring. This is not optional and
  it is not a hover state.

## Density and the numbers

- A figure is never shown without its unit or its scale. `1.033` is not a
  result; `PF 1.033` is.
- A figure derived from too little data says so **next to the figure**, not in a
  tooltip. The dashboard's job is to stop someone believing a number the data
  cannot support — this project has already produced a profit factor of 2.9 from
  nine trades in a 28-hour window, and it looked exactly like a real result.
- Percentages of a distribution beat raw scores. "85th percentile of noise"
  tells a reader something "1.033" does not.

## States, all four

Every list, table and card needs:

1. **Loading** — a skeleton shaped like the content, never a spinner.
2. **Empty** — says what would fill it and how. "No tape yet — run
   `fd-ingest --bin collect`" beats "No data".
3. **Error** — the server's sentence, verbatim, inline. Never a status code
   alone, never a toast for something that is still broken when it closes.
4. **Stale** — this one is specific to us. A number computed from a store that
   is still being written must show what it was computed over. Two runs an hour
   apart have given different answers for the same strategy.

## Components

Prefer the shadcn primitives in `src/components/ui`. Add a new one only when the
composition genuinely does not exist; a wrapper that renames props is churn.

Anything that renders data lives in a component that takes plain props. Nothing
in `src/components` fetches.

## 3D scenes

One is allowed per screen, and only in overview mode. The floor plan on the
dashboard is the model; a second scene on the same page is decoration.

- **It reads the tokens.** Every colour comes from `getComputedStyle` at mount
  (`--primary`, `--caution`, `--card`, `--foreground`, `--muted-foreground`,
  `--border`). A scene with its own palette is a different product embedded in
  ours.
- **It encodes something.** Position, height, colour and motion each carry a
  meaning that is written down in the component's doc comment. On the floor,
  the rooms are the loop's steps in the order a file walks them, the three
  veto rooms stand between the engine and the arbiter's office because that
  is where a veto sits, and an amber stamp sends the file back. A scene where
  the layout is arbitrary is a screensaver.
- **Everything in it is also in text.** A legend panel beside the scene states
  what the scene shows. Nothing may be *only* visual; a reader with the canvas
  disabled loses nothing they need.
- **Plain three.js in a `ref`, not a React wrapper.** Every geometry, material,
  texture and the renderer are disposed exactly once in the effect's cleanup.
  A scene that leaks a WebGL context on navigation takes the tab down after
  enough visits.
- **Lazy-loaded.** three.js is half a megabyte. `React.lazy` + `Suspense` with a
  skeleton, so screens that draw no triangle never download the renderer.
- **`prefers-reduced-motion` stops auto-rotation, idle motion and pulses** by
  default, and the scene says so and offers one control to run anyway. The
  scene still renders and still responds to drag. A Windows machine with
  animation effects off reports reduced motion, and a floor that silently
  stands still on it reads as broken, not considerate.
- **`powerPreference: 'low-power'`, pixel ratio capped at 2.** This is a
  dashboard, not a game.
- **Labels are DOM, positioned by projection.** Crisper than canvas text, and in
  the product's own font.
