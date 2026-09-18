import type { HtfH4 } from '@/lib/api'

/**
 * A READ of the higher timeframe, from the facts the route publishes.
 *
 * THIS IS A PRESENTATION RULE AND NOTHING READS IT BUT THE SCREEN. The route
 * deliberately publishes no verdict — it publishes facts — and the books key
 * their filter on `structure` alone. This word exists because a person asked
 * how to tell at a glance whether the four-hour chart is with them or against
 * them, and three numbers spread across a card do not answer that. It is a
 * summary of the facts below it, not a signal, and every surface that shows it
 * says so.
 *
 * Three votes, each either bull, bear, or abstaining. The word is the majority
 * of the votes CAST; a tie, or fewer than two cast, is RANGE. No score, no
 * percentage: a number between 0 and 100 beside a direction reads as a
 * probability, and nothing here has measured one.
 *
 * Abstention is a real answer and is why this is votes rather than a sum. A
 * flat market abstains on all three and gets RANGE because nothing voted —
 * which is different from two votes cancelling, and the lamps show which
 * happened.
 */

export type Vote = 'bull' | 'bear' | null

export interface BiasVote {
  /** Short name, for the lamp. */
  name: string
  vote: Vote
  /** The rule, stated so it can be checked against the facts on the card. */
  why: string
}

export interface HtfBias {
  word: 'BULLISH' | 'BEARISH' | 'RANGE'
  votes: BiasVote[]
  bull: number
  bear: number
  /** How many of the three voted at all. */
  cast: number
}

/**
 * The rule, in one string, shown in the caption and the hover.
 *
 * IT NAMES ITS OWN TIMEFRAME, and that became load-bearing the moment an H1
 * row appeared beside the H4 one. Three facts now sit near this word and only
 * the four-hour ones voted. A reader who sees `H1: UP · H4: RANGE · BULLISH`
 * and is not told which chart the last word came from will assume it weighed
 * both, because that is the obvious thing for a summary sitting between two
 * rows to do — and it would be a cross-timeframe claim nothing here has
 * measured.
 */
export const BIAS_RULE =
  'The FOUR-HOUR chart only — the H1 row does not vote. Three votes: ' +
  'structure (UP/DOWN), EMA stack (21 vs 55 AND price vs 21), ' +
  'and direction with strength (ADX ≥ 25 with +DI vs −DI). ' +
  'The word is the majority of the votes cast; a tie or fewer than two votes is RANGE.'

export function htfBias(h4: HtfH4 | null | undefined): HtfBias | null {
  if (!h4) return null

  // (a) Structure. RANGE is an abstention rather than a bearish reading: a
  // range is the absence of a direction, not the opposite of one.
  const structure: Vote =
    h4.structure.label === 'UP' ? 'bull' : h4.structure.label === 'DOWN' ? 'bear' : null

  // (b) The stack. BOTH conditions or nothing. Price above a falling EMA21 is
  // genuinely mixed, and calling it bearish because the EMAs are crossed would
  // be reading one half of the test — which is exactly what the word is meant
  // to stop a reader doing by eye.
  let stack: Vote = null
  if (h4.ema21 != null && h4.ema55 != null && h4.last_close != null) {
    if (h4.ema21 > h4.ema55 && h4.last_close > h4.ema21) stack = 'bull'
    else if (h4.ema21 < h4.ema55 && h4.last_close < h4.ema21) stack = 'bear'
  }

  // (c) Direction, but only when there is strength behind it. Below ADX 25 the
  // DI cross is noise and abstains rather than voting weakly — a weak vote and
  // a strong one would count the same, which is how a trend indicator ends up
  // speaking loudest in a chop.
  let momentum: Vote = null
  if (h4.adx14 != null && h4.plus_di14 != null && h4.minus_di14 != null && h4.adx14 >= 25) {
    if (h4.plus_di14 > h4.minus_di14) momentum = 'bull'
    else if (h4.minus_di14 > h4.plus_di14) momentum = 'bear'
  }

  const votes: BiasVote[] = [
    { name: 'structure', vote: structure, why: 'UP or DOWN; a RANGE does not vote' },
    { name: 'EMA stack', vote: stack, why: 'EMA21 vs EMA55 AND price vs EMA21 — both, or no vote' },
    { name: 'momentum', vote: momentum, why: 'ADX14 ≥ 25 with +DI vs −DI; below 25 does not vote' },
  ]

  const bull = votes.filter((v) => v.vote === 'bull').length
  const bear = votes.filter((v) => v.vote === 'bear').length
  const cast = bull + bear
  const word: HtfBias['word'] =
    cast < 2 || bull === bear ? 'RANGE' : bull > bear ? 'BULLISH' : 'BEARISH'

  return { word, votes, bull, bear, cast }
}

/** `▲ ▼ ◆` — the shape, so the word never depends on colour alone. */
export function biasGlyph(word: HtfBias['word']): string {
  return word === 'BULLISH' ? '▲' : word === 'BEARISH' ? '▼' : '◆'
}

/**
 * The same two helpers for STRUCTURE, which is a different vocabulary.
 *
 * `UP / DOWN / RANGE` is a published fact — the route's own `structure.label`,
 * produced by a stated rule. `BULLISH / BEARISH / RANGE` is this file's
 * summary of three of them. They are deliberately different words so that a
 * reader can never be unsure which one they are looking at, and these exist so
 * that showing a structure never requires reaching for the bias helpers and
 * quietly borrowing the bias's meaning.
 *
 * The glyphs match by shape on purpose: up is up on any chart. The WORD is
 * what distinguishes a fact from a summary, not the mark beside it.
 */
export type StructureLabel = 'UP' | 'DOWN' | 'RANGE'

export function structureGlyph(label: StructureLabel): string {
  return label === 'UP' ? '▲' : label === 'DOWN' ? '▼' : '◆'
}

export function structureTint(label: StructureLabel): string {
  return label === 'UP' ? 'text-lc' : label === 'DOWN' ? 'text-lp' : 'text-muted-foreground'
}

export function biasTint(word: HtfBias['word']): string {
  return word === 'BULLISH' ? 'text-lc' : word === 'BEARISH' ? 'text-lp' : 'text-muted-foreground'
}

/**
 * The bias hue as a CSS value, for the ambient wash and the strip pill.
 *
 * `null` on RANGE, and that is the design rather than a gap: nothing lit IS
 * the range signal. Tinting a range would give it a colour that reads as a
 * verdict, when a range is precisely the absence of one — the same reason
 * `htfBias` treats it as an abstention and not as a third direction.
 *
 * A VALUE rather than a class, unlike `biasTint`, because the wash composites
 * it at a measured alpha and `color-mix` needs the colour itself. One
 * function, so the card, the strip and the contrast check cannot disagree
 * about which hue means which word.
 */
export function biasHue(word: HtfBias['word']): string | null {
  return word === 'BULLISH' ? 'var(--lc)' : word === 'BEARISH' ? 'var(--lp)' : null
}

/**
 * How strongly to wash: unanimous reads brighter than a majority.
 *
 * The ONLY strength this rule has. Three votes of three is a different thing
 * from two of three, and it is the one distinction the count supports — so it
 * is a step in alpha and never a second colour, because a second hue would
 * imply a second state rather than more of the same one.
 */
export function biasWash(bias: HtfBias): string {
  return bias.cast >= 3 ? 'var(--bias-wash-strong)' : 'var(--bias-wash-weak)'
}

/** "1 of 3 bearish", or what actually happened when nothing voted. */
export function biasTally(b: HtfBias): string {
  if (b.cast === 0) return 'none of the three voted'
  if (b.word === 'RANGE' && b.bull === b.bear) return `${b.bull} bullish against ${b.bear} bearish`
  const side = b.word === 'BULLISH' ? 'bullish' : b.word === 'BEARISH' ? 'bearish' : ''
  const n = b.word === 'BULLISH' ? b.bull : b.bear
  return side ? `${n} of 3 ${side}` : `${b.cast} of 3 voted`
}
