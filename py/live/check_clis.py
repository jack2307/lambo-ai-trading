# -*- coding: utf-8 -*-
"""Does each model have a route that RUNS on this machine?

    python py/live/check_clis.py claude-opus-5 codex/gpt-5.6-sol deepseek-flash

Exit 0 if every model named resolves to an executable that runs; 1 otherwise,
with the reason on stdout.

What this does NOT check is whether that executable is LOGGED IN. `--version`
answers from a signed-out CLI exactly as it does from a signed-in one, and
checking further would mean either a real billed call or parsing each
vendor's login-status output, which changes between releases. So a clean run
here means "the plumbing is there", not "the campaign will trade" - the first
bar is still what proves the second.

WHY THIS IS NOT `Get-Command claude; Get-Command codex`
------------------------------------------------------
Because that answers a different question on each of the two machines this
desk runs on, and gets the important one wrong.

  - The server reaches Codex through the npm package, which puts `codex.CMD`
    on PATH.
  - This desktop has no `codex` on PATH at all. Its working binary is the one
    the Codex app installed under %LOCALAPPDATA%, which `advisor.codex_bin()`
    finds by glob.

A PATH check would pass on one and refuse to start a healthy campaign on the
other. So the check asks the resolver that the trader itself will use, and
then runs what it returns - presence has already proved insufficient once
here, when an npm shim resolved fine and threw "Missing optional dependency
@openai/codex-win32-x64" on every call.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass


def route(model: str) -> tuple[str, str | None]:
    """(what carries this model, the executable) — executable None means none needed."""
    if model.startswith(("codex/", "codex-")):
        import advisor
        return "Codex CLI", advisor.codex_bin()
    if model.startswith(("claude", "opus", "sonnet", "haiku")):
        # The same call advisor.ask_claude makes.
        return "Claude Code CLI", shutil.which("claude")
    return "metered API", None


def main() -> int:
    models = sys.argv[1:]
    if not models:
        print("usage: check_clis.py <model> [<model> ...]")
        return 2
    bad = 0
    for model in models:
        try:
            what, exe = route(model)
        except Exception as e:
            print(f"{model:24s} FAIL  {type(e).__name__}: {e}")
            bad += 1
            continue
        if exe is None:
            print(f"{model:24s} ok    {what}, nothing to check")
            continue
        try:
            p = subprocess.run([exe, "--version"], stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, timeout=120)
        except (OSError, subprocess.TimeoutExpired) as e:
            print(f"{model:24s} FAIL  {what}: {type(e).__name__} running {exe}")
            bad += 1
            continue
        out = (p.stdout or p.stderr).decode("utf-8", "replace").strip().splitlines()
        first = out[0][:120] if out else "(no output)"
        if p.returncode == 0:
            print(f"{model:24s} ok    {what}: {first}")
        else:
            print(f"{model:24s} FAIL  {what} exited {p.returncode}: {first}")
            print(f"{'':24s}       {exe}")
            bad += 1
    if bad:
        print(f"\n{bad} model(s) cannot be reached. Logging in is done from the "
              f"interactive session: `claude` for Anthropic, `codex login` for OpenAI.")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
