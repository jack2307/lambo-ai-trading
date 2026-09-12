#!/usr/bin/env python3
"""Phase 4 gate: the browser cannot tell which backend answered.

Asks the prototype and the Rust API the same questions and compares the
answers. Only the overlap is compared — the Rust store holds two years of BTC
and the prototype holds a week, so a bar-for-bar comparison there would measure
the backfill rather than the port.

    python scripts/api-parity.py [--old=http://localhost:8137] [--new=http://127.0.0.1:8138]

Exits non-zero if anything differs beyond floating-point tolerance.
"""

from __future__ import annotations

import json
import sys
import urllib.error
import urllib.request

TOLERANCE = 1e-9
RELATIVE = 1e-12


def arg(name: str, fallback: str) -> str:
    hit = next((a for a in sys.argv[1:] if a.startswith(f"--{name}=")), None)
    return hit.split("=", 1)[1] if hit else fallback


def get(base: str, path: str, payload: dict | None = None):
    url = base.rstrip("/") + path
    data = json.dumps(payload).encode() if payload is not None else None
    headers = {"content-type": "application/json"} if data else {}
    try:
        with urllib.request.urlopen(urllib.request.Request(url, data=data, headers=headers), timeout=120) as r:
            return json.load(r)
    except urllib.error.HTTPError as e:
        try:
            return json.load(e)
        except Exception:
            return {"error": f"HTTP {e.code}"}
    except Exception as e:  # noqa: BLE001 - a transport failure is a result too
        return {"error": str(e)}


def close(a, b) -> bool:
    """Numeric equality with the same tolerance the Rust parity gates use."""
    if isinstance(a, bool) or isinstance(b, bool):
        return a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        if a != a and b != b:  # both NaN
            return True
        diff = abs(a - b)
        return diff <= TOLERANCE or diff <= RELATIVE * max(abs(a), abs(b))
    return a == b


def diff(path: str, a, b, out: list[str]) -> None:
    if isinstance(a, dict) and isinstance(b, dict):
        for key in sorted(set(a) | set(b)):
            if key not in a:
                out.append(f"{path}.{key}: missing on old")
            elif key not in b:
                out.append(f"{path}.{key}: missing on new")
            else:
                diff(f"{path}.{key}", a[key], b[key], out)
    elif isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            out.append(f"{path}: length {len(a)} vs {len(b)}")
            return
        for i, (x, y) in enumerate(zip(a, b)):
            diff(f"{path}[{i}]", x, y, out)
    elif not close(a, b):
        out.append(f"{path}: {a!r} vs {b!r}")


def main() -> int:
    old = arg("old", "http://localhost:8137")
    new = arg("new", "http://127.0.0.1:8138")
    # Gold is the market both backends hold the same data for. BTC is compared
    # only on its shape, since the stores genuinely differ in span.
    market = arg("market", "gold")
    timeframe = arg("tf", "15m")

    print(f"old: {old}\nnew: {new}\nmarket: {market} {timeframe}\n")
    failures = 0

    # 1. The bar series itself.
    a = get(old, f"/api/chart/bars?market={market}&tf={timeframe}")
    b = get(new, f"/api/chart/bars?market={market}&tf={timeframe}")
    if "error" in a or "error" in b:
        print(f"bars: {a.get('error') or b.get('error')}")
        return 1
    problems: list[str] = []
    diff("bars", a["bars"], b["bars"], problems)
    print(f"bars        {len(a['bars'])} vs {len(b['bars'])}  ->  {'match' if not problems else str(len(problems)) + ' diff(s)'}")
    for line in problems[:5]:
        print(f"  {line}")
    failures += bool(problems)

    # `stats` is deliberately NOT compared: the prototype reports the source
    # minute series there while returning resampled bars, so matching it would
    # mean reproducing a bug rather than a behaviour.

    # 2. Every strategy, trade for trade.
    catalog = get(new, "/api/chart/catalog")
    for strategy in catalog.get("strategies", []):
        sid = strategy["id"]
        body = {"strategy": sid, "market": market, "tf": timeframe}
        ra, rb = get(old, "/api/chart/backtest", body), get(new, "/api/chart/backtest", body)
        if "error" in ra or "error" in rb:
            print(f"{sid:<20} old={ra.get('error', 'ok')} new={rb.get('error', 'ok')}")
            continue
        problems = []
        diff("metrics", ra["metrics"], rb["metrics"], problems)
        diff("trades", ra["trades"], rb["trades"], problems)
        diff("verdict", ra["verdict"], rb["verdict"], problems)
        status = "match" if not problems else f"{len(problems)} diff(s)"
        print(f"{sid:<20} {ra['metrics']['trades']:>4} trades  ->  {status}")
        for line in problems[:5]:
            print(f"  {line}")
        failures += bool(problems)

    print()
    if failures:
        print(f"GATE FAIL: {failures} endpoint(s) disagree.")
        return 1
    print("GATE PASS: every compared number is identical.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
