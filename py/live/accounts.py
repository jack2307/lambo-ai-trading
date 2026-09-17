# -*- coding: utf-8 -*-
"""Read `config/accounts.toml` and print it as JSON.

The launcher is PowerShell and PowerShell has no TOML reader, but it already
depends on this Python, so the registry stays TOML - where it can carry the
comments that make it editable by hand - and this is the one place that parses
it for the shell.

Deliberately a separate file from the executor: the executor mirrors ONE book
into ONE account and must not grow an opinion about which accounts exist.

    python py/live/accounts.py                 # every enabled account
    python py/live/accounts.py --id demo       # one, enabled or not
    python py/live/accounts.py --all           # every account, enabled or not

`enabled` IS REPORTED AND IS NEVER A PERMISSION HERE. This is a reader: `--id`
and `--all` answer for a disabled account on purpose, because inspecting an
account you have deliberately turned off is a thing people need to do, and a
reader that hid one would be lying about the file it exists to report. Every
record carries `enabled` and the caller decides what to do with it.

Nothing enforced it on EITHER side until 2026-09-17. `start_executors.ps1
-Account <id>` reaches here as `--id`, which skips the enabled filter, and the
launcher never read the field - so an account switched off because a second
machine still holds a terminal logged into it could be started simply by
naming it. The check now lives in the launcher, which is the only thing that
starts processes and therefore the only place where "off" has to mean "does
not run". Do not add it here: two places that can refuse is two places to look
when something does not start, and only one of them can be the right one.
"""
import argparse
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DEFAULT = os.path.join(ROOT, "config", "accounts.toml")

# Fields with no sensible default: an account missing any of them is not an
# account, and guessing one would mean guessing which terminal to trade.
REQUIRED = ("id", "login", "terminal")


def load(path):
    try:
        import tomli
    except ImportError:  # pragma: no cover - environment, not logic
        sys.exit("tomli is not installed: py -3.9 -m pip install tomli")
    if not os.path.isfile(path):
        sys.exit(f"no account registry at {path}")
    with open(path, "rb") as fh:
        try:
            doc = tomli.load(fh)
        except Exception as e:  # noqa: BLE001
            sys.exit(f"{path}: {e}")

    out = []
    seen_ids, seen_logins, seen_terminals = set(), set(), set()
    for i, a in enumerate(doc.get("account") or []):
        missing = [k for k in REQUIRED if a.get(k) in (None, "")]
        if missing:
            sys.exit(f"{path}: account #{i + 1} is missing {', '.join(missing)}")
        aid = str(a["id"])
        if aid in seen_ids:
            sys.exit(f"{path}: two accounts share the id {aid!r}")
        # Two entries on one terminal would be two executor sets reconciling
        # one account against different books: both would see the same
        # positions, and each would try to close what the other opened.
        term = os.path.normcase(str(a["terminal"]))
        if term in seen_terminals:
            sys.exit(f"{path}: {aid!r} reuses a terminal another account already claims: {a['terminal']}")
        if a["login"] in seen_logins:
            sys.exit(f"{path}: {aid!r} reuses login {a['login']}")
        seen_ids.add(aid)
        seen_logins.add(a["login"])
        seen_terminals.add(term)
        out.append({
            "id": aid,
            "label": a.get("label") or aid,
            "login": int(a["login"]),
            "server": a.get("server"),
            "terminal": str(a["terminal"]),
            "lot_scale": float(a.get("lot_scale", 1.0)),
            # Both default to the cautious reading: a new account sends nothing
            # until it is told to, and an account with no `runs` mirrors no book
            # rather than every book.
            "dry_run": bool(a.get("dry_run", True)),
            "enabled": bool(a.get("enabled", True)),
            # Real money is opt-in, and stays opt-in when the key is absent,
            # misspelled, or the file predates this code.
            "real_money": bool(a.get("real_money", False)),
            # What this account calls its instruments.
            #
            # An account property, not a market property: `xauusd` is the same
            # gold on both, but the demo lists it as XAUUSD and the cent
            # account as XAUUSD.sc. '' keeps every account that existed before
            # this field naming exactly what it named.
            "symbol_suffix": str(a.get("symbol_suffix", "")),
            "runs": [str(r) for r in (a.get("runs") or [])],
        })
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--file", default=DEFAULT)
    ap.add_argument("--id", default=None, help="one account by id, enabled or not")
    ap.add_argument("--all", action="store_true", help="include disabled accounts")
    args = ap.parse_args()

    accounts = load(args.file)
    if args.id:
        picked = [a for a in accounts if a["id"] == args.id]
        if not picked:
            sys.exit(f"no account with id {args.id!r} in {args.file}; "
                     f"known: {', '.join(a['id'] for a in accounts) or '(none)'}")
        accounts = picked
    elif not args.all:
        accounts = [a for a in accounts if a["enabled"]]
    json.dump(accounts, sys.stdout)
    return 0


if __name__ == "__main__":
    sys.exit(main())
