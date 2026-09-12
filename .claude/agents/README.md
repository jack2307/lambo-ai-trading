# The review team

Seven agents, none of which trades. They exist to make it harder to fool
ourselves during research, which is the only failure mode this project has
actually suffered so far — every real bug found here has been a number that
looked right.

## Why this is not a vote

The obvious design is a committee: each agent gets a vote, majority wins. It is
the wrong design here, for two reasons that do not go away with better prompts.

**Agents are not independent judges.** Several instances of the same model,
handed the same evidence, fail the same way. A committee of experts is useful
because its members are wrong about *different* things and the errors cancel;
correlated agents instead amplify a single blind spot and hand back a number
that feels like confidence. Five agreeing agents is not five opinions.

**Some roles must be able to stop, not to lose a vote.** If the risk role says
no and four advisory roles say yes, a majority rule trades. That inverts the one
separation that matters most: risk has to be able to say no without research
being able to overrule it. The same is true of data integrity — "this tape has a
three-hour hole" is not an opinion to be outvoted.

So authority here is **asymmetric**, and the numbers outrank everybody.

## The order of authority

1. **The gates.** Golden parity, the promising gate, the null distribution.
   Nothing below can overturn them. A unanimous team does not make a failing
   gate pass; if it could, every gate in this project would eventually be talked
   past, which is precisely what they exist to prevent.
2. **The veto roles.** `data-integrity`, `adversary`, `risk`. Each blocks alone,
   and only with evidence — a veto that cannot point at a file, a number or a
   command output is not a veto, it is a mood.
3. **The advisory roles.** `researcher`, `execution-realist`, `portfolio`,
   `historian`. They inform the written decision. They do not make it.
4. **The manager** (the main session). Enforces the process: did each role cite
   evidence, did any veto fire, and what does the decision record say. The
   manager's job is not to break ties by preference.

## Decorrelation is the whole trick

The only real defence against correlated agents is to give them **different
evidence**. A role that reads the walk-forward output and a role that reads the
null distribution disagree for real reasons. Two roles reading the same summary
disagree for no reason at all, and agree for no reason at all.

So every agent here is pointed at a different artefact, and every one of them is
required to quote what it read. An answer with no citation is treated as no
answer.

## Writing it down

Every decision that clears the team is recorded under `docs/decisions/`, with
the date, the question, what each role said, which artefacts were read, and the
outcome. This is not ceremony: `historian` reads those files, and without them
the team re-tests an idea it already rejected every few weeks — a failure mode
that costs more than it sounds like, because the re-test usually produces a
slightly different number and gets believed the second time.

## Using them

These are project agents; they load when a session runs from the flowdesk
directory. Invoke by name, or let the manager fan out.

Run them on a *result*, not on a plan. "Here is the walk-forward output for
ema-cross over two years, tell me what is wrong with it" gets a useful answer.
"Should we trade EMA crosses" does not.
