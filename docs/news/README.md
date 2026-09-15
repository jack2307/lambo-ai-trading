# Scheduled high-impact news calendar

`data/news/events.csv` and `data/news/events.parquet` hold the release times of
the scheduled events the research loop and the paper bot treat as news:
FOMC decisions, US CPI, US Employment Situation (NFP), ECB decisions, and a
live layer of this week's ForexFactory High/Medium events. Collected
2026-09-14. Rebuild and refresh with `py/ingest/news_calendar.py`.

## Files

| Path | What |
|---|---|
| `data/news/events.csv` | `time_utc,currency,name,impact,source`; ISO-8601 `Z` times; sorted; unique on (time, currency, name) |
| `data/news/events.parquet` | same rows: `time` timestamp[ms, UTC], `currency` string, `name` string, `impact` int8, `source` string; zstd |
| `data/news/raw/fed-YYYY.txt` | Fed calendar rows as extracted (month, days, type, statement URL, release header) |
| `data/news/raw/bls-YYYY.txt` | BLS schedule rows (release, reference month, date, time) |
| `data/news/raw/ecb-YYYY.txt` | ECB decision dates as listed |
| `data/news/raw/forexfactory-<week>-<date>.json` | the feed as downloaded |
| `py/ingest/news_calendar.py` | `--from-raw` (raw -> csv -> parquet), `--build` (csv -> parquet), `--refresh` (ForexFactory merge) |

`impact`: 3 high, 2 medium, 1 low. Everything historical is 3. Only the
ForexFactory layer carries 2 (Low rows are dropped).

## Sources and times

All local times go through the IANA database (`zoneinfo`, `dateutil`
fallback); no fixed offsets. New York summer = UTC-4, winter = UTC-5;
Berlin summer = UTC+2, winter = UTC+1.

### FOMC (`source=federalreserve`, currency USD)

- 2021-2027: <https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm>
- 2010-2020: <https://www.federalreserve.gov/monetarypolicy/fomchistorical{YEAR}.htm>
- Scheduled meetings: `FOMC`, 14:00 America/New_York on the last meeting
  day (18:00 UTC summer, 19:00 UTC winter). The pages give no time; 14:00 is
  the convention the brief fixed. **Flag:** before the March 2013 meeting the
  statement actually came out at 14:15 ET (and 12:30 ET on the press-conference
  days April 2011 - January 2013). The Fed's current press-release pages only
  say "For immediate release" for those years, so this could not be verified
  from source here; the file uses 14:00 for every scheduled meeting.
  A consumer wanting a blackout window should open it a few minutes earlier
  than 14:00 for 2010-2012.
- Unscheduled entries: `FOMC (unscheduled)`, included only when the Fed page
  links a statement/press release for the entry. The time is the "For release
  at ..." header of that press release (fetched 2026-09-14, verbatim in the
  raw file):

  | UTC | What |
  |---|---|
  | 2010-05-10 01:15 | May 9 conference call, swap lines re-established (9:15 p.m. EDT Sunday; Monday in UTC) |
  | 2019-10-11 15:00 | Oct 4 conference call; statement on reserve management purchases released Oct 11, 11:00 a.m. EDT |
  | 2020-03-03 15:00 | Mar 2 unscheduled meeting; 50 bp cut released Mar 3, 10:00 a.m. EST |
  | 2020-03-15 21:00 | unscheduled meeting, 100 bp cut, 5:00 p.m. EDT **Sunday** (the only weekend row) |
  | 2020-03-19 13:00 | notation vote, temporary swap lines with nine central banks, 9:00 a.m. EDT |
  | 2020-03-23 12:00 | notation vote, FOMC statement (unlimited purchases), 8:00 a.m. EDT |
  | 2020-03-31 12:30 | notation vote, FIMA repo facility, 8:30 a.m. EDT |
  | 2020-08-27 13:10 | notation vote, framework statement (average inflation targeting), 9:10 a.m. EDT |
  | 2025-08-22 14:00 | notation vote, framework statement update, 10:00 a.m. EDT |

- Not included (the Fed page lists them with **no statement**): conference
  calls 2010-10-15, 2011-08-01, 2011-11-28, 2013-10-16, 2014-03-04; the
  cancelled 2020-03-17/18 meeting. The 2011-11-30 coordinated swap-line
  action is not on the FOMC calendar as a statement and is not here either.
- 2020 therefore has 7 scheduled + 6 unscheduled rows.

### US CPI and Employment Situation (`source=bls`, currency USD)

- 2010-2026: <https://www.bls.gov/schedule/{YEAR}/home.htm> (the yearly
  schedule; 2026 exists already). Cross-checked 2026 against
  <https://www.bls.gov/schedule/news_release/cpi.htm> and
  <https://www.bls.gov/schedule/news_release/empsit.htm> (identical dates).
- Names: `US CPI` (headline "Consumer Price Index" rows only; no CPI-U
  detailed report, no Real Earnings) and `US Employment Situation (NFP)`.
- 08:30 America/New_York (12:30 UTC summer, 13:30 UTC winter). Every row on
  every page printed 08:30 AM explicitly; nothing was defaulted.
- Deviations from 12 + 12 per year:
  - **2013** (October shutdown): 12 + 12 but September CPI moved to
    Oct 30 and September Employment Situation to Oct 22.
  - **2025** (October-November lapse in appropriations): **11 CPI, 11
    Employment Situation**. September CPI moved to Oct 24, September
    Employment Situation to Nov 20; there is no October-2025 row of either
    kind on the BLS page (November is the next reference month: CPI Dec 18,
    Employment Situation Dec 16).
- **Gap:** no 2027 BLS schedule exists yet, so 2027 has no CPI/NFP rows.

### ECB (`source=ecb`, currency EUR, name `ECB rate decision`)

- 2010-2023: press releases titled "Monetary policy decisions" from
  <https://www.ecb.europa.eu/press/pr/date/{YEAR}/html/index_include.en.html>
  (the fragment the ECB site lazy-loads; the visible index page carries no
  list).
- 2024-2026: that fragment returned one unrelated row for 2024 and 404 for
  2025/2026, so the dates come from the monetary policy statement lists
  <https://www.ecb.europa.eu/press/press_conference/monetary-policy-statement/{YEAR}/html/index_include.en.html>
  (the statement is read at the press conference on the decision day).
- 2026-10-29, 2026-12-17 and all of 2027: day 2 of the "Governing Council
  monetary policy meeting" entries on
  <https://www.ecb.europa.eu/press/calendars/mgcgc/html/index.en.html>.
  That page currently covers Oct 2026 - Dec 2028 only; the December 2027
  meeting is not on it yet, so **2027 has 7 ECB rows**. The 2028 rows were
  not taken (outside the 2027-12-31 horizon).
- Could not get: <https://www.ecb.europa.eu/press/govcdec/mopo/html/index.en.html>
  is a landing page whose list is client-rendered (no dated entries, no
  usable include URL), and its 2024-2026 press-release fragments are gone.
- Times: 13:45 Europe/Berlin through 2022-06-09 and 14:15 from 2022-07-21
  (the ECB's own pages state 14:15 CET today; the 13:45 -> 14:15 change with
  the July 2022 meeting is from the ECB's 2022 announcement and is applied by
  date in the script, not read from a page here). No ECB page lists a time
  per meeting.
- 12 decisions a year 2010-2014 (monthly), 8 a year from 2015 (six-week
  cycle). 2026-02-05 is on the ECB statement list as a monetary policy
  statement and is kept although one search snippet called it non-monetary.
- Not included: unscheduled ECB actions (2020-03-18 PEPP announcement late
  evening, 2022-06-15 ad hoc meeting statement) and the 14:45 press
  conference.

### ForexFactory live layer (`source=forexfactory`)

- <https://nfs.faireconomy.media/ff_calendar_thisweek.json> and
  `ff_calendar_nextweek.json`; fields title, country, date (with offset),
  impact. High -> 3, Medium -> 2, Low -> dropped. `currency` is the feed's
  `country` code (may be `All`).
- 2026-09-14: this-week feed fetched (103 items, 2026-09-13 .. 09-19; 16
  High + 10 Medium = 26 rows kept). **The next-week feed returned 404 on
  every attempt** (json and xml); nothing from next week is in the file.
  Re-run `--refresh` later in the week.
- The live layer overlaps the scheduled rows in the current week under
  different names: 2026-09-16 18:00 UTC has both `FOMC` (federalreserve)
  and `FOMC Statement` / `Federal Funds Rate` / `FOMC Economic Projections`
  (forexfactory). Consumers that build blackout windows should key on time,
  not name.

## Counts per year

| year | FOMC | FOMC (unsched.) | US CPI | US NFP | ECB | forexfactory |
|---|---|---|---|---|---|---|
| 2010 | 8 | 1 | 12 | 12 | 12 | |
| 2011 | 8 | | 12 | 12 | 12 | |
| 2012 | 8 | | 12 | 12 | 12 | |
| 2013 | 8 | | 12 | 12 | 12 | |
| 2014 | 8 | | 12 | 12 | 12 | |
| 2015 | 8 | | 12 | 12 | 8 | |
| 2016 | 8 | | 12 | 12 | 8 | |
| 2017 | 8 | | 12 | 12 | 8 | |
| 2018 | 8 | | 12 | 12 | 8 | |
| 2019 | 8 | 1 | 12 | 12 | 8 | |
| 2020 | 7 | 6 | 12 | 12 | 8 | |
| 2021 | 8 | | 12 | 12 | 8 | |
| 2022 | 8 | | 12 | 12 | 8 | |
| 2023 | 8 | | 12 | 12 | 8 | |
| 2024 | 8 | | 12 | 12 | 8 | |
| 2025 | 8 | 1 | 11 | 11 | 8 | |
| 2026 | 8 | | 12 | 12 | 8 | 26 |
| 2027 | 8 | | | | 7 | |

Total 747 rows. First row 2010-01-08 13:30 UTC `US Employment Situation
(NFP)`; last row 2027-12-08 19:00 UTC `FOMC`. Read back with pyarrow: sorted,
no duplicate keys, one weekend row (2020-03-15 21:00 UTC, Sunday, genuine).

## What is not in the file

- No medium-impact history: impact 2 exists only for the current
  ForexFactory week. No non-USD/EUR history at all (BoE, BoJ, RBA ... only
  appear in the live week).
- No data revisions, no actual/forecast/previous values, no outcome of any
  decision.
- No FOMC minutes, press conferences, SEP, speeches; no ECB press conference
  or accounts; no US PPI, retail sales, GDP, PCE, ISM, jobless claims
  (except within the live week).
- 2027 has FOMC and ECB only (no BLS schedule yet; ECB December 2027 not
  yet announced).
- Times are scheduled release times, not the moment the first print hit
  the tape. Pre-March-2013 FOMC statement times are 14:00 by convention,
  see the flag above.

## Refreshing

```
python py/ingest/news_calendar.py --refresh     # weekly: ForexFactory this/next week -> csv -> parquet
python py/ingest/news_calendar.py --from-raw    # after editing anything in data/news/raw
```

`--refresh` only adds rows (key time + currency + name); when ForexFactory
moves an event the old row stays. When the Fed, BLS or ECB publish the next
year, fetch the page by hand, add a `raw/<source>-<year>.txt` in the same
pipe format, and run `--from-raw`.

## The extended calendar (2026-09-15)

`data/news/events-extended.csv|parquet` is **the research calendar**:
everything in `events.csv`, plus two sources the live calendar deliberately
does not carry.

| source | release | currency | time |
|---|---|---|---|
| `statcan-lfs` | Canada Labour Force Survey | CAD | from the raw file — StatCan moved it from 07:00 to 08:30 Eastern partway through the span, so the hour is per-row and never defaulted |
| `bea-personal-income` | US Personal Income and Outlays (PCE) | USD | 08:30 Eastern |

Build it with `python py/ingest/news_calendar.py --extended`. That command
**never writes `events.csv` or `events.parquet`**, and `--from-raw` still
parses only the Fed, BLS and ECB files, so neither of the new sources can
reach the live calendar by accident.

**Why two calendars.** `fd-api` and `search` load `events.parquet` at startup
and the ten paper books enforce a news blackout from it — 60 minutes before a
high-impact release to 30 after. Adding twelve US personal-income dates a year
would widen that blackout by about eighteen hours a year for every running
book, in the middle of the two-week observation those books exist to produce.
A research question is not a reason to change what a running bot does. Moving
a source into the live calendar is its own decision, taken deliberately, with
the books restarted on purpose.

**Why these two.** `docs/decisions/2026-09-14-nfp-vs-first-friday.md` closed
because eleven of its thirty-two control Fridays turned out to carry an 08:30
New York macro print that the five-name calendar could not see — Canada's
Labour Force Survey on five of them and US Personal Income/PCE on four. The
successor question, whether gold's 07:30 → 08:30 hour falls before **any**
08:30 release rather than before one particular one, cannot be asked until
those dates exist. These two sources are that prerequisite and nothing more.

Provenance is the same standard as the rest of this directory: every date
comes from a page that was read, the raw files carry the URL and the fetch
date in their header, and a release whose time could not be established is
written `UNKNOWN` and dropped by the parser with a count, never guessed.


## The calendar runs out, and the desk now says when

Checked 2026-09-15. The scheduled layer does not end all at once:

| series | last entry on file |
|---|---|
| US Employment Situation (NFP) | **2026-12-04** |
| US CPI | **2026-12-10** |
| ECB rate decision | 2027-10-28 |
| FOMC | 2027-12-08 |

The day after a series' last entry, the news guard stops blacking out that
release — without failing, without logging, and without anybody noticing. So
`/api/paper/status` now carries `news.horizon`, `horizon_days` and
`horizon_name`, and the Desk's top strip shows **`US Employment Situation
(NFP) ends in 80d`** in amber from ninety days out and in red inside thirty.

Two things that number gets right, both found by looking at what it printed:

* **It is the first series to expire, not the last event on the file.** The
  obvious version reported 449 days, because the Fed publishes its meeting
  dates two years ahead — a long series masking a short one, while CPI and the
  employment report were eighty-six days from going unguarded.
* **Only a recurring series has a horizon.** The live ForexFactory layer is one
  week of whatever was on the wire, so every one of its names "runs out" in a
  few days by design. Its second version reported `FOMC Economic Projections
  ends in 1d`, for ever. A series needs twelve entries to count, which
  separates the four scheduled ones (143–203 entries) from the weekly feed
  (one or two) and from `FOMC (unscheduled)` (nine, and not a schedule).

**2027 cannot be collected yet.** `data/news/raw/bls-2027.txt` exists with a
documented header and **zero rows**: BLS has published no 2027 schedule as of
2026-09-15. `schedule/2027/home.htm` is a genuine 404, `schedule/news_release/`
links 2026 only, and both `cpi.htm` and `empsit.htm` stop at reference month
November 2026. The BLS rule (the employment report on the third Friday after
the reference week) may *check* a list and never generate one, so the file is
empty on purpose and the gap stays visible. BLS posts the next year in the
autumn of the preceding one — re-check from October 2026.
