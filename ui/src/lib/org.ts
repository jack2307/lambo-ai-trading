/**
 * Who is on the floor.
 *
 * Shared by the Research screen (which lists the review team) and the Floor
 * screen (which seats them), so the roster is written once. Moved here verbatim
 * from the old Dashboard; nothing about it has been redesigned.
 */

/**
 * The model the main session runs on — the arbiter.
 *
 * Not read live: a browser cannot ask Claude Code which model is driving the
 * terminal. Update this when you `/model`. Everything else on the board reads
 * its model from the agent definition in `.claude/agents/*.md`, where it is
 * pinned and therefore true.
 */
export const SESSION_MODEL = { name: 'Fable 5.1', id: 'claude-fable-5-1' } as const

export const MODELS = {
  fable: { name: 'Fable 5.1', id: 'claude-fable-5-1' },
  sonnet: { name: 'Sonnet 5', id: 'claude-sonnet-5' },
} as const

/**
 * The review team, as configured in `.claude/agents`.
 *
 * `model` mirrors the `model:` line of each definition. The veto roles are
 * pinned to the strongest model available because they can block alone; the
 * advisory roles mostly read files and cite them, which Sonnet does well at a
 * fraction of the cost of running all seven.
 */
export const TEAM = [
  { id: 'data-integrity', title: 'Data Integrity', role: 'Veto', model: MODELS.fable, line: 'Can this data answer the question asked of it?' },
  { id: 'adversary', title: 'Adversary', role: 'Veto', model: MODELS.fable, line: 'Breaks a result that looks good. Owns the null distribution.' },
  { id: 'risk', title: 'Risk', role: 'Veto', model: MODELS.fable, line: 'Limits, tails, and whether a rule is enforced or merely intended.' },
  { id: 'researcher', title: 'Researcher', role: 'Advisory', model: MODELS.sonnet, line: 'Turns ideas into experiments that can fail.' },
  { id: 'execution-realist', title: 'Execution Realist', role: 'Advisory', model: MODELS.sonnet, line: 'What is left after the spread is paid.' },
  { id: 'portfolio', title: 'Portfolio', role: 'Advisory', model: MODELS.sonnet, line: 'How many independent bets are actually on the table.' },
  { id: 'historian', title: 'Historian', role: 'Advisory', model: MODELS.sonnet, line: 'Has this been tried, and what did it cost last time?' },
] as const

/** One member of the team as a seat on the floor. */
export const seat = (id: (typeof TEAM)[number]['id']) => {
  const agent = TEAM.find((a) => a.id === id)!
  return { id: agent.id, title: agent.title, model: agent.model.name }
}
