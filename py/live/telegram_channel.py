"""Read-only V10 trade announcements. No Telegram commands or trading calls.

Each destination learns its initial broker snapshot without replaying history.
Acknowledgements are persisted after each successful send; failures retry on
the next poll. Telegram has no idempotency key, so a lost HTTP response or a
crash between delivery and persistence can still duplicate one announcement.
"""
import html
import json
import math


# Measured on xau-stoch's 73 closed trades by the original Claude prototype.
# These are historical touch rates, not probabilities.  Keeping the model data
# beside the renderer makes another model an explicit, swappable configuration
# rather than silently borrowing xau-stoch's statistics.
MODELS = {
    "xau-stoch": {
        "name": "Lambo V10",
        "sample": 73,
        "ladder": [(0.33, 81), (0.63, 60), (1.17, 41), (1.46, 26), (2.00, 11)],
    },
}


def price(value):
    try:
        return f"{float(value):.2f}"
    except (TypeError, ValueError):
        return "?"


def sized_lot(balance, risk):
    """Largest standard-account lot step that does not exceed 1% risk."""
    raw = (float(balance) * .01) / (float(risk) * 100)
    if raw < .01:
        return None
    return math.floor((raw + 1e-12) / .01) * .01


def trade_shape(rid, broker, pos):
    model = MODELS.get(rid)
    try:
        entry, stop = float(pos["entry_price"]), float(pos["sl"])
        side = str(pos["side"]).upper()
        risk = abs(entry - stop)
    except (KeyError, TypeError, ValueError):
        return None
    if model is None or side not in ("LONG", "SHORT") or risk <= 0:
        return None
    sign = 1 if side == "LONG" else -1
    levels = [entry + sign * risk * multiple for multiple, _ in model["ladder"]]
    return {"entry": entry, "stop": stop, "target": pos.get("tp"),
            "side": side, "risk": risk, "levels": levels,
            "opened_at": pos.get("opened_at"), "lots": pos.get("lots"),
            "model": model, "highest": 0, "message_id": None, "closed": False}


def signal_message(broker, shape):
    sell = shape["side"] == "SHORT"
    symbol = str(broker.get("symbol") or "XAUUSD").split(".")[0]
    out = [
        ("🔴 BÁN" if sell else "🟢 MUA") + " " + esc(symbol)
        + " · Model: <b>" + esc(shape["model"]["name"]) + "</b>",
        "Vào <b>" + price(shape["entry"]) + "</b>   SL <b>"
        + price(shape["stop"]) + "</b>   (" + price(shape["risk"]) + ")",
        "",
    ]
    for number, ((multiple, rate), level) in enumerate(
            zip(shape["model"]["ladder"], shape["levels"]), 1):
        out.append(f"TP{number}  <b>{price(level)}</b> · {rate}%")
    out += [
        ("<i>% = tỷ lệ lệnh trong " + str(shape["model"]["sample"])
         + " lệnh quá khứ từng chạm mức đó, không phải xác suất lãi.</i>"),
        "Hệ thống thoát <b>" + price(shape["target"]) + "</b>",
        "",
        "<b>KHỐI LƯỢNG — rủi ro 1% vốn</b>",
    ]
    lots = []
    for balance in (1000, 5000, 10000):
        lot = sized_lot(balance, shape["risk"])
        lots.append(f"{balance // 1000}.{0:03d}$ → "
                    + (f"{lot:.2f}" if lot is not None else "dưới 0.01"))
    out += ["   ".join(lots),
            "<code>lot = (vốn × 1%) ÷ (" + price(shape["risk"]) + " × 100)</code>",
            "<i>Tài khoản thường, 1 lot = 100 oz. Tài khoản cent lệch 100 lần.</i>",
            "",
            "⚠️ CFD có đòn bẩy, bạn có thể mất toàn bộ vốn. Đây là tín hiệu hệ "
            "thống tự động, không phải khuyến nghị đầu tư."]
    return "\n".join(out)


def milestone_message(number, shape):
    multiple = shape["model"]["ladder"][number - 1][0]
    return f"✅ TP{number} chạm {price(shape['levels'][number - 1])} (+{multiple:.2f}R)"


def matching_signal(signals, fill):
    direction = str(fill.get("direction") or "").upper()
    try:
        entry = float(fill.get("entryPrice"))
    except (TypeError, ValueError):
        entry = None
    try:
        entry_time = int(fill.get("entryTime"))
    except (TypeError, ValueError):
        entry_time = None
    try:
        lots = float(fill.get("lots"))
    except (TypeError, ValueError):
        lots = None
    candidates = []
    for ticket, shape in signals.items():
        if shape.get("closed") or shape.get("side") != direction:
            continue
        try:
            time_delta = abs(int(shape.get("opened_at")) - entry_time)
        except (TypeError, ValueError):
            time_delta = float("inf")
        try:
            lot_delta = abs(float(shape.get("lots")) - lots)
        except (TypeError, ValueError):
            lot_delta = float("inf")
        price_delta = (abs(shape.get("entry", 0) - entry)
                       if entry is not None else float("inf"))
        candidates.append(((time_delta, lot_delta, price_delta), ticket, shape))
    return min(candidates, default=(None, None, None), key=lambda row: row[0])[2]


def close_message(fill, broker, shape):
    try:
        exit_price = float(fill.get("exitPrice"))
        result_r = ((exit_price - shape["entry"]) / shape["risk"]
                    * (1 if shape["side"] == "LONG" else -1))
    except (TypeError, ValueError, ZeroDivisionError):
        result_r = None
    pnl = fill.get("pnl")
    pnl_text = f"{pnl:+.2f}" if isinstance(pnl, (int, float)) else "?"
    touched = (f" (đã chạm TP{shape['highest']})" if shape.get("highest") else "")
    r_text = f"{result_r:+.2f}R" if result_r is not None else "?R"
    return ("🏁 Đóng " + price(fill.get("exitPrice")) + " · " + r_text + touched
            + "\nP&amp;L: <b>" + pnl_text + " " + esc(broker.get("currency")) + "</b>")


def crossed(shape, current):
    if shape["side"] == "LONG":
        return sum(current >= level for level in shape["levels"])
    return sum(current <= level for level in shape["levels"])


def book_extreme(run):
    """Price extreme retained by the book, even after the live price retraces."""
    opened = run.get("open") or {}
    try:
        entry = float(opened["entry_price"])
        risk = float(opened["risk"])
        mfe = float(opened["mfe"])
        side = str(opened["side"]).upper()
    except (KeyError, TypeError, ValueError):
        return None
    if side == "LONG":
        return entry + mfe * risk
    if side == "SHORT":
        return entry - mfe * risk
    return None


def esc(value):
    return html.escape(str(value if value is not None else "?"))


def fill_key(fill):
    # Identity excludes P&L/rebate, which can be restated independently.
    return json.dumps([fill.get(k) for k in
                       ("entryTime", "exitTime", "direction", "lots")])


def relay(runs, state, chat, send, save):
    """Publish only broker-confirmed V10 opens/closes using injected I/O."""
    books = state.setdefault("v10_channels", {}).setdefault(str(chat), {})
    for run in runs:
        for broker in run.get("brokers") or []:
            if broker.get("account") != "vantage-v10":
                continue
            rid = str(run["id"])
            pos = broker.get("position") or {}
            ticket = str(pos["ticket"]) if pos.get("ticket") else None
            fills = broker.get("fills") or []
            if rid not in books:
                books[rid] = {"tickets": [ticket] if ticket else [],
                              "fills": list(dict.fromkeys(fill_key(f) for f in fills)),
                              "signals": {}}
                save()
                continue
            seen = books[rid]
            signals = seen.setdefault("signals", {})
            head = ("<b>Lambo V10</b> · " + esc(broker.get("symbol"))
                    + " · " + esc(rid))
            # Close first, then the replacement position when both change.
            for fill in fills:
                key = fill_key(fill)
                if key in seen["fills"]:
                    continue
                shape = matching_signal(signals, fill)
                if shape and shape.get("message_id"):
                    send(close_message(fill, broker, shape), shape["message_id"])
                    shape["closed"] = True
                else:
                    # A close whose OPEN this channel never announced. Recorded,
                    # never broadcast: a bare "ĐÓNG" with no signal above it is
                    # noise to a reader and reads like a loss report with no
                    # context. It also arrives in floods -- `matching_signal`
                    # returns None once every announced shape is `closed`, so a
                    # backlog of fills (a fresh book after a channel change, a
                    # run id the ladder has no model for, positions that open
                    # and close between two polls) orphans EVERY one of them.
                    # Measured 2026-10-10: the book carried 755 fills against
                    # 16 announced signals.
                    print("v10 channel: orphan fill not broadcast (%s %s -> %s)"
                          % (fill.get("direction"), fill.get("entryPrice"),
                             fill.get("exitPrice")), flush=True)
                seen["fills"].append(key)
                save()
            if ticket and ticket not in seen["tickets"]:
                shape = trade_shape(rid, broker, pos)
                if shape:
                    shape["message_id"] = send(signal_message(broker, shape))
                    signals[ticket] = shape
                else:
                    # No ladder model for this run id, or the position is
                    # missing entry/stop. Half a signal -- no levels, no risk
                    # unit -- is worse than silence here, and it is also what
                    # orphans the close later. `MODELS` carries `xau-stoch`
                    # only, so every other run id took this path.
                    print("v10 channel: no shape, open not announced (%s %s)"
                          % (rid, ticket), flush=True)
                seen["tickets"].append(ticket)
                save()
                continue
            if ticket and ticket in signals:
                shape = signals[ticket]
                try:
                    current = float(pos.get("price_now"))
                except (TypeError, ValueError):
                    current = None
                if current is not None and shape.get("message_id"):
                    reached = crossed(shape, current)
                    extreme = book_extreme(run)
                    if extreme is not None:
                        reached = max(reached, crossed(shape, extreme))
                    while shape["highest"] < reached:
                        number = shape["highest"] + 1
                        send(milestone_message(number, shape), shape["message_id"])
                        shape["highest"] = number
                        save()
