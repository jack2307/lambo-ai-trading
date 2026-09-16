"""Telegram alerts for the paper desk, and a read-only bot behind an allowlist.

    python py/live/telegram_notify.py                 # watch and announce
    python py/live/telegram_notify.py --once --test   # send one probe and exit

Two jobs, one process:

* **It watches.** Polls `/api/paper/status` and announces what changed: a trade
  opened, a trade closed, a guard fired, a feed died. Nothing here reaches a
  book — it reads the same status route the UI reads.
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

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
STATE = os.path.join(ROOT, "data", "paper", "telegram_state.json")
API = "https://api.telegram.org"

# How far behind its own timeframe a book may fall before it counts as dead.
# Deliberately looser than the desk's own `stale` badge: the badge is read by a
# person who is already looking, this wakes someone up.
STALE_BARS = 6
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


def is_dead(run: dict, now_ms: float) -> bool:
    last = run.get("last_bar_time")
    if not last:
        return True
    limit = TF_MS.get(run.get("tf", ""), 900_000) * STALE_BARS
    return (now_ms - last) > limit and market_open(run.get("market", ""), int(now_ms))


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
    step = TF_MS.get(run.get("tf", ""), 900_000)
    return (now_ms - last) > step * DECIDER_STALE_BARS and market_open(run.get("market", ""), int(now_ms))


# --------------------------------------------------------------- the watch

def changes(runs: list, state: dict, now_ms: float) -> list[str]:
    """What is worth waking someone for, and nothing else."""
    out: list[str] = []
    seen: dict = state.setdefault("runs", {})
    first_run = not seen  # a fresh state announces nothing; it learns

    for r in runs:
        rid = r["id"]
        was = seen.get(rid, {})
        net = r.get("net_usd") or 0.0
        trades = r.get("trades") or 0
        open_pos = r.get("open") or None
        dead = is_dead(r, now_ms)
        mute = decider_gone(r, now_ms)

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
            # The feed died, or came back. Only on the transition.
            if dead and not was.get("dead"):
                behind = (now_ms - (r.get("last_bar_time") or now_ms)) / 60000
                out.append(f"⚠️ <b>{esc(rid)}</b> feed dead — no bar for {behind:.0f} min")
            elif was.get("dead") and not dead:
                out.append(f"✅ <b>{esc(rid)}</b> feed back")

        seen[rid] = {
            "trades": trades, "net": net, "open": bool(open_pos),
            "guards": sum((r.get("closed_by_guard") or {}).values()), "dead": dead,
            "mute": mute,
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
            lines = changes(runs, state, time.time() * 1000)
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
