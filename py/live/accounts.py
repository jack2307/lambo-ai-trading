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
    python py/live/accounts.py --prices        # the terminal the desk reads bars from

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


def prices(path):
    """The one key naming the terminal the desk's bars come from.

    Separate from `load` and from the account list on purpose: it is a fact
    about the DESK, not about an account, and the two questions have different
    answers. The funded account's terminal and the price terminal happen to be
    the same path today and are not the same thing — the first is where orders
    go, the second is where bars come from, and `config/accounts.toml` carries
    the argument for why they are currently one.

    Refuses rather than defaults. A missing or empty `terminal` here would
    otherwise let every caller fall back to "whatever terminal is running",
    which on a machine with two is a coin toss, and the losing side of that
    toss is a desk whose bars are a different contract from its fills.
    """
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
    table = doc.get("prices") or {}
    terminal = str(table.get("terminal") or "").strip()
    if not terminal:
        sys.exit(f"{path}: [prices] needs a `terminal` - the desk has no default price feed")
    # An absent suffix is a real answer (a standard account carries none), so
    # it defaults to empty where `terminal` refuses to.
    # `fill_on_open` is the owner's switch for the poller's `--fill-on-open`
    # (docs/decisions/2026-09-17-entry-lag.md). Absent means off, exactly as
    # the flag does, so an old registry keeps the old behaviour.
    return {
        "terminal": terminal,
        "symbol_suffix": str(table.get("symbol_suffix") or ""),
        "fill_on_open": bool(table.get("fill_on_open", False)),
        "weekend_flat": weekend_flat(path, table),
    }


def weekend_flat(path, table):
    """`[prices] weekend_flat` as a string to hand the executor, or "" for absent.

    The owner's rule of 2026-09-19 is that the account never holds a position
    over a weekend, and the executor's `--weekend-flat` is the layer that makes
    that true of the ACCOUNT rather than of the book
    (docs/decisions/2026-09-19-weekend-flat-never-fires.md). It used to live
    only in the executor's argparse default, which meant the November DST
    change - 20:45 becomes 21:45 - had to be typed in every launcher that might
    pass it and in the runbook that checks it, none of which can see each
    other. This key is the one place it now lives.

    ABSENT RETURNS "", MEANING "PASS NOTHING", so the executor keeps its own
    default and a registry written before this key existed behaves exactly as
    it did. Same rule `fill_on_open` follows, for the same reason.

    AN EXPLICIT EMPTY STRING IS ALSO ABSENT, not `off`. The two readings are
    ambiguous and only one of them is safe: read as `off`, a half-finished edit
    would silently disable the backstop, which is the failure this whole note
    is about. So disabling takes the word `off`, typed, and an empty value
    leaves the backstop on.

    IT REFUSES ON ANYTHING ELSE rather than falling back. A mistyped value
    quietly becoming 20:45 is exactly the class of failure the backstop exists
    to catch, and the launcher that calls this then stops before it has killed
    anything. Rule 3 of docs/decisions/2026-09-17-unit-carrying.md.

    This is a SECOND validator - `mt5_executor.weekend_cut_minute` is the
    authority and checks the value again when it is handed one. Deliberate, and
    the two are not redundant: this one refuses a bad REGISTRY before any
    executor is stopped, and that one refuses a bad COMMAND LINE, which is a
    path this file cannot see. If the accepted shape ever changes it changes in
    both, and py/live/size_guard_selftest.py pins the executor's half.
    """
    raw = table.get("weekend_flat", None)
    if raw is None:
        return ""
    if not isinstance(raw, str):
        sys.exit(f"{path}: [prices] weekend_flat must be a string like \"20:45\" or \"off\", "
                 f"not {type(raw).__name__} ({raw!r})")
    s = raw.strip()
    if s == "":
        return ""
    if s.lower() in ("off", "none"):
        return s.lower()
    parts = s.split(":")
    bad = f"{path}: [prices] weekend_flat = {raw!r} is not HH:MM (UTC) or \"off\""
    if len(parts) != 2:
        sys.exit(bad)
    try:
        hour, minute = int(parts[0]), int(parts[1])
    except ValueError:
        sys.exit(bad)
    if not (0 <= hour <= 23 and 0 <= minute <= 59):
        sys.exit(f"{path}: [prices] weekend_flat = {raw!r} is not a time of day")
    # Returned in the canonical HH:MM the executor prints and the runbook
    # compares against, so "8:5" and "08:05" cannot read as two settings in two
    # different logs.
    return "%02d:%02d" % (hour, minute)


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
            # Whether the launcher may hand this terminal its own
            # `config\\autologin.ini`. **Default FALSE, and the default is the
            # point.**
            #
            # `/portable` gives every install a `config` directory, and any
            # terminal ever configured through the GUI may have an autologin
            # file in it naming some login. Deriving "pass /config: if the file
            # exists" would hand that file to whichever terminals have one -
            # including, on this desk, the one carrying real money, which the
            # previous launcher never did. Either it names a different account
            # and the terminal comes up on the wrong login, or it names the
            # same one and forces a re-login on the terminal that is also the
            # price feed. Nobody has read those files.
            #
            # So it is opt-in per account. The launcher warns when a terminal
            # has an autologin it is NOT being told to use, which keeps the
            # thing worth keeping - a terminal silently starting unauthorised
            # is a real defect - without acting on a file no one has looked at.
            "autologin": bool(a.get("autologin", False)),
            "runs": [str(r) for r in (a.get("runs") or [])],
        })
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--file", default=DEFAULT)
    ap.add_argument("--id", default=None, help="one account by id, enabled or not")
    ap.add_argument("--all", action="store_true", help="include disabled accounts")
    ap.add_argument("--prices", action="store_true",
                    help="print the [prices] table instead of the accounts: the terminal the "
                         "pollers and the higher-timeframe export both read")
    args = ap.parse_args()

    if args.prices:
        # Printed alone rather than folded into the account list, so the
        # existing callers that parse that list as an array keep working.
        json.dump(prices(args.file), sys.stdout)
        return 0

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
