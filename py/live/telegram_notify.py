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
ALARMING_KINDS = frozenset({"autotrading-off", "refused"})


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


def decider_gone(run: dict, now_ms: float) -> bool:
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
    if is_dead(run, now_ms):
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
    why = str(newest.get("reason") or newest.get("kind") or "refused")
    return [f"⛔ <b>{esc(run_id)}</b> on <b>{esc(account_id)}</b> — {esc(why)}"]


def changes(runs: list, state: dict, now_ms: float, funded: set | None = None) -> list[str]:
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
        mute = decider_gone(r, now_ms)
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
                out.append(
                    f"\U0001f507 <b>{esc(rid)}</b> — <code>{esc(d.get('last'))}</code> has not answered "
                    f"for {quiet:.0f} min. Bars are still arriving; the model is not."
                )
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
            "mute": mute, "mirrors": live_mirrors, "refused": refused,
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
            lines = changes(runs, state, now_ms, funded)
            lines += mirror_gaps(accounts, state, now_ms, markets)

            # Executor events, fetched ONLY for the books an account should be
            # mirroring and is not. A refusal that leaves the mirror running is
            # already carried by `blocked` on the status payload; what is not
            # visible anywhere else is the executor that refused to start and
            # exited, and that book is by definition missing from `mirroring`.
            # Reading only the gap keeps this bounded - no gap, no extra GETs.
            for a in accounts:
                aid = str(a.get("id") or "")
                have = {str(b) for b in (a.get("mirroring") or [])}
                for book in [str(b) for b in (a.get("runs") or []) if str(b) not in have]:
                    try:
                        got = desk(
                            f"/api/paper/broker-events/{urllib.parse.quote(book)}"
                            f"?account={urllib.parse.quote(aid)}&limit={BROKER_EVENT_LIMIT}"
                        )
                    except Exception:  # noqa: BLE001
                        continue      # one unreadable book must not stop the rest
                    lines += broker_alarms(book, aid, got.get("events") or [], state, now_ms)

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
