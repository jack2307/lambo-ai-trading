"""Telegram alerts for the paper desk, and a read-only bot behind an allowlist.

    python py/live/telegram_notify.py                 # watch and announce
    python py/live/telegram_notify.py --once --test   # send one probe and exit

Two jobs, one process:

* **It watches.** Polls `/api/paper/status`, `/api/paper/accounts` and each
  account's `/api/paper/broker-events` and announces what changed: a trade
  opened, a trade closed, a guard fired, a feed died, an account stopped
  mirroring a book, a terminal refused to send. Nothing here reaches a book —
  it reads the same routes the UI reads, and every one of them is a GET.
* **It answers.** Polls `getUpdates` and replies to `/status`, `/books`, `/ai`.
  **Only the allowlisted chat gets an answer.** Anything from another chat is
  refused and logged, and refused with a message that tells the sender nothing
  about this desk.

**It cannot trade.** Every call it makes is a GET. There is no command that
posts an intent, changes a guard, starts or stops a book, and there is no code
path here that could: the desk's write routes are never called. A bot token in
a group chat is a credential other people can reach, so the only safe amount of
authority to give it is none.

The token lives in `config/local.toml`, which is gitignored. It is never
printed, never passed on a command line (it would show in the process list),
and never included in an error message.
"""

from __future__ import annotations

import argparse
import datetime as dt
import html
import io
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

# Output is UTF-8, and undisplayable characters are replaced rather than fatal.
#
# Windows hands a redirected stdout the cp1252 codec, and the text below is not
# ours - it is whatever a model wrote in its reasoning, or whatever a broker put
# in a comment. On 2026-09-17 a single arrow in a DeepSeek stand-aside raised
# UnicodeEncodeError out of the print, out of main(), and killed the trader for
# five bars. `errors="replace"` is the important half: encoding can then never
# be the thing that stops a process, whatever arrives.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):  # not a TextIOWrapper; nothing to do
        pass

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
STATE = os.path.join(ROOT, "data", "paper", "telegram_state.json")
API = "https://api.telegram.org"

# How far behind its own timeframe a book may fall before it counts as dead.
#
# Two numbers, because the cost of missing this is not the same on both sides.
# A paper book that loses its feed loses a record. A book mirrored into the
# funded cent account can be holding a REAL position with nobody reconciling
# it: the book stops deciding, the executor has nothing new to copy, and the
# only thing still protecting the money is the stop and target the broker
# already holds. That is a worse thing to find out about late.
#
# The floor is structural, not a preference. `last_bar_time` is the bar's OPEN
# time and a bar is only posted once it CLOSES, so a perfectly healthy 15m book
# already sits between 15 and 30 minutes behind at all times. Two bars would
# therefore fire on every book on every bar. Three is the tightest setting that
# can mean anything, and it means two consecutive closes were missed — half an
# hour of dead feed.
#
# Paper stays looser than the desk's own `stale` badge (3 bars, Desk.tsx) for
# the original reason, which still holds there: the badge is read by someone
# already looking, an alert interrupts. Six bars was too loose even for that
# and is now four — 60 minutes on a 15m book, 20 on a 5m one.
#
# Real money is set EQUAL to the badge, which abandons that reasoning on
# purpose rather than by accident. Measured 2026-09-17: a terminal died and the
# feed was dead for about 60 minutes, under the old 90-minute threshold, and
# this watch said nothing about any of it. At three bars it would have said so
# 30 minutes in. The price of that is real — a poller restart that straddles
# two closes now sends one false alarm — and on an account carrying money it is
# the cheaper of the two mistakes.
STALE_BARS = 4
STALE_BARS_REAL = 3
TF_MS = {"1m": 60_000, "5m": 300_000, "15m": 900_000, "1h": 3_600_000}

# How many closed bars an AI book's decider may miss before it counts as gone.
#
# Tighter than the feed's six because the signal is cleaner: a model that
# DECLINES still posts, so `last_at` moves on every bar it sees. Silence means
# the call failed, not that the market was quiet. Three bars is 45 minutes on
# a 15m book — long enough to ride out one slow answer, short enough that
# 2026-09-16's 45-minute Codex token expiry would have been caught while it was
# still happening instead of afterwards.
DECIDER_STALE_BARS = 3

# How long a mirror may go quiet before it counts as stopped.
#
# An executor writes its snapshot every poll (15 s), so three missed writes is a
# dead process rather than a slow one. Absolute rather than relative to other
# books, unlike the API's own `mirroring` count: here the question is "has this
# stopped", and if the whole set died at once every one of them should say so.
MIRROR_STALE_MS = 60_000

# How many of an account's own executor events to read, and how far back to
# look the first time a book is seen.
#
# The events that matter here are all START-TIME refusals: no account_info, a
# login that is not the one named, a symbol the broker does not list,
# AutoTrading off. The executor writes one and exits 3, so twenty lines is
# several restarts' worth, not a window that can overflow in a busy minute.
#
# The freshness window exists for one case: the VPS reboots and brings the
# watch and the executors back together. `executor.jsonl` is append-only and
# survives, so the newest line in it may be from last week; announcing on
# first sight unconditionally would replay old history into the channel, and
# announcing nothing on first sight would swallow the refusal that just
# happened. Fifteen minutes covers the reboot and nothing else.
BROKER_EVENT_LIMIT = 20
BROKER_EVENT_FRESH_MS = 15 * 60_000

# The executor event kinds worth waking someone for.
#
# Deliberately not `refused-size` or `refused-margin`: those are the sizing
# rules working, they repeat on every bar a book wants too much, and they say
# nothing a person has to act on. `clipped`, `not-adopted` and `standing-out`
# are the same — the mirror declining a trade on purpose. What is here is the
# executor refusing to run at all.
ALARMING_KINDS = frozenset({"autotrading-off", "refused", "order-failed"})

# How long before a `stopped` event a failed close still belongs to it.
#
# The STOP path closes every position and then logs `stopped` in the same pass
# of the loop, so a refused close is written milliseconds earlier. A minute is
# far more than that needs and far less than the fifteen-second poll could put
# between two unrelated failures.
STOP_CLOSE_WINDOW_MS = 60_000

# How often a HEALTHY funded book is checked for the broker resizing it, and
# how many of its recent events must be clipped before that is worth saying.
#
# Clipping is a standing condition, not an event: it happens on a mirror that
# is working perfectly, so the gap-triggered fetch can never see it, and a
# per-trade alert would be noise on every trade of a book that is too small.
# Half an hour keeps a healthy desk at a handful of extra GETs an hour, and
# three occurrences is enough to separate "the minimum moved one odd trade"
# from "this book is sized wrong and has been for a while".
CLIP_CHECK_MS = 30 * 60_000
CLIP_MIN_EVENTS = 3

# Entry lag: how long after a fill's own bar the desk learned about it.
#
# `docs/decisions/2026-09-17-entry-lag.md` found that a paper book only reports
# a position once the bar has CLOSED, so the gap between the bar a fill is
# priced at and the wall clock the desk learned it is a full bar - about
# fifteen minutes on every entry today, and twenty-four times larger than the
# fill-price effect the investigation was commissioned to measure.
#
# It is MEASURED here and not assumed, because an engine change on `entry-lag`
# is meant to bring it to seconds and the watch should say what the number
# actually is on the day someone asks.
LAG_WINDOW_MS = 24 * 3_600_000
LAG_CHECK_MS = 15 * 60_000

# The alarm's threshold, in seconds, from `[alerts.watch] entry_lag_alarm_s` in
# config/local.toml. ABSENT MEANS OFF, and off is the shipped default.
#
# Off because today the lag is a full bar on every entry: switched on now it
# would fire on every trade the desk takes, and an alert that fires on every
# trade teaches its reader to swipe the channel away - which costs far more
# than this measurement is worth, because the same channel carries the funded
# account's mirror and kill-switch alerts.
#
# What turns it on: the deploy that ships the `entry-lag` engine change, with
# `entry_lag_alarm_s = 60`. At that point a lag above a minute means the new
# path is not running, which is a real thing to be told.
LAG_ALARM_DEFAULT_S = 0


# --------------------------------------------------------------- the secret

def credentials() -> tuple[str, str]:
    """Token and the one allowed chat, from `config/local.toml`."""
    path = os.path.join(ROOT, "config", "local.toml")
    try:
        text = io.open(path, encoding="utf-8").read()
    except OSError:
        sys.exit(f"no {path}; put [alerts.telegram] bot_token and chat_id there")
    token = re.search(r'bot_token\s*=\s*"([^"]+)"', text)
    chat = re.search(r'chat_id\s*=\s*"([^"]+)"', text)
    if not token or not chat or not token.group(1) or not chat.group(1):
        sys.exit("config/local.toml has no [alerts.telegram] bot_token / chat_id")
    return token.group(1), chat.group(1)


# ------------------------------------------------------------------- wire

def tg(token: str, method: str, params: dict, timeout: float = 30.0) -> dict:
    """One Telegram call. The token never appears in an exception message."""
    url = f"{API}/bot{token}/{method}"
    body = urllib.parse.urlencode(params).encode("utf-8")
    try:
        with urllib.request.urlopen(urllib.request.Request(url, data=body), timeout=timeout) as r:
            return json.loads(r.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace")[:200]
        raise RuntimeError(f"telegram {method}: HTTP {e.code} {detail}") from None
    except Exception as e:  # noqa: BLE001
        raise RuntimeError(f"telegram {method}: {type(e).__name__}") from None


def say(token: str, chat: str, text: str) -> None:
    tg(token, "sendMessage", {
        "chat_id": chat, "text": text, "parse_mode": "HTML",
        "disable_web_page_preview": "true",
    })


def desk(path: str, timeout: float = 20.0):
    with urllib.request.urlopen(f"http://127.0.0.1:8138{path}", timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


# ------------------------------------------------------------ market hours

def market_open(market: str, now_ms: int) -> bool:
    """Whether this market should be producing bars right now.

    Without this every gold book alerts as dead at 21:00 UTC each night and all
    weekend, and an alert that cries wolf nightly is one nobody reads by the
    second week. Vantage halts metals and FX 21:00-22:00 UTC daily (17:00 New
    York) and from Friday 21:00 UTC to Sunday 22:00 UTC. BTC does not halt —
    measured: zero gaps in fourteen days.
    """
    if market.startswith("btc"):
        return True
    t = dt.datetime.utcfromtimestamp(now_ms / 1000)
    wd, hour = t.weekday(), t.hour  # Mon=0 .. Sun=6
    if wd == 5:  # Saturday
        return False
    if wd == 4 and hour >= 21:  # Friday evening
        return False
    if wd == 6 and hour < 22:  # Sunday before the reopen
        return False
    return not 21 <= hour < 22  # the daily halt


# ----------------------------------------------------------------- state

def load_state() -> dict:
    try:
        return json.load(io.open(STATE, encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def save_state(state: dict) -> None:
    os.makedirs(os.path.dirname(STATE), exist_ok=True)
    tmp = STATE + ".tmp"
    with io.open(tmp, "w", encoding="utf-8") as f:
        json.dump(state, f)
    os.replace(tmp, STATE)


# ------------------------------------------------------------- formatting

def money(v: float) -> str:
    return ("+$%.2f" if v >= 0 else "-$%.2f") % abs(v)


def esc(s) -> str:
    return html.escape(str(s))


def snapshot(runs: list) -> str:
    net = sum(r.get("net_usd") or 0.0 for r in runs)
    now = time.time() * 1000
    stale = [r for r in runs if is_dead(r, now)]
    lines = [
        "<b>Backcom Desk</b>",
        f"{len(runs)} books · net <b>{money(net)}</b>",
        f"feeds dead: {len(stale)}" if stale else "feeds: all fed",
    ]
    ai = [r for r in runs if r.get("decider") and (r["decider"].get("last") != "coin")]
    for r in ai:
        d = r["decider"]
        trades = sum(d.get("decisions", {}).values())
        quiet = (now - (d.get("last_at") or now)) / 60000
        flag = " \U0001f507" if decider_gone(r, now) else ""
        lines.append(
            f"· <code>{esc(r['id'])}</code> {esc(d.get('last'))}{flag} — "
            f"{trades} trades, {d.get('stood_aside', 0)} stood aside, {money(r.get('net_usd') or 0.0)}"
            f", last spoke {quiet:.0f} min ago"
        )
    return "\n".join(lines)


def is_dead(run: dict, now_ms: float, bars: int = STALE_BARS) -> bool:
    """Whether this book's feed has stopped.

    `bars` is a parameter rather than the constant because `STALE_BARS_REAL`
    was added on 2026-09-17 with a measured justification and no caller: the
    constant and its comment promised a 45-minute alarm on a funded book while
    every book, funded or not, was still read at four bars. The outage that
    comment cites - a terminal dead for about 60 minutes on a 15m book - is
    exactly four bars, and the test is a strict `>`, so the change written to
    catch it would still have said nothing about it.
    """
    last = run.get("last_bar_time")
    if not last:
        return True
    limit = TF_MS.get(run.get("tf", ""), 900_000) * bars
    return (now_ms - last) > limit and market_open(run.get("market", ""), int(now_ms))


def funded_books(accounts: list) -> set:
    """Books an account is configured to mirror with nothing in the way of a real order.

    Read off `real_money` and NOT off `dry_run`. `dry_run` says what is true of
    the books reporting right now; `real_money` says what the registry permits.
    A funded book whose mirror happens to be dry this minute is the same book
    that carries money on the next bar, and a threshold that loosened itself
    whenever the mirrors were down would be loosest exactly when the desk was
    least watched.

    The cost is named rather than hidden: a book configured on the funded
    account but only ever run dry is now watched at three bars and will
    occasionally cry wolf on a poller restart that straddles two closes.
    """
    out: set = set()
    for a in accounts or []:
        if a.get("real_money"):
            out.update(a.get("runs") or [])
    return out


def lag_alarm_s() -> int:
    """The entry-lag threshold in seconds, or 0 for off.

    Read per call rather than cached at startup: turning this on should be a
    config edit and a restart of nothing, on the day the engine change ships.
    """
    try:
        text = io.open(os.path.join(ROOT, "config", "local.toml"), encoding="utf-8").read()
    except OSError:
        return LAG_ALARM_DEFAULT_S
    m = re.search(r"\[alerts\.watch\][^\[]*?entry_lag_alarm_s\s*=\s*(\d+)", text, re.S)
    return int(m.group(1)) if m else LAG_ALARM_DEFAULT_S


def lags(events: list, now_ms: float, window_ms: float = LAG_WINDOW_MS) -> list[float]:
    """Seconds between the bar each entry was PRICED at and when the desk learned it.

    Read from the `opened` lines of `fills.jsonl`, which the run-detail route
    serves - NOT from the status payload's copy. `OpenDto.learned_at` is
    cleared when the book goes flat, so a day's worth of entries cannot be read
    from it: by the time you ask, most of them are gone. The file keeps one
    line per entry whatever the book did afterwards.

    A line missing either clock is skipped rather than counted as zero. Zero is
    a meaningful value here - it is what the engine change is trying to reach -
    and inventing it would make the fix look finished.
    """
    out: list[float] = []
    for e in events or []:
        if e.get("kind") != "opened":
            continue
        t, learned = e.get("time"), e.get("learned_at")
        if not isinstance(t, (int, float)) or not isinstance(learned, (int, float)):
            continue
        if now_ms - float(learned) > window_ms:
            continue
        out.append((float(learned) - float(t)) / 1000.0)
    return out


def lag_line(run_id: str, seconds: list[float]) -> str:
    """One book's lag, as a sentence someone reads on a phone.

    Median and max rather than a mean: one restart that replayed a stale bar
    would drag a mean somewhere nobody could interpret, and the pair answers
    both questions a reader has - what it usually is, and what the worst was.
    """
    if not seconds:
        return f"· <code>{esc(run_id)}</code> — no entries in 24h"
    ordered = sorted(seconds)
    mid = ordered[len(ordered) // 2] if len(ordered) % 2 else \
        (ordered[len(ordered) // 2 - 1] + ordered[len(ordered) // 2]) / 2
    return (f"· <code>{esc(run_id)}</code> — {len(ordered)} entries, "
            f"median {mid:.0f}s, max {max(ordered):.0f}s")


def lag_alarms(run_id: str, events: list, state: dict, now_ms: float,
               threshold_s: int) -> list[str]:
    """One line per ENTRY that took longer than the threshold to be learned.

    Keyed on the entry's own bar time, so an entry is announced once however
    many polls see it, and a book that keeps being slow says so once per trade
    rather than once per fifteen seconds.

    Silent entirely when the threshold is 0, which is the shipped default - see
    LAG_ALARM_DEFAULT_S for why, and what turns it on.
    """
    if threshold_s <= 0:
        return []
    seen: dict = state.setdefault("lag_seen", {})
    out: list[str] = []
    for e in events or []:
        if e.get("kind") != "opened":
            continue
        t, learned = e.get("time"), e.get("learned_at")
        if not isinstance(t, (int, float)) or not isinstance(learned, (int, float)):
            continue
        gap = (float(learned) - float(t)) / 1000.0
        if gap <= threshold_s or now_ms - float(learned) > LAG_WINDOW_MS:
            continue
        key = f"{run_id}/{int(t)}"
        if seen.get(key):
            continue
        seen[key] = True
        out.append(
            f"⏱️ <b>{esc(run_id)}</b> — an entry priced at its bar was learned "
            f"{gap:.0f}s later, over the {threshold_s}s threshold."
        )
    return out


def drifts(run: dict, state: dict, now_ms: float) -> list[str]:
    """The account holding a different shape from the book, per account.

    `drift` is recomputed by the executor every poll from what is actually
    held, so unlike `blocked` it can never be stale - and unlike `blocked` it
    is NOT set by this path, so an alert hung on `blocked` would never see it.
    It arrives as the executor's own sentence, already naming tickets and lots,
    so it is quoted rather than summarised: "2 positions open on one book
    (tickets 51, 52)" sends someone to the right place and a count does not.

    WHAT THE MESSAGE HAS TO SAY, and it is not "go and fix this". The executor
    reports and deliberately does not correct, because two auto-correcting
    executors on one run id both close one position, see the account flat, and
    both open - an oscillation at the poll interval paying the spread twice a
    cycle. The drift also clears itself the next time the book goes flat, since
    that path closes everything of its magic. So the bound is ONE TRADE, and
    that bound is what stops this reading like an emergency. What is worth
    saying instead is the thing the bound implies: if it does NOT clear when
    the book next goes flat, something is running twice.

    Deduplicated on the sentence, not on a flag, so a drift that CHANGES shape
    - one extra position becoming two - is announced again while a steady one
    stays quiet.

    Tolerant of the field being absent: it is written to broker.json today but
    is not on the wire until `drift` is added to `BrokerDto`. Until then every
    broker reads None and this says nothing, which is the correct inert
    behaviour for an alert whose input has not arrived.
    """
    out: list[str] = []
    seen: dict = state.setdefault("drift", {})
    fresh = mirrors(run, now_ms)
    for b in fresh:
        acct = str(b.get("account") or "?")
        key = f"{acct}/{run.get('id')}"
        why = b.get("drift")
        why = str(why).strip() if why else ""
        before = seen.get(key) or ""
        if why == before:
            continue
        seen[key] = why
        if why:
            out.append(
                f"⚖️ <b>{esc(str(run.get('id')))}</b> on <b>{esc(acct)}</b> — "
                f"{esc(why)}. Reported, not corrected: it clears when the book next goes "
                f"flat. If it does not clear then, something is running twice."
            )
        elif before:
            out.append(
                f"✅ <b>{esc(str(run.get('id')))}</b> on <b>{esc(acct)}</b> — "
                f"the account matches the book again."
            )
    return out


def cause_of(decisions: list) -> str:
    """Why a decider stopped answering, in its own words, short enough to send.

    `ai_trader` does not post to the desk when a model is unreachable - that is
    deliberate and load-bearing, because posting on failure once destroyed the
    very silence `decider_gone` reads. But it DOES write the attempt to
    `decisions.jsonl` with `response` set to "ERROR: <Type>: <detail>", and
    `/api/paper/reasoning` serves that file. So the desk knows the cause even
    though nothing was posted, and an alert that did not quote it would be
    sending someone to find a line the desk was already holding.

    The exception TYPE is the part that decides what happens next -
    RuntimeError from a signed-out CLI is a `claude setup-token`, a timeout is
    a slow model worth ignoring - so it is kept whole and the detail is cut.
    """
    newest = None
    for d in (decisions or []):
        if newest is None or int(d.get("at") or 0) >= int(newest.get("at") or 0):
            newest = d
    if not newest:
        return ""
    resp = str(newest.get("response") or "")
    reason = str(newest.get("reason") or "")
    if resp.startswith("ERROR:"):
        return resp[len("ERROR:"):].strip()[:120]
    if "unreachable" in reason:
        tail = reason.rsplit(":", 1)[-1].strip()
        return tail[:120] or reason[:120]
    return ""


def undriven(run: dict, now_ms: float, bars: int = DECIDER_STALE_BARS) -> bool:
    """An `external` book with bars arriving and NOBODY driving it.

    `decider_gone` cannot see this case and never will: it returns False when
    `decider` is None, and the API writes that object only when an intent is
    accepted. So a book whose trader never started - a campaign left out of the
    launcher, a CLI that is not signed in, a process that died before its first
    post - is silent for ever, while a book whose trader started and then broke
    alerts in three bars. The desk could tell "this is broken" and could not
    tell "nobody is running this", which is the worse of the two to miss
    because nothing about it looks wrong.

    Measured 2026-09-17: the Opus campaign was not started on the VPS at all,
    because `claude auth status` reported loggedIn false, and no alert exists
    today that would say so.

    Only for `external` books. Every rule-based run has `decider` None by
    design - the strategy drives it - and alarming on those would fire on most
    of the desk for ever.
    """
    if run.get("strategy") != "external" or run.get("decider"):
        return False
    started = run.get("started_at") or 0
    step = TF_MS.get(run.get("tf", ""), 900_000)
    # Measured from the book's own start, not from now: a book created a minute
    # ago has not failed to be driven, it is waiting for its first bar.
    return (now_ms - started) > step * bars and not is_dead(run, now_ms) \
        and market_open(run.get("market", ""), int(now_ms))


def decider_gone(run: dict, now_ms: float, bars: int = STALE_BARS) -> bool:
    """Whether this book's model has stopped answering.

    Only for books a MODEL drives. A coin book's `last_at` moves only when its
    model takes a trade, so silence there is ordinary and alerting on it would
    cry wolf on every quiet hour.

    Suppressed while the feed itself is dead: a book with no bars has nothing to
    decide on, and two alarms for one outage is how an alert channel gets muted.
    """
    d = run.get("decider") or None
    if not d or d.get("last") == "coin":
        return False
    last = d.get("last_at")
    if not last:
        return False
    # The SAME threshold `changes` used to decide the feed was dead. Passing
    # the default here while the caller used the funded one gave a funded book
    # a window - feed stale by three bars and a half - where the feed alarm
    # fired AND the decider alarm was not suppressed, which is the two-alarms-
    # for-one-outage this guard exists to prevent.
    if is_dead(run, now_ms, bars):
        return False
    # A book holding a position is not asked at all — the desk allows one at a
    # time, so there is nothing to decide and the trader skips the call. Its
    # `last_at` freezes for as long as the trade runs, up to the four-hour
    # maximum hold, and alarming on that would fire on every winning trade.
    if run.get("open"):
        return False
    step = TF_MS.get(run.get("tf", ""), 900_000)
    return (now_ms - last) > step * DECIDER_STALE_BARS and market_open(run.get("market", ""), int(now_ms))


# --------------------------------------------------------------- the watch

def mirrors(run: dict, now_ms: float) -> list:
    """This book's mirrors that are still reporting, freshest first."""
    return [b for b in (run.get("brokers") or [])
            if b.get("at") and (now_ms - b["at"]) < MIRROR_STALE_MS]


def mirror_state(run: dict, now_ms: float) -> tuple:
    """(how many mirrors are reporting, the first refusal among them).

    A refusal is carried up verbatim. "The mirror is blocked" sends someone to
    a log; "broker refused: 10027 AutoTrading disabled by client" sends them to
    the button that fixes it.
    """
    live = mirrors(run, now_ms)
    blocked = next((b["blocked"] for b in live if b.get("blocked")), None)
    return len(live), blocked


def mirror_gaps(accounts: list, state: dict, now_ms: float,
                markets: dict | None = None) -> list[str]:
    """Books an account is configured to mirror and is not, BY NAME.

    Separate from the per-book mirror line in `changes`, which reads
    `run["brokers"]` and can only fire when a book's LAST mirror goes quiet. A
    book mirrored into two accounts that loses one of them is invisible there,
    and on 2026-09-17 one of those two carries money. This reads the other way
    round - from the account's own `runs` against its `mirroring` - so the
    question is "what is this account not copying", which is the question
    someone actually has to answer.

    Naming them is the whole point. "6 of 8" makes the reader open the desk to
    find out which two; the names are what they would have gone looking for.

    SUPPRESSION, and this one is a judgement with a cost. An executor that
    stops reporting is a process failure, not a market event: it does not fix
    itself at the open, and on a funded account it means a position may sit
    with nobody reconciling it. So a real-money gap is announced whatever the
    clock says. A paper account's gap waits for its book's market to open,
    because waking someone at three in the morning for a book that cannot
    trade until eight is how a channel gets muted - and a muted channel is
    worth nothing when the funded one does break.
    """
    out: list[str] = []
    markets = markets or {}
    seen: dict = state.setdefault("accounts", {})
    first = not seen
    for a in accounts or []:
        aid = str(a.get("id") or "?")
        want = [str(b) for b in (a.get("runs") or [])]
        have = {str(b) for b in (a.get("mirroring") or [])}
        real = bool(a.get("real_money"))
        missing = [b for b in want if b not in have]
        if not real:
            missing = [b for b in missing if market_open(markets.get(b, ""), int(now_ms))]
        was = seen.get(aid) or {}
        before = list(was.get("missing") or [])
        seen[aid] = {"missing": missing}
        if first or missing == before:
            continue
        if missing:
            tag = " — REAL MONEY" if real else ""
            names = ", ".join(f"<code>{esc(b)}</code>" for b in missing)
            out.append(
                f"\U0001f50c <b>{esc(aid)}</b>{tag} is not mirroring {len(missing)} of "
                f"{len(want)}: {names}. Those books go on deciding; the account will not follow."
            )
        elif before:
            out.append(f"✅ <b>{esc(aid)}</b> is mirroring every book it should again.")
    return out


#: The account fields every alert above is read from.
#:
#: None of them carries `skip_serializing_if` in `AccountDto`
#: (crates/fd-api/src/paper.rs), so each is always present in the JSON and a
#: missing one means the contract moved rather than that the value was empty.
#: That is what makes the check below safe to make loud: no ordinary state of
#: the desk can trigger it.
ACCOUNT_FIELDS = ("runs", "mirroring", "real_money")


def blind_spots(accounts: list, state: dict) -> list[str]:
    """When this watch has stopped being able to see, say so.

    `real_money`, `runs` and `mirroring` were added to the accounts route on
    2026-09-17 and nothing else depends on them yet, so they are the newest and
    least load-bearing part of the contract these alerts stand on. If any of
    them disappears, every account alert goes QUIET - no gap is ever computed,
    no book is ever named, and the channel looks exactly like a desk with
    nothing wrong. A watchdog's worst failure is silence, and silence is this
    one's default failure.

    Two ways to go blind, not one:

      * the payload arrives without the fields the alerts read
      * the payload arrives with no accounts at all, where there were some

    The second is not a contract change but it has the same effect: no rows,
    no gaps, nothing said. It is worth one line because an executor registry
    that empties itself is either a config that was edited or a desk that
    reloaded without one, and both are things somebody chose without meaning
    to.

    Deliberately NOT a fallback that guesses. A watch that invented a missing
    `real_money` would put every book on the tighter threshold or none of
    them, and either way would report a state of the world it made up.
    """
    out: list[str] = []
    was_blind = bool(state.get("blind"))
    missing: set = set()
    for a in accounts or []:
        missing.update(f for f in ACCOUNT_FIELDS if f not in a)

    if missing:
        if not was_blind:
            state["blind"] = True
            out.append(
                "⚠️ The accounts route no longer carries "
                + ", ".join(f"<code>{esc(f)}</code>" for f in sorted(missing))
                + " — mirror and executor alerts are blind until it does."
            )
        return out

    if accounts:
        if was_blind:
            state["blind"] = False
            out.append("✅ The accounts route is complete again.")
        state["had_accounts"] = True
    elif state.get("had_accounts") and not state.get("no_accounts"):
        state["no_accounts"] = True
        out.append(
            "⚠️ The desk is reporting no broker accounts at all — "
            "nothing is mirroring anything, and no account alert can fire."
        )
    if accounts:
        state["no_accounts"] = False
    return out


def broker_alarms(run_id: str, account_id: str, events: list,
                  state: dict, now_ms: float) -> list[str]:
    """The executor refusing to run at all, said once.

    Only `ALARMING_KINDS` - the start-time refusals that leave the mirror dead
    rather than the sizing rules declining one trade. AutoTrading off is the
    one that matters most and looks like nothing: every order comes back 10027
    and the executor is indistinguishable from a strategy that never fires,
    which cost 36 refused orders on the demo before anyone noticed.

    Announced on the event's own timestamp moving, not on the poll. The
    freshness window covers one case and nothing else: a VPS reboot brings the
    watch up beside an append-only `executor.jsonl` whose newest line may be
    from last week. Replaying that into the channel would be worse than
    silence, so an unseen event older than the window is LEARNED rather than
    announced - recorded so the next one is a transition.
    """
    key = f"{account_id}/{run_id}"
    seen: dict = state.setdefault("broker", {})
    last = int(seen.get(key) or 0)
    alarming = [e for e in (events or [])
                if e.get("kind") in ALARMING_KINDS and e.get("at")]
    if not alarming:
        return []
    newest = max(alarming, key=lambda e: int(e.get("at") or 0))
    at = int(newest.get("at") or 0)
    if at <= last:
        return []
    seen[key] = at
    if last == 0 and (now_ms - at) > BROKER_EVENT_FRESH_MS:
        return []
    return [f"⛔ <b>{esc(run_id)}</b> on <b>{esc(account_id)}</b> — {esc(describe(newest))}"]


def describe(event: dict) -> str:
    """One readable line out of an executor event.

    `order-failed` carries `retcode` and `comment` and no `reason`, and the
    retcode is the whole message: 10027 is AutoTrading off and is fixed by a
    button, while 10019 is no money and is not. A line that said only "an order
    failed" would send the reader to a log to find the one number that decides
    what they do next.
    """
    kind = str(event.get("kind") or "event")
    if kind == "order-failed":
        bits = [b for b in (
            str(event.get("action") or "an order"),
            f"retcode {event['retcode']}" if event.get("retcode") is not None else "",
            str(event.get("comment") or ""),
        ) if b]
        return "order failed: " + ", ".join(bits)
    return str(event.get("reason") or kind)


def stop_report(run_id: str, account_id: str, events: list,
                state: dict, now_ms: float) -> list[str]:
    """A STOP that was honoured, and whether it actually got flat.

    This exists because a kill switch that failed is currently announced as a
    success. `mt5_executor` closes every position on the STOP path, DISCARDS
    the boolean each close returns, logs `stopped` and exits 0, and writes no
    snapshot on the way out. With AutoTrading off every close comes back 10027,
    so the process reports a clean stop while a REAL position stays open with
    nothing reconciling it - and the launcher then refuses to restart, because
    the STOP file it is reacting to is still there.

    The only other signal is the mirror gap, and that says "not mirroring",
    which is exactly what an operator who dropped the STOP file expects to see.
    That is the trap: the failure and the intention look identical.

    "Stopped" is therefore the wrong thing to say - the operator meant to stop
    it. The question they cannot answer from anywhere else is whether it got
    flat, and the events answer it: the closes are attempted immediately before
    the `stopped` line, so a failed close sits within a second of it. A book
    that held nothing attempts no close and can produce no failure, which is
    why silence on that side is evidence rather than an absence of it.

    Both outcomes are announced. A clean stop gets a line on purpose: the
    mirror gap is about to say "not mirroring" and leave the reader guessing,
    and one line saying it closed flat is what stops that guess.
    """
    key = f"{account_id}/{run_id}"
    seen: dict = state.setdefault("stops", {})
    last = int(seen.get(key) or 0)
    stops = [e for e in (events or []) if e.get("kind") == "stopped" and e.get("at")]
    if not stops:
        return []
    newest = max(stops, key=lambda e: int(e.get("at") or 0))
    at = int(newest.get("at") or 0)
    if at <= last:
        return []
    seen[key] = at
    if last == 0 and (now_ms - at) > BROKER_EVENT_FRESH_MS:
        return []
    failed = [e for e in (events or [])
              if e.get("kind") == "order-failed" and e.get("at")
              and 0 <= at - int(e["at"]) <= STOP_CLOSE_WINDOW_MS]
    why = str(newest.get("reason") or "a STOP file")
    if failed:
        worst = max(failed, key=lambda e: int(e.get("at") or 0))
        return [
            f"\U0001f6a8 <b>{esc(run_id)}</b> on <b>{esc(account_id)}</b> — stopped on "
            f"{esc(why)}, but the close was REFUSED ({esc(describe(worst))}). "
            f"A position may still be open on the account with nothing reconciling it."
        ]
    return [
        f"✅ <b>{esc(run_id)}</b> on <b>{esc(account_id)}</b> — stopped on "
        f"{esc(why)} and closed flat."
    ]


def clipping(run_id: str, account_id: str, events: list,
             state: dict, now_ms: float) -> list[str]:
    """The broker resizing a book's orders, as a standing condition.

    `clamp_volume` returns whether the broker's minimum or maximum MOVED the
    size rather than merely rounding it, and the executor logs `clipped` with
    what it asked for and what it sent. On the funded account at lot_scale 0.2
    a book asking for less than 0.05 lots is pushed UP to the 0.01 minimum -
    bigger than the book intended, on every trade, and nowhere on any screen.
    Oversized is the direction that matters: undersized costs a book its edge,
    oversized costs the account more than the sizing rules agreed to risk.

    Announced as a condition, once, with a recovery - not per trade. A book too
    small for the account is too small on every trade it takes, and an alert
    that repeats on each one is an alert that gets muted by the end of the day.

    This belongs on a screen more than in a channel: it is a fact about how an
    account is configured, not an event someone must act on this minute. It is
    here because there is no screen for it and the account is funded. If one is
    built, delete this.
    """
    clips = [e for e in (events or []) if e.get("kind") == "clipped" and e.get("at")]
    seen: dict = state.setdefault("clipped", {})
    key = f"{account_id}/{run_id}"
    was = bool(seen.get(key))
    if len(clips) < CLIP_MIN_EVENTS:
        if was:
            seen[key] = False
            return [f"✅ <b>{esc(run_id)}</b> on <b>{esc(account_id)}</b> — "
                    f"the broker is no longer resizing it."]
        return []
    seen[key] = True
    if was:
        return []
    newest = max(clips, key=lambda e: int(e.get("at") or 0))
    asked, sending = newest.get("asked"), newest.get("sending")
    how = ""
    if isinstance(asked, (int, float)) and isinstance(sending, (int, float)) and asked > 0:
        way = "UP" if sending > asked else "down"
        how = f": it asked {asked:.4g} and the broker sent {sending:.4g}, resized {way}"
    return [
        f"⚠️ <b>{esc(run_id)}</b> on <b>{esc(account_id)}</b> — the broker's "
        f"size limits are moving this book's orders on {len(clips)} of its recent trades{esc(how)}."
    ]


def changes(runs: list, state: dict, now_ms: float, funded: set | None = None,
            reasons: dict | None = None) -> list[str]:
    """What is worth waking someone for, and nothing else.

    `funded` is the set of book ids configured on a real-money account, from
    `funded_books`. Defaulted to empty so a caller that cannot reach
    `/api/paper/accounts` keeps the old behaviour on the payload it does have,
    rather than losing every alert because one route was down.
    """
    funded = funded or set()
    out: list[str] = []
    seen: dict = state.setdefault("runs", {})
    first_run = not seen  # a fresh state announces nothing; it learns

    for r in runs:
        rid = r["id"]
        was = seen.get(rid, {})
        net = r.get("net_usd") or 0.0
        trades = r.get("trades") or 0
        open_pos = r.get("open") or None
        dead = is_dead(r, now_ms, STALE_BARS_REAL if rid in funded else STALE_BARS)
        mute = decider_gone(r, now_ms, STALE_BARS_REAL if rid in funded else STALE_BARS)
        idle = undriven(r, now_ms)
        # How many accounts are mirroring this book right now, and whether any
        # of them is being refused. `configured` is what the desk last saw, so
        # a mirror that was there and is gone reads as a drop rather than as a
        # book nobody ever mirrored.
        live_mirrors, refused = mirror_state(r, now_ms)

        if not first_run:
            # A trade closed: the only line here that is about money.
            if trades > was.get("trades", 0):
                closed = trades - was.get("trades", 0)
                delta = net - was.get("net", 0.0)
                out.append(
                    f"<b>{esc(rid)}</b> closed {closed} trade{'s' if closed > 1 else ''} "
                    f"{money(delta)} → book {money(net)}"
                )
            # A position opened.
            if open_pos and not was.get("open"):
                who = (r.get("decider") or {}).get("last")
                tag = f" [{esc(who)}]" if who else ""
                out.append(
                    f"<b>{esc(rid)}</b>{tag} opened <b>{esc(open_pos['side'])}</b> "
                    f"@ {esc(open_pos['entry_price'])} "
                    f"stop {esc(open_pos.get('stop'))} target {esc(open_pos.get('target'))}"
                )
            # A guard fired. Counted, not described: the desk's own log has the
            # detail and this is a nudge to go and look.
            fired = sum((r.get("closed_by_guard") or {}).values())
            if fired > was.get("guards", 0):
                names = ", ".join((r.get("closed_by_guard") or {}).keys())
                out.append(f"<b>{esc(rid)}</b> guard closed a position ({esc(names)})")
            # The model stopped answering, or started again. A separate
            # outage from a dead feed and it needs its own line: on
            # 2026-09-16 Codex's token expired for 45 minutes while the bars
            # kept arriving, so every feed was healthy and four decisions were
            # simply never made.
            if mute and not was.get("mute"):
                d = r.get("decider") or {}
                quiet = (now_ms - (d.get("last_at") or now_ms)) / 60000
                # The cause, when the caller could fetch it. The trader writes
                # its exception into decisions.jsonl even on the path where it
                # posts nothing (ai_trader.py: response = "ERROR: <Type>: ..."),
                # and /api/paper/reasoning serves that file — so the desk DOES
                # know why. An alert saying only "has not answered" would send
                # someone to find a line the desk could have quoted, and the
                # difference between "the model is slow" and "the CLI is signed
                # out" is the whole of what they do next.
                cause = (reasons or {}).get(rid) or ""
                out.append(
                    f"\U0001f507 <b>{esc(rid)}</b> — <code>{esc(d.get('last'))}</code> has not answered "
                    f"for {quiet:.0f} min. Bars are still arriving; the model is not."
                    + (f" Last error: {esc(cause)}." if cause else "")
                )
            # Nobody is driving it AT ALL — a different failure from a driver
            # that broke, and the one that looks like nothing is wrong.
            if idle and not was.get("idle"):
                out.append(
                    f"\U0001f6ab <b>{esc(rid)}</b> — no decider has ever posted to this book, "
                    f"and bars are arriving. Nothing is driving it."
                )
            elif was.get("idle") and not idle:
                out.append(f"✅ <b>{esc(rid)}</b> — something is driving it now.")
            elif was.get("mute") and not mute:
                d = r.get("decider") or {}
                out.append(f"\u2705 <b>{esc(rid)}</b> — <code>{esc(d.get('last'))}</code> is answering again.")
            # The mirror stopped writing. Its own outage, separate from a
            # dead feed and from a silent model: bars arrive, the model decides,
            # the paper book moves, and the account does nothing. The book's
            # positions are still protected by the stop and target the broker
            # holds - what is lost is every trade from here on.
            if was.get("mirrors", 0) > 0 and live_mirrors == 0:
                out.append(
                    f"\U0001f50c <b>{esc(rid)}</b> — the mirror has stopped reporting. "
                    f"The book goes on deciding; the account will not follow it."
                )
            elif live_mirrors > was.get("mirrors", 0) and was.get("mirrors", -1) == 0:
                out.append(f"\u2705 <b>{esc(rid)}</b> — the mirror is reporting again.")

            # The broker refused an order. Said once per distinct reason, not
            # once per poll: 10027 repeats every fifteen seconds for as long as
            # a button stays off, and an alert channel that repeats itself is
            # one nobody reads.
            # The account holding a different shape from the book. Read from
            # the snapshot rather than the event stream: it is recomputed every
            # poll and clears itself, and a self-clearing condition is awkward
            # to state correctly from a log of changes.
            out += drifts(r, state, now_ms)

            if refused and refused != was.get("refused"):
                out.append(f"\u26d4 <b>{esc(rid)}</b> — {esc(refused)}")
            elif was.get("refused") and not refused:
                out.append(f"\u2705 <b>{esc(rid)}</b> — orders are going through again.")

            # The feed died, or came back. Only on the transition.
            if dead and not was.get("dead"):
                behind = (now_ms - (r.get("last_bar_time") or now_ms)) / 60000
                out.append(f"⚠️ <b>{esc(rid)}</b> feed dead — no bar for {behind:.0f} min")
            elif was.get("dead") and not dead:
                out.append(f"✅ <b>{esc(rid)}</b> feed back")

        seen[rid] = {
            "trades": trades, "net": net, "open": bool(open_pos),
            "guards": sum((r.get("closed_by_guard") or {}).values()), "dead": dead,
            "mute": mute, "idle": idle, "mirrors": live_mirrors, "refused": refused,
        }

    for gone in set(seen) - {r["id"] for r in runs}:
        if not first_run:
            out.append(f"<b>{esc(gone)}</b> is no longer on the desk")
        seen.pop(gone, None)
    return out


# ------------------------------------------------------------ the commands

HELP = (
    "<b>Backcom Desk</b> — read only.\n"
    "/status — books, net, feeds\n"
    "/books — every book\n"
    "/ai — what the models last said\n\n"
    "This bot cannot trade. It only reads."
)


def answer(token: str, chat: str, cmd: str) -> None:
    try:
        runs = desk("/api/paper/status")["runs"]
    except Exception:  # noqa: BLE001
        say(token, chat, "The desk is not answering on 127.0.0.1:8138.")
        return

    if cmd.startswith("/books"):
        rows = sorted(runs, key=lambda r: -(r.get("net_usd") or 0.0))
        body = "\n".join(
            f"<code>{esc(r['id']):<18}</code> {money(r.get('net_usd') or 0.0):>10}  "
            f"{r.get('trades', 0)} fills"
            for r in rows
        )
        say(token, chat, f"<b>Books</b>\n<pre>{body}</pre>")
    elif cmd.startswith("/ai"):
        parts = []
        for r in runs:
            if not r.get("decider"):
                continue
            try:
                got = desk(f"/api/paper/reasoning/{urllib.parse.quote(r['id'])}?limit=1")
            except Exception:  # noqa: BLE001
                continue
            for d in got.get("decisions", [])[:1]:
                when = dt.datetime.utcfromtimestamp(d["bar_time"] / 1000).strftime("%H:%MZ")
                parts.append(
                    f"<b>{esc(r['id'])}</b> · <code>{esc(d['model'])}</code>\n"
                    f"{when} <b>{esc(d['side'])}</b> — {esc(d['reason'])}"
                )
        say(token, chat, "\n\n".join(parts) or "No model has spoken yet.")
    elif cmd.startswith("/status"):
        say(token, chat, snapshot(runs))
    else:
        say(token, chat, HELP)


def serve_commands(token: str, allowed: str, state: dict) -> None:
    """Answer the allowlisted chat. Tell everyone else nothing.

    The refusal is deliberately blank about what this is. A bot token can leak,
    and a stranger who finds it should learn the name of no book, no strategy
    and no number from it.
    """
    offset = state.get("offset", 0)
    try:
        got = tg(token, "getUpdates", {"offset": offset, "timeout": 0, "limit": 20}, timeout=25)
    except RuntimeError as e:
        print(f"getUpdates: {e}", flush=True)
        return
    for upd in got.get("result", []):
        state["offset"] = upd["update_id"] + 1
        msg = upd.get("message") or upd.get("edited_message") or {}
        chat_id = str((msg.get("chat") or {}).get("id", ""))
        text = (msg.get("text") or "").strip()
        if not text:
            continue
        if chat_id != allowed:
            who = (msg.get("from") or {}).get("username") or (msg.get("from") or {}).get("id")
            print(f"refused chat {chat_id} (@{who})", flush=True)
            try:
                say(token, chat_id, "Not available.")
            except RuntimeError:
                pass
            continue
        print(f"command {text!r}", flush=True)
        try:
            answer(token, allowed, text.lower())
        except RuntimeError as e:
            print(f"answer failed: {e}", flush=True)


# ------------------------------------------------------------------- main

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--poll", type=float, default=30.0)
    ap.add_argument("--once", action="store_true")
    ap.add_argument("--test", action="store_true", help="send a probe message and exit")
    ap.add_argument("--no-commands", action="store_true", help="watch only; do not answer")
    args = ap.parse_args()

    token, allowed = credentials()
    if args.test:
        try:
            runs = desk("/api/paper/status")["runs"]
            say(token, allowed, "🟢 Telegram alerts wired.\n\n" + snapshot(runs))
        except Exception as e:  # noqa: BLE001
            say(token, allowed, f"🟢 Telegram alerts wired, but the desk is not answering: {type(e).__name__}")
        print("probe sent", flush=True)
        return 0

    state = load_state()
    print(f"telegram: watching the desk, answering chat {allowed} only", flush=True)
    while True:
        # Reading the desk and reaching Telegram are separate failures and must
        # be reported as such. Wrapped together, an unreachable Telegram —
        # "chat not found" before anyone has pressed /start, a network blip —
        # came back as "the desk stopped answering", which is a false alarm
        # about the one thing this exists to watch.
        runs = None
        try:
            runs = desk("/api/paper/status")["runs"]
        except Exception as e:  # noqa: BLE001
            if not state.get("api_down"):
                state["api_down"] = True
                try:
                    say(token, allowed, f"⚠️ The desk stopped answering ({type(e).__name__}).")
                except RuntimeError as t:
                    print(f"desk down, and telegram unreachable: {t}", flush=True)

        if runs is not None:
            now_ms = time.time() * 1000
            # The accounts route is read in its OWN try. It is newer than
            # `/api/paper/status` and it answers from a different part of the
            # desk, so a failure here must cost the account alerts and nothing
            # else - losing the trade and feed lines because a second route
            # was down would be this watch reporting its own outage as the
            # desk's, which is the mistake `api_down` already exists for.
            accounts = []
            try:
                accounts = desk("/api/paper/accounts").get("accounts") or []
            except Exception as e:  # noqa: BLE001
                if not state.get("accounts_down"):
                    state["accounts_down"] = True
                    print(f"telegram: accounts route unreadable ({type(e).__name__})", flush=True)
            else:
                state["accounts_down"] = False

            funded = funded_books(accounts)
            markets = {str(r.get("id")): str(r.get("market") or "") for r in runs}

            # Why, for the books that are about to be reported silent — and
            # only those. `decider_gone` is pure and cheap, so asking it first
            # costs nothing and keeps this fetch at zero on a desk where every
            # model is answering. Already-reported books are skipped too: the
            # alert fires on the transition, so the cause is only ever needed
            # once.
            reasons: dict = {}
            known: dict = state.get("runs") or {}
            for r in runs:
                rid = str(r.get("id"))
                bars = STALE_BARS_REAL if rid in funded else STALE_BARS
                if not decider_gone(r, now_ms, bars) or (known.get(rid) or {}).get("mute"):
                    continue
                try:
                    got = desk(f"/api/paper/reasoning/{urllib.parse.quote(rid)}?limit=3")
                    reasons[rid] = cause_of(got.get("decisions") or [])
                except Exception:  # noqa: BLE001
                    pass          # the alert still fires; it just cannot say why

            lines = changes(runs, state, now_ms, funded, reasons)
            # Before the account alerts, whether they can see at all. Only when
            # the route answered: a route that is down is already reported by
            # `accounts_down` above, and saying both would be two alarms for
            # one outage.
            if not state.get("accounts_down"):
                lines += blind_spots(accounts, state)
            lines += mirror_gaps(accounts, state, now_ms, markets)

            # Executor events, fetched ONLY for the books an account should be
            # mirroring and is not. A refusal that leaves the mirror running is
            # already carried by `blocked` on the status payload; what is not
            # visible anywhere else is the executor that refused to start and
            # exited, and that book is by definition missing from `mirroring`.
            # Reading only the gap keeps this bounded - no gap, no extra GETs.
            def events_for(book: str, acct: str):
                got = desk(
                    f"/api/paper/broker-events/{urllib.parse.quote(book)}"
                    f"?account={urllib.parse.quote(acct)}&limit={BROKER_EVENT_LIMIT}"
                )
                return got.get("events") or []

            due: dict = state.setdefault("clip_checked", {})
            for a in accounts:
                aid = str(a.get("id") or "")
                have = {str(b) for b in (a.get("mirroring") or [])}
                for book in [str(b) for b in (a.get("runs") or []) if str(b) not in have]:
                    try:
                        evs = events_for(book, aid)
                    except Exception:  # noqa: BLE001
                        continue      # one unreadable book must not stop the rest
                    # Both read the same fetch. `stop_report` is the one that
                    # matters on a funded account: it is the only place a STOP
                    # that failed to get flat can be told apart from a STOP
                    # that worked, and from here they look identical.
                    lines += stop_report(book, aid, evs, state, now_ms)
                    lines += broker_alarms(book, aid, evs, state, now_ms)

                # A HEALTHY funded mirror, checked rarely. Clipping happens on a
                # book that is working, so the gap fetch above can never see it,
                # and polling every book every 30 seconds to find a condition
                # that changes weekly would be paying continuously for news that
                # almost never arrives.
                if not a.get("real_money"):
                    continue

                # Entry lag, from the run-detail route's `events` — which is
                # `fills.jsonl` served, so this reads the file's own `opened`
                # lines rather than the status payload's copy of `learned_at`.
                # That copy is cleared when a book goes flat, so a day of
                # entries cannot be read back from it: by the time anyone asks,
                # most of them are gone.
                thr = lag_alarm_s()
                lag_due: dict = state.setdefault("lag_checked", {})
                rows: dict = state.setdefault("lag_rows", {})
                for book in sorted(have):
                    if now_ms - float(lag_due.get(book) or 0) >= LAG_CHECK_MS:
                        lag_due[book] = now_ms
                        try:
                            det = desk(f"/api/paper/run/{urllib.parse.quote(book)}?bars=1")
                        except Exception:  # noqa: BLE001
                            pass
                        else:
                            evs = det.get("events") or []
                            rows[book] = lags(evs, now_ms)
                            lines += lag_alarms(book, evs, state, now_ms, thr)

                for book in sorted(have):
                    if now_ms - float(due.get(f"{aid}/{book}") or 0) < CLIP_CHECK_MS:
                        continue
                    due[f"{aid}/{book}"] = now_ms
                    try:
                        lines += clipping(book, aid, events_for(book, aid), state, now_ms)
                    except Exception:  # noqa: BLE001
                        continue

            # One entry-lag line per funded book, once a day.
            #
            # There was no periodic summary in this file to put it in - the
            # watch speaks on transitions and otherwise says nothing - so this
            # is one, and it is deliberately the smallest one that answers the
            # question: what is the gap today. A first run LEARNS the day
            # rather than sending, the same rule every other alert here
            # follows, so starting the watch does not announce a summary
            # nobody asked for.
            today = dt.datetime.utcfromtimestamp(now_ms / 1000).strftime("%Y-%m-%d")
            rows = state.get("lag_rows") or {}
            if state.get("lag_day") is None:
                state["lag_day"] = today
            elif state["lag_day"] != today and rows:
                state["lag_day"] = today
                lines.append("\n".join(
                    ["<b>Entry lag, last 24h</b>"]
                    + [lag_line(k, v) for k, v in sorted(rows.items())]
                ))

            if state.get("api_down"):
                state["api_down"] = False
                lines.insert(0, "✅ The desk is answering again.")
            for line in lines:
                try:
                    say(token, allowed, line)
                except RuntimeError as t:
                    # The desk is fine; the messenger is not. Say so on stdout
                    # and keep the state, so nothing is announced twice later.
                    print(f"could not send: {t}", flush=True)

        if not args.no_commands:
            try:
                serve_commands(token, allowed, state)
            except Exception as e:  # noqa: BLE001
                print(f"commands: {type(e).__name__}", flush=True)

        save_state(state)
        if args.once:
            return 0
        time.sleep(args.poll)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(0)
