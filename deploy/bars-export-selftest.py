# -*- coding: utf-8 -*-
"""run-bars-export.cmd, invoked the way Task Scheduler invokes it.

    py -3.9 deploy/bars-export-selftest.py

Runs the real wrapper against a stub exporter in a temp tree. No MetaTrader,
no network, nothing written outside `%TEMP%`.

WHY IT INVOKES THINGS SO AWKWARDLY. An earlier version of this test called the
wrapper as `subprocess.run(["cmd", "/c", path, a, b, c])`, which builds a
correctly quoted command line — and therefore could not see the defect that
took the VPS run down on 2026-09-18.

Task Scheduler hands its `Arguments` field to cmd.exe as ONE string. cmd's
rule for `/c` is that when the remainder starts with a quote it strips the
FIRST and LAST quote and runs what is left, so

    /c "run-bars-export.cmd" "C:\\MT5-cent\\terminal64.exe" "XAUUSD.sc" "M1,M5,H4"

became `run-bars-export.cmd" "…" "…" "M1,M5,H4` — cmd answered "The filename,
directory name, or volume label syntax is incorrect", exited 1 in about five
seconds, and never reached the wrapper, so there was no log at all. The same
mangling shifted the quoting further along the line, which is why a hand-run
delivered only `M1` as `%3`: cmd splits numbered parameters on COMMAS as well
as spaces once the quotes around them are gone.

**A test that invokes the thing more carefully than production does is a test
that passes for a reason production does not have.** So every case here builds
the literal command line and passes it as one string.
"""
from __future__ import annotations

import io
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SRC = os.path.join(HERE, "run-bars-export.cmd")

ALL = ["1m", "5m", "15m", "1h", "4h", "1d"]
TFS = "M1,M5,M15,H1,H4,D1"
TERM = r"C:\MT5-cent\terminal64.exe"

src = io.open(SRC, encoding="utf-8").read()
PY_LINE = next(l for l in src.splitlines() if "mt5_export.py" in l and l.startswith('"C:'))

fails: list[str] = []


def check(name: str, ok: bool, extra: str = "") -> None:
    print(("ok   " if ok else "FAIL ") + name + (("\n       " + extra) if extra and not ok else ""))
    if not ok:
        fails.append(name)


def build(root: str, rc: int, present: list[str]) -> str:
    """A temp copy of the wrapper whose exporter is a stub.

    The stub echoes its own argv, so a test can assert what the wrapper
    actually handed on rather than only what came back.
    """
    os.makedirs(os.path.join(root, "deploy"), exist_ok=True)
    os.makedirs(os.path.join(root, "data", "bars"), exist_ok=True)
    stub = ["@echo off", 'set "ROOT=%~dp0.."', "echo STUB-ARGV=[%*]"]
    stub += [f'echo x> "%ROOT%\\data\\bars\\XAUUSD-{tf}.parquet"' for tf in present]
    stub.append(f"exit /b {rc}")
    io.open(os.path.join(root, "deploy", "stub.cmd"), "w", newline="\r\n").write(
        "\r\n".join(stub) + "\r\n")
    patched = src.replace(
        PY_LINE,
        'call "%ROOT%\\deploy\\stub.cmd" --symbols "%SYMBOLS%" --timeframes "%TIMEFRAMES%" '
        '--since-stored --terminal "%TERMINAL%" >>"%LOG%" 2>&1')
    assert patched != src, "the exporter line was not replaced; this test would prove nothing"
    dst = os.path.join(root, "deploy", "run-bars-export.cmd")
    io.open(dst, "w", newline="\r\n").write(patched)
    return dst


def as_task(inner: str) -> subprocess.CompletedProcess:
    """cmd.exe with ONE argument string, wrapped as install-tasks.ps1 wraps it."""
    return subprocess.run(f'cmd.exe /c "{inner}"', capture_output=True, text=True)


def log_of(root: str) -> str:
    p = os.path.join(root, "data", "paper", "logs", "bars-export.out")
    return io.open(p, encoding="utf-8", errors="replace").read() if os.path.exists(p) else ""


def main() -> int:
    print("=== the argument string the task actually carries ===")
    root = tempfile.mkdtemp(prefix="barswrap-")
    try:
        dst = build(root, 0, ALL)
        out = as_task(f'"{dst}" "{TERM}" "XAUUSD.sc" "{TFS}"')
        text = log_of(root)
        check("the wrapper runs at all (the VPS failure was exit 1 with no log)",
              out.returncode == 0 and text != "", f"rc={out.returncode} log={len(text)}B")
        check("the timeframe list arrives WHOLE, not split on its commas",
              f"timeframes {TFS}" in text, text[:300])
        check("the terminal arrives whole", f"terminal {TERM}" in text, text[:300])
        check("and the exporter is handed the full list",
              f"--timeframes {TFS}" in text or f'--timeframes "{TFS}"' in text, text[:400])
    finally:
        shutil.rmtree(root, ignore_errors=True)

    print("\n=== the list as SEPARATE arguments, which is how a hand-run types it ===")
    root = tempfile.mkdtemp(prefix="barswrap-")
    try:
        dst = build(root, 0, ALL)
        as_task(f'"{dst}" "{TERM}" "XAUUSD.sc" M1 M5 M15 H1 H4 D1')
        check("separate arguments are rejoined into the same list",
              f"timeframes {TFS}" in log_of(root), log_of(root)[:300])
    finally:
        shutil.rmtree(root, ignore_errors=True)

    print("\n=== exit codes ===")
    cases = [
        (0, ALL, 0, "all six written"),
        (1, [], 10, "exporter wrote nothing -> every missing file named"),
        (2, ALL[:-1], 10, "partial: 1d missing -> fatal, and the log names it"),
        (2, ALL, 2, "a partial the wrapper cannot see -> the exporter's 2 survives"),
        (0, ALL[:3], 10, "an old exporter reporting success over absent files -> still refuses"),
        (0, [t for t in ALL if t != "4h"], 10, "only 4h missing -> the htf route's own input"),
    ]
    for rc, present, want, what in cases:
        root = tempfile.mkdtemp(prefix="barswrap-")
        try:
            dst = build(root, rc, present)
            out = as_task(f'"{dst}" "{TERM}" "XAUUSD.sc" "{TFS}"')
            check(f"exporter rc={rc}, {len(present)}/6 present -> {out.returncode} "
                  f"(want {want}): {what}", out.returncode == want)
            if want == 10:
                text = log_of(root)
                for tf in ALL:
                    if tf not in present:
                        check(f"    the log names the missing {tf}",
                              f"XAUUSD-{tf}.parquet" in text)
        finally:
            shutil.rmtree(root, ignore_errors=True)

    print("\n=== no arguments at all: a task registered wrong ===")
    root = tempfile.mkdtemp(prefix="barswrap-")
    try:
        dst = build(root, 0, [])
        out = as_task(f'"{dst}"')
        text = log_of(root)
        check("refuses with 20 and does NOT reach the exporter",
              out.returncode == 20 and "STUB-ARGV" not in text, f"rc={out.returncode}")
        check("and the boundary line was written before the refusal",
              "bars export start" in text, text[:200])
    finally:
        shutil.rmtree(root, ignore_errors=True)

    print("\n=== the quoting itself ===")
    # Not a test of our code. It pins WHY the extra quote pair exists, so that
    # if a future Windows stops mangling the unwrapped form this fails and
    # says the pair is now optional - rather than leaving it in as cargo
    # nobody dares remove.
    root = tempfile.mkdtemp(prefix="barswrap-")
    try:
        dst = build(root, 0, ALL)
        inner = f'"{dst}" "{TERM}" "XAUUSD.sc" "{TFS}"'
        out = subprocess.run(f"cmd.exe /c {inner}", capture_output=True, text=True)
        text = log_of(root)
        check("WITHOUT the extra quote pair cmd still mangles it, so the pair is needed",
              out.returncode != 0 or text == "" or f"timeframes {TFS}" not in text,
              f"rc={out.returncode} log={len(text)}B")
    finally:
        shutil.rmtree(root, ignore_errors=True)

    # The edit most likely to undo all of this is someone "tidying" a doubled
    # quote out of install-tasks.ps1, so the emitted action is checked too.
    out = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass",
                          "-File", os.path.join(HERE, "install-tasks.ps1")],
                         capture_output=True, text=True, cwd=ROOT)
    line = next((l for l in out.stdout.splitlines()
                 if "run-bars-export.cmd" in l and "action" in l), "")
    check('install-tasks.ps1 emits cmd.exe /c ""<script>" ..."', '/c ""' in line, line.strip()[:160])
    check("  with the timeframe list inside it intact", TFS in line, line.strip()[:160])

    print()
    print("all checks passed" if not fails else f"{len(fails)} FAILED")
    return 1 if fails else 0


if __name__ == "__main__":
    raise SystemExit(main())
