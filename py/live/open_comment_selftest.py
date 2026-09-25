"""The opening order's comment must fit, stay whole, and stay distinguishable.

WHY THIS FILE EXISTS. The open comment was `f"flowdesk {run}"[:31]` inline, and
31 is the limit MT5 DOCUMENTS rather than the one this account has been
measured at. `COMMENT_MAX` in `mt5_executor.py` records the measurement: 22 to
25 accepted, 31 refused with `order_send` returning None, 1,389 times.

It bit again on 2026-09-24, the first time a run id on this account was longer
than `ai-xau-ds-ctx`. Three coin books were put on real money; their ids are 21,
27 and 29 characters, so two of them sent 31-character comments and the account
placed NOTHING for 26 hours while looking like it was trading. 624 refused
orders on one book alone. Nobody checked that an order could be sent - only
that the process had started and connected.

Run: C:\\Python39\\python.exe py/live/open_comment_selftest.py
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from mt5_executor import COMMENT_MAX, OPEN_COMMENT_MAX, open_comment  # noqa: E402

FAILURES = []


def check(name, condition, detail=""):
    status = "ok  " if condition else "FAIL"
    print("  %s %s%s" % (status, name, ("  -- " + detail) if detail and not condition else ""))
    if not condition:
        FAILURES.append(name)


# Every run id this account has ever mirrored or been asked to mirror.
RUNS = [
    "ai-xau-ds-ctx",
    "xau-macd-asia",
    "ai-xau-terra-ctx-coin",
    "ai-xau-ds-plan-trigger-coin",
    "ai-xau-ds-ctx-htf-filter-coin",
    "ai-xau-ds-ctx-htf-filter",
    "ai-xau-opus-ctx-b",
    "ai-xau-ds-plan-trigger",
    "coin-live-101",
]

print("open_comment, against OPEN_COMMENT_MAX = %d (COMMENT_MAX = %d)" % (OPEN_COMMENT_MAX, COMMENT_MAX))
print()

for run in RUNS:
    comment = open_comment(run)
    print("  %-32s -> %-26s %2d" % (run, comment, len(comment)))
print()

for run in RUNS:
    comment = open_comment(run)
    check(
        "%s fits" % run,
        len(comment) <= OPEN_COMMENT_MAX,
        "%d characters: %r" % (len(comment), comment),
    )

check(
    "nothing sits on the measured ceiling",
    all(len(open_comment(r)) < COMMENT_MAX for r in RUNS),
    "this file argues against sitting on a measured boundary",
)

check(
    "no doubled separator",
    all("--" not in open_comment(r) for r in RUNS),
    "rstrip('-') on the kept head is what prevents ai-xau-ds-ctx-htf--coin",
)

# The two books this account was already mirroring must be byte-identical to
# what they sent before the fix, or this is not an additive change to a live
# money path.
check(
    "ai-xau-ds-ctx unchanged",
    open_comment("ai-xau-ds-ctx") == "flowdesk ai-xau-ds-ctx",
    repr(open_comment("ai-xau-ds-ctx")),
)
check(
    "xau-macd-asia unchanged",
    open_comment("xau-macd-asia") == "flowdesk xau-macd-asia",
    repr(open_comment("xau-macd-asia")),
)

# A book and its own coin control must not collapse to the same string: the
# comment is the one field a person reads to tell them apart in a deal history,
# and a plain truncation makes them identical.
for book in ["ai-xau-ds-ctx-htf-filter", "ai-xau-ds-plan-trigger", "ai-xau-ds-ctx"]:
    check(
        "%s differs from its coin" % book,
        open_comment(book) != open_comment(book + "-coin"),
        "%r vs %r" % (open_comment(book), open_comment(book + "-coin")),
    )

# The discriminating last segment survives, because that is the whole point of
# keeping the tail rather than truncating from the right.
for run in [r for r in RUNS if r.endswith("-coin")]:
    check(
        "%s keeps its -coin" % run,
        open_comment(run).endswith("-coin"),
        repr(open_comment(run)),
    )

# A pathological id: no separator at all and far too long. It must still fit
# rather than raise or return something oversized.
long_flat = "x" * 80
check(
    "a separator-less 80-character id still fits",
    len(open_comment(long_flat)) <= OPEN_COMMENT_MAX,
    repr(open_comment(long_flat)),
)

# An id whose last segment alone exceeds the budget falls back to a plain cut
# rather than producing a negative slice.
pathological = "a-" + "y" * 60
check(
    "an oversized last segment falls back safely",
    len(open_comment(pathological)) <= OPEN_COMMENT_MAX,
    repr(open_comment(pathological)),
)

print()
if FAILURES:
    print("FAILED: %d" % len(FAILURES))
    for f in FAILURES:
        print("   - %s" % f)
    sys.exit(1)
print("all checks passed")
