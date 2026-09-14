"""Scheduled high-impact news calendar: data/news/events.csv -> events.parquet.

    python py/ingest/news_calendar.py --from-raw   # re-parse data/news/raw/*.txt|json into events.csv, then build
    python py/ingest/news_calendar.py --build      # rebuild events.parquet from events.csv
    python py/ingest/news_calendar.py --refresh    # fetch ForexFactory this/next week, merge High+Medium, build

The historical layer (FOMC 2010-2027, US CPI / Employment Situation 2010-2026,
ECB decisions 2010-2027) was collected by hand on 2026-09-14 with WebFetch
against the Fed, BLS and ECB pages, because BLS returns 403 to scripts and the
ECB lists are lazy-loaded. Those extractions live in data/news/raw/ as small
pipe-delimited text files, and --from-raw re-parses them; this script never
fetches the Fed, BLS or ECB itself. The only network call is --refresh, which
reads the two public ForexFactory JSON feeds (this week / next week) and keeps
rows with impact High (3) or Medium (2). Rows are keyed on (time, currency,
name); a refresh never removes rows.

Times: FOMC 14:00 America/New_York on the last day of the meeting unless the
raw file carries a "For release at ..." header (unscheduled statements); BLS
08:30 America/New_York; ECB 13:45 Europe/Berlin until 2022-07-20 and 14:15
from 2022-07-21; ForexFactory times carry their own offset. All conversions go
through the IANA database (zoneinfo, dateutil fallback), never a fixed offset.

Schema of events.parquet: time timestamp[ms, UTC], currency string, name string,
impact int8 (3 high, 2 medium, 1 low), source string. See docs/news/README.md.
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import glob
import json
import os
import re
import sys
import urllib.request

import pyarrow as pa
import pyarrow.parquet as pq

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
NEWS = os.path.join(ROOT, "data", "news")
RAW = os.path.join(NEWS, "raw")
CSV_PATH = os.path.join(NEWS, "events.csv")
PARQUET_PATH = os.path.join(NEWS, "events.parquet")
HEADER = ["time_utc", "currency", "name", "impact", "source"]

FF_FEEDS = {
    "thisweek": "https://nfs.faireconomy.media/ff_calendar_thisweek.json",
    "nextweek": "https://nfs.faireconomy.media/ff_calendar_nextweek.json",
}
FF_IMPACT = {"High": 3, "Medium": 2, "Low": 1}

MONTHS = {m: i for i, m in enumerate(
    ["january", "february", "march", "april", "may", "june", "july",
     "august", "september", "october", "november", "december"], 1)}


def zone(name: str):
    try:
        from zoneinfo import ZoneInfo
        return ZoneInfo(name)
    except Exception:  # Windows without the tzdata wheel
        from dateutil import tz
        z = tz.gettz(name)
        if z is None:
            raise RuntimeError(f"no tz database for {name}; pip install tzdata")
        return z


NY = zone("America/New_York")
BERLIN = zone("Europe/Berlin")
UTC = dt.timezone.utc


def to_utc(local: dt.datetime, z) -> dt.datetime:
    return local.replace(tzinfo=z).astimezone(UTC)


def iso(t: dt.datetime) -> str:
    return t.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


def parse_iso(s: str) -> dt.datetime:
    return dt.datetime.strptime(s, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=UTC)


def raw_lines(path: str):
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#"):
                yield [c.strip() for c in line.split("|")]


# ----------------------------------------------------------------------------- Fed
RELEASE_RE = re.compile(
    r"release at (\d{1,2}):(\d{2}) ([ap])\.m\. E[DS]T ([A-Za-z]+) (\d{1,2}), (\d{4})")


def parse_fed(path: str, year: int):
    rows = []
    for month, days, kind, statement, release in raw_lines(path):
        kind_l = kind.lower()
        if "cancel" in kind_l or statement.lower() == "no statement":
            continue  # cancelled meeting, or a conference call that published nothing
        scheduled = kind_l == "meeting"
        if scheduled:
            m = MONTHS[month.split("/")[0].lower()]
            parts = [int(d) for d in days.split("-")]
            last = parts[-1]
            if len(parts) == 2 and parts[1] < parts[0]:  # "31-1": spills into next month
                m += 1
            local = dt.datetime(year, m, last, 14, 0)
            rows.append((to_utc(local, NY), "USD", "FOMC", 3, "federalreserve"))
            continue
        mt = RELEASE_RE.search(release)
        if mt:
            hh, mm, ap, mon, day, yr = mt.groups()
            hh = int(hh) % 12 + (12 if ap == "p" else 0)
            local = dt.datetime(int(yr), MONTHS[mon.lower()], int(day), hh, int(mm))
        else:  # no header fetched: statement date from the URL, default time
            md = re.search(r"monetary(\d{4})(\d{2})(\d{2})", statement)
            if not md:
                raise ValueError(f"{path}: cannot date unscheduled entry {month} {days}")
            local = dt.datetime(int(md[1]), int(md[2]), int(md[3]), 14, 0)
        rows.append((to_utc(local, NY), "USD", "FOMC (unscheduled)", 3, "federalreserve"))
    return rows


# ----------------------------------------------------------------------------- BLS
def parse_bls(path: str, year: int):
    rows = []
    for name, _ref, date_s, time_s in raw_lines(path):
        date_s = date_s.split(",", 1)[1].strip() if "," in date_s.split(" ")[0] else date_s
        d = dt.datetime.strptime(date_s, "%B %d, %Y")
        t = dt.datetime.strptime(time_s, "%I:%M %p")
        local = d.replace(hour=t.hour, minute=t.minute)
        if name.startswith("Consumer Price Index"):
            label = "US CPI"
        elif name.startswith("Employment Situation"):
            label = "US Employment Situation (NFP)"
        else:
            raise ValueError(f"{path}: unexpected release {name!r}")
        rows.append((to_utc(local, NY), "USD", label, 3, "bls"))
    return rows


# ----------------------------------------------------------------------------- ECB
ECB_TIME_CHANGE = dt.date(2022, 7, 21)  # press release moved from 13:45 to 14:15 CET


def parse_ecb(path: str, year: int):
    rows = []
    for date_s, _title in raw_lines(path):
        d = dt.date.fromisoformat(date_s)
        hh, mm = (14, 15) if d >= ECB_TIME_CHANGE else (13, 45)
        local = dt.datetime(d.year, d.month, d.day, hh, mm)
        rows.append((to_utc(local, BERLIN), "EUR", "ECB rate decision", 3, "ecb"))
    return rows


# ----------------------------------------------------------------------------- ForexFactory
def ff_rows(items, min_impact: int = 2):
    rows = []
    for it in items:
        impact = FF_IMPACT.get(it.get("impact"), 0)
        if impact < min_impact:
            continue
        t = dt.datetime.fromisoformat(it["date"])  # carries its own offset
        if t.tzinfo is None:
            raise ValueError(f"forexfactory row without offset: {it}")
        rows.append((t.astimezone(UTC), it["country"], it["title"].strip(), impact, "forexfactory"))
    return rows


def fetch_ff(name: str) -> list | None:
    req = urllib.request.Request(FF_FEEDS[name], headers={"User-Agent": "Mozilla/5.0"})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            data = r.read()
    except Exception as e:  # the next-week feed is often 404 until mid-week
        print(f"forexfactory {name}: {e}")
        return None
    stamp = dt.datetime.now(UTC).strftime("%Y-%m-%d")
    with open(os.path.join(RAW, f"forexfactory-{name}-{stamp}.json"), "wb") as f:
        f.write(data)
    return json.loads(data)


# ----------------------------------------------------------------------------- CSV / parquet
def read_csv() -> dict:
    rows = {}
    if not os.path.exists(CSV_PATH):
        return rows
    with open(CSV_PATH, encoding="utf-8", newline="") as f:
        rd = csv.DictReader(f)
        assert rd.fieldnames == HEADER, rd.fieldnames
        for r in rd:
            key = (r["time_utc"], r["currency"], r["name"])
            rows[key] = (parse_iso(r["time_utc"]), r["currency"], r["name"], int(r["impact"]), r["source"])
    return rows


def write_csv(rows: dict) -> list:
    ordered = sorted(rows.values(), key=lambda r: (r[0], r[1], r[2]))
    with open(CSV_PATH, "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(HEADER)
        for t, cur, name, impact, source in ordered:
            w.writerow([iso(t), cur, name, impact, source])
    return ordered


def build_parquet() -> pa.Table:
    rows = sorted(read_csv().values(), key=lambda r: (r[0], r[1], r[2]))
    table = pa.table({
        "time": pa.array([r[0] for r in rows], pa.timestamp("ms", tz="UTC")),
        "currency": pa.array([r[1] for r in rows], pa.string()),
        "name": pa.array([r[2] for r in rows], pa.string()),
        "impact": pa.array([r[3] for r in rows], pa.int8()),
        "source": pa.array([r[4] for r in rows], pa.string()),
    })
    meta = {
        b"built_at": dt.datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ").encode(),
        b"built_from": b"data/news/events.csv",
        b"doc": b"docs/news/README.md",
    }
    table = table.replace_schema_metadata(meta)
    pq.write_table(table, PARQUET_PATH, compression="zstd", compression_level=3)
    return table


def merge(rows: dict, new_rows) -> int:
    added = 0
    for r in new_rows:
        key = (iso(r[0]), r[1], r[2])
        if key not in rows:
            rows[key] = r
            added += 1
    return added


def report(table: pa.Table) -> None:
    times = table.column("time").to_pylist()
    names = table.column("name").to_pylist()
    per = {}
    for t, n in zip(times, names):
        per.setdefault(t.year, {}).setdefault(n, 0)
        per[t.year][n] += 1
    for y in sorted(per):
        print(y, dict(sorted(per[y].items())))
    weekend = [(t, n) for t, n in zip(times, names) if t.weekday() >= 5]
    print(f"{table.num_rows} rows, {times[0]} .. {times[-1]}; weekend rows: {len(weekend)}")
    for t, n in weekend:
        print("  weekend:", t, n)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--from-raw", action="store_true", help="re-parse data/news/raw into events.csv, then build")
    ap.add_argument("--build", action="store_true", help="rebuild events.parquet from events.csv")
    ap.add_argument("--refresh", action="store_true", help="fetch ForexFactory feeds, merge, build")
    args = ap.parse_args()
    if not (args.from_raw or args.build or args.refresh):
        ap.print_help()
        return 2

    if args.from_raw:
        rows = {}
        parsers = {"fed": parse_fed, "bls": parse_bls, "ecb": parse_ecb}
        for path in sorted(glob.glob(os.path.join(RAW, "*-*.txt"))):
            src, year = os.path.basename(path)[:-4].rsplit("-", 1)
            if src in parsers:
                merge(rows, parsers[src](path, int(year)))
        for path in sorted(glob.glob(os.path.join(RAW, "forexfactory-*.json"))):
            with open(path, encoding="utf-8") as f:
                merge(rows, ff_rows(json.load(f)))
        ordered = write_csv(rows)
        print(f"events.csv: {len(ordered)} rows from raw")

    if args.refresh:
        rows = read_csv()
        for name in FF_FEEDS:
            items = fetch_ff(name)
            if items is not None:
                added = merge(rows, ff_rows(items))
                print(f"forexfactory {name}: {len(items)} items, {added} new High/Medium rows")
        write_csv(rows)

    table = build_parquet()
    report(pq.read_table(PARQUET_PATH))
    return 0


if __name__ == "__main__":
    sys.exit(main())
