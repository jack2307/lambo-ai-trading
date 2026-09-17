"""Exercise the log rotation in ai_trader and mt5_executor on real files.

    C:\\Python39\\python.exe py/live/rotation_selftest.py

Nothing is mocked except the threshold and the root directory: the functions
under test are the ones the trading processes call on their hot path. It
writes only into a temporary directory and touches no account, no terminal and
no file under `data/`.

What it pins, and why each one is here:

  1  the history is never destroyed. Rotation ROLLS ASIDE - it renames, and
     nothing in either module removes or truncates a file. The count of lines
     across the live file and every rolled one equals the count written, and
     they read back in the order they were written.
  2  a rolled file is never overwritten. The rolled name is stamped to the
     second, so two rolls inside one second would collide; `os.rename` refuses
     that on Windows and POSIX `rename` does NOT, which is why both modules
     check `exists` first. This pins the check, because removing it would look
     harmless and would silently destroy a day's record on Linux.
  3  a rename the platform refuses costs the roll and never the line. A reader
     holding the file open on Windows blocks the rename; `log()` must still
     append, must not raise into a trading loop, and must roll on a later line
     once the reader has gone.

Written 2026-09-17, when the desk moved to a funded account running 24 hours
and both logs had grown without limit since the first day.
"""

from __future__ import annotations

import io
import json
import os
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ai_trader  # noqa: E402
import mt5_executor  # noqa: E402

FAIL = 0

# Small enough to roll in a test, and the only thing mocked. The real values
# are 32 MiB in both modules, with the arithmetic behind them in their
# comments.
SMALL = 4096


def check(name: str, ok: bool) -> None:
    global FAIL
    if not ok:
        FAIL += 1
    print(f"  {'ok  ' if ok else 'FAIL'} {name}")


def lines_in(path) -> list:
    with io.open(path, encoding="utf-8") as f:
        return [line for line in f if line.strip()]


def rolled_beside(path) -> list:
    """Every file that has been rolled aside from `path`, oldest name first."""
    path = Path(path)
    return sorted(str(p) for p in path.parent.glob(path.stem + "-*" + path.suffix))


def tick() -> None:
    """Let the clock move past the second the last roll was stamped with.

    A real book writes a line every fifteen minutes, so this never matters in
    production; it matters here only because the test writes a year of lines
    in a moment.
    """
    time.sleep(1.1)


def decisions_roll() -> None:
    print("ai_trader.log -> decisions.jsonl")
    tmp = tempfile.mkdtemp(prefix="rot-ai-")
    ai_trader.ROOT = tmp
    ai_trader.ROTATE_BYTES = SMALL
    run = "book"
    path = os.path.join(tmp, "data", "paper", run, "decisions.jsonl")

    written = 0
    for burst in range(3):
        if burst:
            tick()
        for _ in range(140):
            ai_trader.log(run, {"at": written, "pad": "z" * 60})
            written += 1

    # The invariant is not "the live file is always small" - a file that has
    # already rolled this second will not roll again until the clock moves.
    # It is "the first line written after the clock moves rolls it".
    tick()
    ai_trader.log(run, {"at": written, "pad": "z" * 60})
    written += 1

    rolls = rolled_beside(path)
    check(f"rolled more than once ({len(rolls)} rolled files)", len(rolls) >= 2)
    check("the live file is small again after the roll", os.path.getsize(path) < SMALL)
    check("rolled names sort chronologically", rolls == sorted(rolls))
    check("every rolled file was full when it rolled",
          all(os.path.getsize(p) >= SMALL for p in rolls))

    total = sum(len(lines_in(p)) for p in rolls) + len(lines_in(path))
    check(f"no line lost: {written} written, {total} on disk", total == written)

    ats = []
    for p in rolls + [path]:
        ats += [json.loads(line)["at"] for line in lines_in(p)]
    check("the record reads unbroken and in order across every roll",
          ats == list(range(written)))


def executor_roll() -> None:
    print("mt5_executor.log -> executor.jsonl")
    tmp = tempfile.mkdtemp(prefix="rot-mt5-")
    mt5_executor.ROTATE_BYTES = SMALL
    path = Path(tmp) / "live" / "acct" / "book" / "executor.jsonl"

    written = 0
    quiet, real = io.StringIO(), sys.stdout
    sys.stdout = quiet  # log() narrates every line; not wanted here
    try:
        for burst in range(3):
            if burst:
                tick()
            for _ in range(140):
                mt5_executor.log(path, "order", n=written, pad="z" * 60)
                written += 1
    finally:
        sys.stdout = real

    rolls = rolled_beside(path)
    check(f"rolled more than once ({len(rolls)} rolled files)", len(rolls) >= 2)
    total = sum(len(lines_in(p)) for p in rolls) + len(lines_in(path))
    check(f"no line lost: {written} written, {total} on disk", total == written)
    ns = []
    for p in rolls + [str(path)]:
        ns += [json.loads(line)["n"] for line in lines_in(p)]
    check("the record reads unbroken and in order across every roll",
          ns == list(range(written)))


def a_rolled_file_is_never_overwritten() -> None:
    print("two rolls inside one second")
    tmp = tempfile.mkdtemp(prefix="rot-clash-")
    path = Path(tmp) / "executor.jsonl"
    path.write_text("the first day\n", encoding="utf-8")
    mt5_executor.ROTATE_BYTES = 1

    mt5_executor.roll_aside(path)
    rolls = rolled_beside(path)
    check("the first roll happened", len(rolls) == 1)
    first = io.open(rolls[0], encoding="utf-8").read()

    path.write_text("the second day\n", encoding="utf-8")
    mt5_executor.roll_aside(path)
    check("a second roll in the same second is declined, not taken",
          rolled_beside(path) == rolls)
    check("the first rolled file still holds the first day",
          io.open(rolls[0], encoding="utf-8").read() == first)
    check("and the live file was not destroyed either",
          path.exists() and path.read_text(encoding="utf-8") == "the second day\n")


def a_refused_rename_costs_the_roll_not_the_line() -> None:
    print("a rename the platform refuses")
    tmp = tempfile.mkdtemp(prefix="rot-held-")
    ai_trader.ROOT = tmp
    ai_trader.ROTATE_BYTES = SMALL
    run = "book"
    path = os.path.join(tmp, "data", "paper", run, "decisions.jsonl")
    for i in range(100):
        ai_trader.log(run, {"at": i, "pad": "z" * 60})

    # Python's open() does not share DELETE on Windows, so this handle is
    # exactly the kind of reader that refuses a rename. fd-api is not: Rust
    # opens with FILE_SHARE_DELETE and a roll under it succeeds (checked
    # 2026-09-17 against the gnu toolchain - the held handle went on reading
    # the file under its new name).
    tick()
    held = io.open(path, encoding="utf-8")
    before, rolls_before = len(lines_in(path)), rolled_beside(path)
    raised = None
    try:
        ai_trader.log(run, {"at": 9999, "pad": "z" * 60})
    except Exception as e:  # noqa: BLE001 - that nothing escapes IS the check
        raised = e
    held.close()

    check("log() did not raise into the trading loop", raised is None)
    check(f"the line was appended anyway ({before} -> {len(lines_in(path))})",
          len(lines_in(path)) == before + 1)
    if sys.platform == "win32":
        check("no roll happened while the reader held it",
              rolled_beside(path) == rolls_before)
    else:
        print("  note not Windows; a reader does not refuse the rename here")

    tick()
    ai_trader.log(run, {"at": 10_000, "pad": "z" * 60})
    check("the next line after the reader left rolled it",
          len(rolled_beside(path)) > len(rolls_before))


def main() -> int:
    decisions_roll()
    executor_roll()
    a_rolled_file_is_never_overwritten()
    a_refused_rename_costs_the_roll_not_the_line()
    print(f"\n{'all checks passed' if not FAIL else str(FAIL) + ' CHECK(S) FAILED'}")
    return 1 if FAIL else 0


if __name__ == "__main__":
    sys.exit(main())
