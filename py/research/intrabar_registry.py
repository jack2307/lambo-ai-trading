"""C1 of docs/decisions/2026-10-07-intrabar-resolution.md: the registry, read
for the stop each mechanism actually places.

Reads `builtin::register_all` for the order, then each `impl Strategy for X`
block for `default_params()`, `grid()` and how `on_bar` forms its stop. A
strategy that calls `atr_stop(..., p.get("stopAtr"))` DECLARES an ATR multiple;
one that hands the engine a price level (a swing, a range edge, a previous-day
level) is STRUCTURAL and its stop in ATR is only knowable by measurement.
"""
import glob
import json
import os
import re
import sys
from collections import defaultdict

ROOT = "E:/rust/fd-intrabar"
STOPS_READABLE_FROM = 0.60   # the threshold this axis measured

# Parameters whose name contains "stop" but whose UNIT is not an ATR multiple,
# so they cannot be read against a table in ATR. Kept and printed, never binned.
NOT_IN_ATR = {
    "stopMode": "a switch (0 = structural channel edge, 1 = stopAtr x ATR)",
    "stopGapMult": "a multiple of the GAP, not of ATR",
    "stopPrice": "a distance in price points",
    "minStopPoints": "a floor in price points",
}


def impl_blocks():
    out = {}
    for f in glob.glob(f"{ROOT}/crates/fd-strategy/src/*.rs"):
        text = open(f, encoding="utf-8").read()
        for m in re.finditer(r"impl Strategy for (\w+)\s*\{", text):
            i = m.end() - 1
            depth = 0
            for j in range(i, len(text)):
                if text[j] == "{":
                    depth += 1
                elif text[j] == "}":
                    depth -= 1
                    if depth == 0:
                        out[m.group(1)] = (os.path.basename(f), text[i:j + 1])
                        break
    return out


def fn_body(block, name):
    m = re.search(rf"fn {name}\(&self[^)]*\)[^{{]*\{{", block)
    if not m:
        return ""
    i = m.end() - 1
    depth = 0
    for j in range(i, len(block)):
        if block[j] == "{":
            depth += 1
        elif block[j] == "}":
            depth -= 1
            if depth == 0:
                return block[i:j + 1]
    return ""


def main():
    reg = open(f"{ROOT}/crates/fd-strategy/src/builtin.rs", encoding="utf-8").read()
    order = re.findall(r"registry\.register\(Box::new\((?:crate::\w+::)?(\w+)\)\)", reg)
    blocks = impl_blocks()

    rows = []
    for name in order:
        f, b = blocks.get(name, ("?", ""))
        sid = re.search(r"fn id\(&self\)[^\"]*\"([\w-]+)\"", b)
        sid = sid.group(1) if sid else "?"
        d = fn_body(b, "default_params")
        g = fn_body(b, "grid")
        ob = fn_body(b, "on_bar")
        exits = re.search(r"fn exits\(&self[^)]*\)[^{]*\{\s*Exits::(\w+)", b)
        declared = dict(re.findall(r"\(\"(\w*[Ss]top\w*)\",\s*([-\d._]+)\)", d))
        gridded = {k: [v.strip() for v in vals.split(",") if v.strip()]
                   for k, vals in re.findall(
                       r"\(\"(\w*[Ss]top\w*)\"(?:\.to_string\(\))?,\s*&?(?:vec!)?\[([^\]]*)\]", g)}
        uses_atr_stop = "atr_stop(" in ob or "atr_stop(" in b
        hands_level = bool(re.search(r"stop:\s*Some\(", ob)) or "stop_price" in ob
        rows.append(dict(id=sid, struct=name, file=f,
                         declared=declared, grid=gridded,
                         exits=(exits.group(1) if exits else "Engine (default)"),
                         atr_stop_call=uses_atr_stop, level_in_on_bar=hands_level))

    print(f"`builtin::register_all` registers {len(order)} strategies.\n")
    w = max(len(r["id"]) for r in rows)
    print(f"{'id':<{w}}  {'stop it places':<34} {'declared stopAtr':<22} "
          f"{'stopAtr grid':<22} exits")
    print("-" * 132)
    declared_atr, structural, no_stop = [], [], []
    for r in rows:
        dec = ", ".join(f"{k}={v}" for k, v in r["declared"].items()) or "-"
        gr = ", ".join(f"{k}=[{'/'.join(v)}]" for k, v in r["grid"].items()) or "-"
        if r["declared"].get("stopAtr") or r["grid"].get("stopAtr"):
            kind = "ATR multiple (declared)"
            declared_atr.append(r)
        elif r["level_in_on_bar"] or r["atr_stop_call"]:
            kind = "price level (STRUCTURAL)"
            structural.append(r)
        else:
            kind = "none / engine fallback 1.2 ATR"
            no_stop.append(r)
        print(f"{r['id']:<{w}}  {kind:<34} {dec:<22} {gr:<22} {r['exits']}")

    print(f"\ndeclares an ATR stop: {len(declared_atr)}  "
          f"structural (stop is a price level): {len(structural)}  "
          f"no stop of its own: {len(no_stop)}")

    # Every declared ATR value anywhere in the registry, against the threshold.
    vals = defaultdict(set)
    other_units = defaultdict(set)
    for r in rows:
        for k, v in r["declared"].items():
            (other_units if k in NOT_IN_ATR else vals)[(k, float(v)) if k in NOT_IN_ATR
                                                       else float(v)].add(r["id"])
        for k, vs in r["grid"].items():
            for v in vs:
                (other_units if k in NOT_IN_ATR else vals)[(k, float(v)) if k in NOT_IN_ATR
                                                           else float(v)].add(r["id"])
    print(f"\n{'='*132}")
    print("EVERY DECLARED STOP VALUE IN THE REGISTRY, AGAINST THE 0.600 ATR "
          "THRESHOLD THIS AXIS MEASURED")
    print(f"{'='*132}")
    below = []
    for v in sorted(vals):
        tag = "BELOW THRESHOLD" if v < STOPS_READABLE_FROM else "readable"
        if v < STOPS_READABLE_FROM:
            below.extend(vals[v])
        print(f"{v:>6.2f} ATR  {tag:<16} {', '.join(sorted(vals[v]))}")
    print(f"\nregistered mechanisms whose DECLARED ATR stop (default or grid) is "
          f"below 0.600 ATR: {len(set(below))}"
          f"{' -> ' + ', '.join(sorted(set(below))) if below else ' (NONE)'}")
    print("\nstop parameters in the registry that are NOT in ATR and so cannot "
          "be binned against this table:")
    for (k, v), ids in sorted(other_units.items()):
        print(f"  {k} = {v:<6} {NOT_IN_ATR[k]:<48} {', '.join(sorted(ids))}")

    # The engine's own fallback, used when a strategy hands no stop at all.
    cfg = open(f"{ROOT}/config/default.toml", encoding="utf-8").read()
    m = re.search(r"^stop_atr = ([\d.]+)", cfg, re.M)
    print(f"\n[trading] stop_atr fallback (used when a strategy gives no stop "
          f"and the engine derives one): {m.group(1)} ATR -> readable")

    # Merge in what the published record MEASURED for each base, because a
    # structural stop has no declared multiple and only a measurement can
    # place it on the table. `null` where nothing measured it.
    try:
        pub = json.load(open(f"{ROOT}/receipts/intrabar-published-rows.json"))
    except FileNotFoundError:
        pub = []
    meas = defaultdict(list)
    for p in pub:
        meas[p["base"]].append(p["stop_atr"])
    print(f"\n{'='*132}")
    print("EACH REGISTERED MECHANISM PLACED ON THE TABLE BY ITS REALISED STOP "
          "(median over every published row of it); `null` = never measured")
    print(f"{'='*132}")
    nmeas = nnull = nbelow = 0
    names_below = []
    for r in rows:
        v = sorted(meas.get(r["id"], []))
        if not v:
            print(f"{r['id']:<{w}}  null  - no published row carries a realised "
                  f"stop for it")
            nnull += 1
            continue
        nmeas += 1
        med = v[len(v) // 2]
        band = ("NOT EVIDENCE" if med < 0.30 else
                "caveat" if med < 0.60 else "readable")
        if med < 0.60:
            nbelow += 1
            names_below.append(r["id"])
        print(f"{r['id']:<{w}}  median {med:>6.3f} ATR  (min {v[0]:.3f} max "
              f"{v[-1]:.3f}, {len(v)} rows)  -> {band}")
    extra = sorted(set(meas) - {r["id"] for r in rows})
    for b in extra:
        v = sorted(meas[b])
        med = v[len(v) // 2]
        band = ("NOT EVIDENCE" if med < 0.30 else
                "caveat" if med < 0.60 else "readable")
        print(f"{b:<{w}}  median {med:>6.3f} ATR  ({len(v)} rows)  -> {band}   "
              f"** in the record but NOT in register_all on this branch **")
    print(f"\nregistered mechanisms with a measured stop: {nmeas}; never "
          f"measured: {nnull}; measured BELOW 0.600 ATR: {nbelow}"
          f"{' -> ' + ', '.join(names_below) if names_below else ''}")


if __name__ == "__main__":
    sys.exit(main())
