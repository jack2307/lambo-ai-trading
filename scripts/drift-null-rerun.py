"""Re-run a published hypotheses receipt against the drift-controlled null.

    python scripts/drift-null-rerun.py <id>...            # both nulls, registered seeds
    python scripts/drift-null-rerun.py --scope <id>...    # one seed: side ratios only
    python scripts/drift-null-rerun.py --stage=oos <id>...

Task A of `docs/hypotheses/2026-09-24-what-the-record-cannot-see.md`. For each
`docs/hypotheses/<id>.toml` it runs the SAME batch, on the SAME window, with the
SAME binary, twice: `--null-sides=coin` (the null every published percentile was
read against) and `--null-sides=ratio` (the control carrying the method's own
measured long share). Receipts land in
`docs/research/runs/2026-09-24-drift-repair-a/<id>-<stage>-<sides>.txt`, so a
corrected percentile can be published beside its original from two files that
differ in one flag and nothing else.

**THE SEAL IS ENFORCED HERE, NOT REMEMBERED.** A stage whose window has no
`*_to` bound, or a bound after 2025-09-23, is REFUSED and named — it would read
the withheld year. That is why the corrected table covers the windows it covers
and not others; see
`docs/research/notes/2026-09-24-drift-control-choice.md`.

`--scope` runs one null seed per row. The method's own figures do not depend on
the seed count, so one seed is enough to print `long share: method …`, which is
how the affected list is established by measurement instead of by reading names.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SEARCH = os.environ.get("FD_SEARCH") or os.path.join(ROOT, "target", "release", "search.exe" if os.name == "nt" else "search")
# Where the research corpus lives. The worktree has no `data/`; the primary tree
# holds the bars every closed registration was measured on (docs/research/WHICH-BARS.md).
DATA = os.environ.get("FD_DATA") or os.path.join(ROOT, "data")
OUT = os.path.join(ROOT, "docs", "research", "runs", "2026-09-24-drift-repair-a")

# `data-sealed/` ends here. Nothing this programme runs may read a bar at or
# after it. A DATE, not a count of days: the cutoff is the one the sealed store
# was built with.
SEAL = "2025-09-23"


def load_toml(path: str) -> dict:
    import tomllib

    with open(path, "rb") as f:
        return tomllib.load(f)


def market_tf(spec: str) -> tuple[str, str]:
    market, _, tf = spec.partition(":")
    if not market or not tf:
        sys.exit(f"bad market spec `{spec}`")
    return market, tf


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("ids", nargs="+", help="hypothesis ids, without the .toml")
    ap.add_argument("--stage", default="in", choices=["in", "oos"])
    ap.add_argument("--scope", action="store_true", help="one null seed: side ratios only")
    ap.add_argument("--sides", default="coin,ratio", help="which nulls to run")
    args = ap.parse_args()

    if not os.path.exists(SEARCH):
        sys.exit(f"{SEARCH} not built")
    os.makedirs(OUT, exist_ok=True)
    stage_key = "in_sample" if args.stage == "in" else "out_of_sample"
    refused: list[str] = []

    for hyp_id in args.ids:
        path = os.path.join(ROOT, "docs", "hypotheses", f"{hyp_id}.toml")
        if not os.path.exists(path):
            refused.append(f"{hyp_id}: no such hypothesis file")
            continue
        spec = load_toml(path)
        run_cfg = spec.get("run", {})
        if not run_cfg.get(stage_key):
            refused.append(f"{hyp_id}/{args.stage}: no {stage_key} in [run]")
            continue
        market, tf = market_tf(run_cfg[stage_key])
        lo, hi = run_cfg.get(f"{stage_key}_from"), run_cfg.get(f"{stage_key}_to")
        hi_s = str(hi) if hi else None
        # THE SEAL. An unbounded window reads to the end of the store, which is
        # past the cutoff, so "no bound" is refused exactly like a late bound.
        if hi_s is None:
            refused.append(f"{hyp_id}/{args.stage}: window is unbounded above, so it would read past {SEAL} — refused")
            continue
        if hi_s > SEAL:
            refused.append(f"{hyp_id}/{args.stage}: window ends {hi_s}, past the seal at {SEAL} — refused")
            continue
        bounds = ([f"--from={lo}"] if lo else []) + [f"--to={hi_s}"]
        guard = ["--guards"] if run_cfg.get("guards") else []
        fixed = ["--fixed"] if run_cfg.get("fixed") else []
        seeds = 1 if args.scope else int(run_cfg.get("seeds", 200))

        for sides in args.sides.split(","):
            tag = f"{hyp_id}-{args.stage}-{sides}" + ("-scope" if args.scope else "")
            out_path = os.path.join(OUT, f"{tag}.txt")
            cmd = [
                SEARCH,
                f"--market={market}",
                f"--interval={tf}",
                f"--data={DATA}",
                "--mode=hypotheses",
                f"--batch-file={path}",
                f"--seeds={seeds}",
                f"--null-sides={sides}",
                *bounds,
                *fixed,
                *guard,
            ]
            started = time.time()
            with open(out_path, "w", encoding="utf-8") as out:
                out.write(f"$ {' '.join(cmd)}\n\n")
                out.flush()
                proc = subprocess.run(cmd, stdout=out, stderr=subprocess.STDOUT, cwd=ROOT, check=False)
            print(f"  -> {os.path.relpath(out_path, ROOT)} ({time.time() - started:.0f}s, exit {proc.returncode})")
            if args.scope:
                break  # the method's side ratio is the same under either null

    for line in refused:
        print(f"REFUSED {line}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
