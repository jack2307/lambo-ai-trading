"""Run one hypothesis file through the standard pipeline and keep every output.

    python scripts/research-run.py docs/hypotheses/<id>.toml [--stage in|oos|all] [--seeds N]

Reads the `[run]` table of the hypothesis file:

    [run]
    in_sample = "xauusd:1m"
    out_of_sample = "xauduka:1m"
    out_of_sample_to = "2025-04-10"   # optional UTC date bounds (to is exclusive):
    in_sample_from = "2025-04-11"     #   *_from / *_to for either stage
    seeds = 200             # matched-null runs per hypothesis
    direction_samples = 1000

and writes to docs/research/runs/<id>/:

    in-sample.txt        search --mode=hypotheses --batch-file=... on the in-sample market
    in-sample-fixed.txt  the registered parameters replayed there, when `[run] fixed = true`
    direction-<slug>.txt search --mode=null-dir per hypothesis row (in-sample; --stage dir re-runs only these)
    out-of-sample.txt    the same batch on the out-of-sample market  (--stage oos or all)
    out-of-sample-fixed.txt  the registered parameters replayed there, no re-selection

The out-of-sample stage refuses to run until in-sample.txt exists: the order
is the discipline. Nothing here decides anything; it runs the machinery and
keeps the receipts for the record.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# `FD_SEARCH` points the runner at another build of the binary — a second
# target directory, when the release one is busy in a long run.
SEARCH = os.environ.get("FD_SEARCH") or os.path.join(ROOT, "target", "release", "search.exe" if os.name == "nt" else "search")


def load_toml(path: str) -> dict:
    try:
        import tomllib  # Python 3.11+
        with open(path, "rb") as f:
            return tomllib.load(f)
    except ImportError:
        try:
            import tomli  # type: ignore
            with open(path, "rb") as f:
                return tomli.load(f)
        except ImportError:
            sys.exit("need Python 3.11 or `pip install tomli`")


def market_tf(spec: str) -> tuple[str, str]:
    market, _, tf = spec.partition(":")
    if not market or not tf:
        sys.exit(f"bad market spec `{spec}`: expected market:timeframe")
    return market, tf


def bounds(run_cfg: dict, stage: str) -> list[str]:
    """`--from/--to` for a stage, from `<stage>_from` / `<stage>_to`."""
    out = []
    for key, flag in ((f"{stage}_from", "--from"), (f"{stage}_to", "--to")):
        if run_cfg.get(key):
            out.append(f"{flag}={run_cfg[key]}")
    return out


def run(args: list[str], out_path: str) -> None:
    env = dict(os.environ)
    started = time.time()
    with open(out_path, "w", encoding="utf-8") as out:
        out.write(f"$ {' '.join(args)}\n\n")
        out.flush()
        proc = subprocess.run(args, stdout=out, stderr=subprocess.STDOUT, cwd=ROOT, env=env, check=False)
    print(f"  -> {os.path.relpath(out_path, ROOT)} ({time.time() - started:.0f}s, exit {proc.returncode})")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("hypothesis", help="docs/hypotheses/<id>.toml")
    ap.add_argument("--stage", choices=["in", "dir", "oos", "all"], default="in", help="dir = the in-sample direction nulls only (after a null amendment)")
    ap.add_argument("--seeds", type=int, default=None)
    ap.add_argument("--direction-samples", type=int, default=None)
    args = ap.parse_args()

    path = os.path.abspath(args.hypothesis)
    spec = load_toml(path)
    run_cfg = spec.get("run", {})
    seeds = args.seeds or int(run_cfg.get("seeds", 200))
    direction = args.direction_samples or int(run_cfg.get("direction_samples", 1000))
    hyp_id = os.path.splitext(os.path.basename(path))[0]
    out_dir = os.path.join(ROOT, "docs", "research", "runs", hyp_id)
    os.makedirs(out_dir, exist_ok=True)

    if not os.path.exists(SEARCH):
        sys.exit(f"{SEARCH} not built: cargo build --release -p fd-backtest --bin search")

    bases = sorted({h["base"] for h in spec.get("hypothesis", [])})
    if not bases:
        sys.exit("no [[hypothesis]] tables")

    # Direction nulls run per hypothesis, with its preset: a control on the
    # method's defaults says nothing about the variant actually tested.
    def direction_runs():
        for h in spec["hypothesis"]:
            overrides = h.get("overrides", {})
            params = ",".join(f"{k}={v}" for k, v in overrides.items())
            filters = ";".join(h.get("filters", []))
            slug = re.sub(r"[^A-Za-z0-9]+", "-", h["label"]).strip("-")
            extra = ([f"--params={params}"] if params else []) + ([f"--filters={filters}"] if filters else [])
            yield h["base"], slug, extra

    if args.stage in ("in", "all"):
        market, tf = market_tf(run_cfg.get("in_sample", ""))
        print(f"in-sample: {market} {tf}, {len(spec['hypothesis'])} hypotheses, {seeds} null seeds")
        b = bounds(run_cfg, "in_sample")
        run(
            [SEARCH, f"--market={market}", f"--interval={tf}", "--mode=hypotheses", f"--batch-file={path}", f"--seeds={seeds}", *b],
            os.path.join(out_dir, "in-sample.txt"),
        )
        if run_cfg.get("fixed"):
            # A registered-parameters replay on the primary too, when the
            # hypothesis pins its parameters (no selection to walk forward).
            run(
                [SEARCH, f"--market={market}", f"--interval={tf}", "--mode=hypotheses", "--fixed", f"--batch-file={path}", f"--seeds={seeds}", *b],
                os.path.join(out_dir, "in-sample-fixed.txt"),
            )
    if args.stage in ("in", "dir", "all"):
        market, tf = market_tf(run_cfg.get("in_sample", ""))
        b = bounds(run_cfg, "in_sample")
        for base, slug, preset in direction_runs():
            print(f"direction null: {slug} ({base}) on {market} {tf}, {direction} samples")
            run(
                [SEARCH, f"--market={market}", f"--interval={tf}", "--mode=null-dir", f"--strategy={base}", f"--samples={direction}", *preset, *b],
                os.path.join(out_dir, f"direction-{slug}.txt"),
            )

    if args.stage in ("oos", "all"):
        if not os.path.exists(os.path.join(out_dir, "in-sample.txt")):
            sys.exit("out-of-sample refused: run the in-sample stage first and record its result")
        market, tf = market_tf(run_cfg.get("out_of_sample", ""))
        b = bounds(run_cfg, "out_of_sample")
        print(f"out-of-sample: {market} {tf} {' '.join(b)}")
        run(
            [SEARCH, f"--market={market}", f"--interval={tf}", "--mode=hypotheses", f"--batch-file={path}", f"--seeds={seeds}", *b],
            os.path.join(out_dir, "out-of-sample.txt"),
        )
        for base, slug, preset in direction_runs():
            run(
                [SEARCH, f"--market={market}", f"--interval={tf}", "--mode=null-dir", f"--strategy={base}", f"--samples={direction}", *preset, *b],
                os.path.join(out_dir, f"direction-{slug}-oos.txt"),
            )
        # The replay: registered parameters, no re-selection on the new window.
        run(
            [SEARCH, f"--market={market}", f"--interval={tf}", "--mode=hypotheses", "--fixed", f"--batch-file={path}", f"--seeds={seeds}", *b],
            os.path.join(out_dir, "out-of-sample-fixed.txt"),
        )

    # A one-line digest so the arbiter can read the verdicts without the tables.
    for name in ("in-sample.txt", "out-of-sample.txt"):
        p = os.path.join(out_dir, name)
        if os.path.exists(p):
            text = open(p, encoding="utf-8").read()
            survivors = re.findall(r"^Survivors: (.*)$", text, re.M)
            print(f"{name}: {'SURVIVORS ' + survivors[0] if survivors else 'nothing survived' if 'Nothing survived' in text else 'no verdict line'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
