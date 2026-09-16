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
