"""Re-run every published row whose PF_usd lies in the declared band
[0.700, 1.700] and that is reproducible from its receipt's own echoed command.

Registration: docs/decisions/2026-10-09-pf-unit-audit.md.

Input is `gate_legs_rows.json` from `py/research/gate_legs_check.py` (reused
byte-identical from `agent/gate-legs`, itself fed by
`floor_audit_{harvest,census}.py` reused byte-identical from
`agent/floor-audit`). This file harvests nothing and parses no receipt text of
its own beyond the `$ ...search...` command echo.

Only the binary path, the batch-file path and the data/config roots are
rewritten onto this worktree; every other flag of the published command is
passed through untouched, except an optional `--seeds=` override.

`--seeds=` only sizes the matched-null population. MEASURED on
`2026-09-13-recent-year-sessions.toml` (xauusd 15m, 2025-09-13 -> 2026-09-12):
`--seeds=1` and `--seeds=200` print the method rows and EVERY `Lbar ... PF_r
... PF_usd ... residual` line byte-identical. Only the null p50/p95, the
percentile and the verdict WORD move, and this axis reads none of those - it
reads the published verdict off the published row.

    argv: OUT [subdir] [seeds-override, 0 = keep published] [workers]
"""
import json
import os
import re
import subprocess
import sys
import hashlib
from concurrent.futures import ThreadPoolExecutor

OUT = sys.argv[1]
SUB = sys.argv[2] if len(sys.argv) > 2 else "runs"
SEEDS = int(sys.argv[3]) if len(sys.argv) > 3 else 0
WORKERS = int(sys.argv[4]) if len(sys.argv) > 4 else 1
LO, HI = 0.700, 1.700
EXE = "E:/rust/fd-stop-width/target-sw/release/search.exe"
REPO = "E:/rust/fd-pfr-audit"
DATA = "E:/rust/flowdesk/data"


def key(r):
    return (r["label"], r["trades"], round(r["pf"], 3), round(r["e"], 3))


def rewrite(c):
    """The published command with only the paths moved onto this worktree."""
    toks = c.replace("\\", "/").split()
    out = [EXE]
    for t in toks[1:]:
        if t.startswith("--batch-file="):
            t = "--batch-file=" + REPO + "/docs/hypotheses/" + t.split("docs/hypotheses/")[-1]
        elif t.startswith("--data=") or t.startswith("--config="):
            continue
        elif t.startswith("--seeds=") and SEEDS:
            t = f"--seeds={SEEDS}"
        out.append(t)
    if SEEDS and not any(t.startswith("--seeds=") for t in out):
        out.append(f"--seeds={SEEDS}")
    out += [f"--data={DATA}", f"--config={REPO}/config"]
    return out


def good(path):
    """A run file is usable only if the engine got to its flag table and
    printed at least one row. Anything else is re-run, not read."""
    if not os.path.exists(path) or os.path.getsize(path) < 400:
        return False
    t = open(path, encoding="utf-8", errors="replace").read()
    return "# exit 0" in t and "flags:" in t and "Lbar" in t


def main():
    rows = json.load(open(f"{OUT}/gate_legs_rows.json", encoding="utf-8"))
    cmd_of = json.load(open(f"{OUT}/cmd_of.json", encoding="utf-8"))
    band = [r for r in rows if LO <= r["pf"] <= HI]
    repro = {}
    for r in band:
        cs = set()
        for s in r["shas"]:
            cs.update(cmd_of.get(s, []))
        if cs:
            repro.setdefault(key(r), set()).update(cs)
    cmds = sorted({c for v in repro.values() for c in v})
    print(f"band rows                 : {len(band)}")
    print(f"reproducible band keys    : {len(repro)}")
    print(f"commands to run           : {len(cmds)}  (seeds override: {SEEDS or 'none'})")
    os.makedirs(f"{OUT}/{SUB}", exist_ok=True)
    index = {hashlib.md5(c.encode()).hexdigest()[:12]: c for c in cmds}
    json.dump(index, open(f"{OUT}/{SUB}/index.json", "w"), indent=0)
    json.dump({"|".join(map(str, k)): sorted(v) for k, v in repro.items()},
              open(f"{OUT}/repro_band.json", "w"), indent=0)

    todo = [(h, c) for h, c in sorted(index.items())
            if not good(f"{OUT}/{SUB}/{h}.txt")]
    order = sys.argv[5] if len(sys.argv) > 5 else "fwd"
    if order == "rev":
        todo.reverse()
    elif order == "shuf":
        import random
        random.Random(20261009).shuffle(todo)
    print(f"already usable            : {len(index) - len(todo)}")
    print(f"to run                    : {len(todo)}", flush=True)

    def one(hc):
        h, c = hc
        if good(f"{OUT}/{SUB}/{h}.txt"):
            return
        argv = rewrite(c)
        p = subprocess.run(argv, capture_output=True, text=True,
                           errors="replace", cwd=REPO)
        tmp = f"{OUT}/{SUB}/{h}.part"
        with open(tmp, "w", encoding="utf-8") as fh:
            fh.write("$ " + " ".join(argv) + "\n")
            fh.write(f"# PUBLISHED COMMAND: {c}\n")
            fh.write(f"# exit {p.returncode}\n")
            fh.write(p.stdout)
            if p.stderr:
                fh.write("\n--- stderr ---\n" + p.stderr)
        os.replace(tmp, f"{OUT}/{SUB}/{h}.txt")
        print(f"done {h} exit={p.returncode} {' '.join(argv[1:])[:110]}", flush=True)

    with ThreadPoolExecutor(max_workers=WORKERS) as ex:
        list(ex.map(one, todo))
    print("ALL DONE")


if __name__ == "__main__":
    main()
