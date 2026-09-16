"""A panel of advisors that may refuse a trade or make it smaller. Nothing else.

    python py/live/advisor.py --api=http://127.0.0.1:8138 --poll=20
    python py/live/advisor.py --dry-run          # decide and log locally, post nothing
    python py/live/advisor.py --rules-only       # no model calls at all
    python py/live/advisor.py --model=gpt-4o     # the whole panel on OpenAI
    python py/live/advisor.py --agent-model risk=gpt-4o --agent-model news=claude-opus-5

Either provider, and a mixed panel on purpose. The API is chosen from the
model's name (`claude*` -> Anthropic, `gpt*`/`o1`/`o3`/`o4` -> OpenAI) and the
key is read from `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`; an agent with no key
is named at startup and does not run. Every turn records the model that
answered it, so `scripts/advisor_review.py --agents` can later say which
provider's objections were worth listening to — on this desk's own trades
rather than on a benchmark.

It polls `GET /api/paper/pending` for entries about to fill, asks each agent in
turn, reconciles them, and posts one verdict to `POST /api/paper/advice` with
the whole conversation attached.

**Read `docs/paper/ADVISOR.md` before changing anything here.** The contract in
that file is enforced on the Rust side — this process has no way to choose a
side, move a stop, set a price, enlarge a trade, or open one nobody asked for,
because the route it posts to has no field for any of it. What is left is one
number in [0, 1], and the only honest thing this file can get wrong is that
number.

**The panel is conservative by construction.** The verdict is the MINIMUM of
the agents' numbers, so any one of them can veto and none can overrule a
refusal. That is the right asymmetry for a thing whose whole job is to say no:
an advisor that could be talked round by a more confident colleague is just a
slower way of allowing everything.

**If this process dies, the desk trades exactly as the strategy decided.** No
advice is posted, the intent fills unmodified, and nothing retries. That is the
declared default and it is worth more than any uptime this script could offer.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import glob
import io
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

#: Nothing in the request may be larger than this, whatever a model says.
MAX_FACTOR = 1.0

#: Room for one JSON object, plus whatever the model thinks first.
#:
#: 512 was enough for a model that answers and far too little for one that
#: reasons: measured on gpt-5, a 512 budget was consumed entirely by reasoning
#: tokens and the reply came back EMPTY. An empty reply is read as a full-size
#: allow — safe, and silently useless, because such an agent agrees with
#: everything for ever while the log shows it working. Unused output tokens
#: cost nothing, so the cap is set where a reasoning model can still speak.
OPENAI_MAX_TOKENS = 4096

#: Models that think before answering, and that therefore take `reasoning_effort`.
#:
#: "low" rather than the default: this panel is asked a small, bounded question
#: about a trade it can see in full, and on the same prompt "low" cut reasoning
#: from 448 tokens to 128 and latency from 3.8s to 2.3s with no change in the
#: quality of the answer. Reasoning tokens are billed, and a cheaper advisor
#: gets consulted more often.
REASONING_PREFIXES = ("gpt-5", "o1", "o3", "o4")
REASONING_EFFORT = "low"

#: The default panel. Any agent may be pointed at any model with
#: `--agent-model risk=gpt-4o`, and a mixed panel is not a compromise — it is
#: the experiment. Every turn already records the model that produced it, so
#: after a few hundred consultations `advisor_review.py --agents` says which
#: provider's objections were worth listening to, on this desk's own trades.
DEFAULT_MODEL = "claude-opus-5"

#: How to talk to each provider. Two shapes, one adapter each.
#:
#: Deliberately hand-rolled over `urllib` rather than importing two vendor
#: SDKs: this process must start on a machine with nothing installed, and the
#: whole of what it needs from either API is "send one prompt, read one
#: string". An SDK would add a dependency, a version to track, and no ability
#: this file uses.
# Where the Codex binary lives. The npm package (`@openai/codex`) does not
# install its Windows platform binary on this machine — `npm i -g` leaves a
# shim that throws "Missing optional dependency @openai/codex-win32-x64" — but
# the Codex desktop app ships a working `codex.exe` under AppData. So: the
# environment wins, then PATH, then the app's own copy.
CODEX_APP_GLOB = os.path.join(
    os.environ.get("LOCALAPPDATA", ""), "OpenAI", "Codex", "bin", "*", "codex.exe"
)


_CODEX_BIN: list = []


def codex_bin() -> str:
    """The first Codex executable that actually RUNS.

    Presence on PATH is not enough and was actively misleading here: the npm
    package leaves a `codex.CMD` shim that shadows the working binary and
    throws `Missing optional dependency @openai/codex-win32-x64` on every
    call. A locator that stopped at the first name it found picked the broken
    one. So each candidate is tried with `--version` and the first that exits
    cleanly wins; the answer is cached, because this is once per process, not
    once per bar.
    """
    if _CODEX_BIN:
        return _CODEX_BIN[0]
    candidates = []
    named = os.environ.get("CODEX_BIN")
    if named:
        candidates.append(named)
    candidates.extend(sorted(glob.glob(CODEX_APP_GLOB), reverse=True))
    found = shutil.which("codex")
    if found:
        candidates.append(found)

    tried = []
    for path in candidates:
        if not os.path.isfile(path):
            continue
        try:
            probe = subprocess.run([path, "--version"], stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, timeout=60)
        except (OSError, subprocess.TimeoutExpired) as e:
            tried.append(f"{path}: {type(e).__name__}")
            continue
        if probe.returncode == 0:
            _CODEX_BIN.append(path)
            return path
        tried.append(f"{path}: exit {probe.returncode}")
    raise RuntimeError(
        "no working codex binary; set CODEX_BIN or install the Codex app. Tried: "
        + "; ".join(tried or ["nothing"])
    )


# The system prompt the CLI provider runs under. It REPLACES Claude Code's
# own, which is what keeps a decision call from dragging a coding agent's
# tools, skills and project files into a question about forty candles.
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))

CLI_SYSTEM_PROMPT = (
    "You are a trading decider answering one question about one instrument. "
    "Answer with JSON and nothing else. Do not explain, do not use tools, do "
    "not ask questions."
)

PROVIDERS = {
    # The subscription, through the Claude Code CLI, rather than a metered API
    # key. The owner's instruction, 2026-09-15: "lấy từ gói luôn ko cần qua
    # API". `env` is None because there is no key to find — the CLI carries
    # the account's own credentials.
    #
    # It is listed FIRST so a Claude model goes to the plan by default;
    # `--provider anthropic` still forces the metered API for anyone who wants
    # it. The flags matter and each one is load-bearing:
    #   --allowed-tools ""        a decider must not read files or run commands
    #   --strict-mcp-config       no MCP server joins a trading decision
    #   --no-session-persistence  every bar is asked cold, with no memory of
    #                             the last one, exactly like the HTTP models
    #   --system-prompt           replaces Claude Code's, see above
    # and the prompt goes in on STDIN, never argv: it carries quotes, dollar
    # signs and newlines, and Windows argv quoting would mangle it silently.
    "claude-cli": {
        "url": None,
        "env": None,
        "prefixes": ("claude", "opus", "sonnet", "haiku"),
        "cli": "claude",
    },
    # The ChatGPT plan, through the Codex CLI, rather than a metered OpenAI
    # key. Owner's instruction, 2026-09-15: "GPT cũng lấy gói ra đi". Same
    # shape and same reasoning as `claude-cli` above; the flags differ because
    # the tool does.
    #
    # `--ignore-user-config` is the load-bearing one here. `~/.codex/config.toml`
    # on this machine belongs to the Codex DESKTOP app and enables its plugins
    # (browser, documents, pdf, spreadsheets). None of those belong anywhere
    # near a question about forty candles, and loading them would also mean a
    # trading loop and a desktop app sharing one mutable config file.
    #
    # Listed AFTER `openai` so a bare `gpt-5` keeps going to the metered API it
    # is already running on: switching a live campaign's provider silently
    # would change its decider mid-book. Ask for the plan by name —
    # `--model=codex/gpt-5` — or pass `--provider codex-cli`.
    "codex-cli": {
        "url": None,
        "env": None,
        "prefixes": ("codex/", "codex-"),
        "cli": "codex",
    },
    "anthropic": {
        "url": "https://api.anthropic.com/v1/messages",
        "env": "ANTHROPIC_API_KEY",
        "prefixes": ("claude",),
    },
    # DeepSeek speaks the OpenAI chat API, so it needs no branch in `ask` —
    # only a base URL and a key. Priced per token rather than by subscription:
    # measured 2026-09-16 at roughly $1/month for one 15m book on
    # `deepseek-flash`, against about $80/month equivalent for the same book on
    # the Claude plan, because a direct API call spends 1,427 tokens where the
    # CLI route spends 26,509 on the same question.
    "deepseek": {
        "url": "https://api.deepseek.com/v1/chat/completions",
        "env": "DEEPSEEK_API_KEY",
        "config_section": "ai.deepseek",
        "prefixes": ("deepseek",),
    },
    "openai": {
        "url": "https://api.openai.com/v1/chat/completions",
        "env": "OPENAI_API_KEY",
        "prefixes": ("gpt", "o1", "o3", "o4", "chatgpt"),
    },
}


def key_for(provider: str) -> str:
    """The provider's key, from the environment or from `config/local.toml`.

    Two places because they suit different secrets. A key exported into the
    environment is convenient for one shell; a key in the gitignored config is
    there after a reboot and is not visible in a process list. Environment
    wins, so a shell can override the file without editing it.
    """
    spec = PROVIDERS[provider]
    env = spec.get("env")
    if env:
        from_env = os.environ.get(env)
        if from_env:
            return from_env
    section = spec.get("config_section")
    if not section:
        return ""
    path = os.path.join(ROOT, "config", "local.toml")
    try:
        text = io.open(path, encoding="utf-8").read()
    except OSError:
        return ""
    m = re.search(r"\[" + re.escape(section) + r"\][^\[]*?api_key\s*=\s*\"([^\"]+)\"", text, re.S)
    return m.group(1) if m else ""


def provider_of(model: str) -> str:
    """Which API a model name belongs to.

    Named by prefix rather than configured, because a panel with four agents
    should not need four more flags to say the obvious. `--provider` overrides
    it for a model whose name this does not recognise.
    """
    lowered = model.lower()
    for name, spec in PROVIDERS.items():
        if lowered.startswith(spec["prefixes"]):
            return name
    raise ValueError(
        f"cannot tell which API `{model}` belongs to; pass --provider "
        + "|".join(sorted(PROVIDERS))
    )


def post_json(url: str, payload: dict, timeout: float = 30.0, headers: dict | None = None) -> dict:
    body = json.dumps(payload).encode("utf-8")
    head = {"Content-Type": "application/json"}
    head.update(headers or {})
    req = urllib.request.Request(url, data=body, headers=head, method="POST")
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def get_json(url: str, timeout: float = 15.0):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


# --------------------------------------------------------------- the agents

#: Each agent gets one job and is judged on it alone.
#:
#: Separate agents rather than one prompt with three paragraphs, because the
#: log records a size_factor per agent: after a few hundred consultations the
#: question "were the news objections worth listening to" has an answer, and an
#: agent that is always wrong can be dropped on evidence. One prompt produces
#: one verdict and no way to attribute it.
AGENTS: list[tuple[str, str]] = [
    (
        "risk",
        """You are the risk advisor on a paper trading desk. A strategy has already decided to take
the trade below. You cannot change its direction, its stop, its target or its price, and you
cannot make it larger. You may only let it through at full size, make it smaller, or refuse it.

Refuse or cut ONLY for a reason visible in what you are shown. Do not refuse because you dislike
the strategy, because the market "feels" uncertain, or because you would have traded differently:
the strategy's edge, if it has one, is not yours to second-guess and this desk has thirty closed
registrations saying that overriding a rule on instinct is how a rule stops being testable.

Good reasons to cut or refuse: the book is already deep in drawdown today; this run has just lost
several in a row and is compounding into it; the position would be far larger than the recent
norm; the stop is implausibly wide or narrow against the bars you can see.""",
    ),
    (
        "news",
        """You are the news advisor on a paper trading desk. The engine already enforces a hard
blackout around scheduled high-impact releases, so a trade reaching you is one the calendar
allowed. Your job is the thing the calendar cannot see.

You cannot change direction, stop, target or price, and you cannot make the trade larger. You may
only allow, cut, or refuse.

Cut or refuse only when the timing itself is the problem: an hour the feed is thin, a session
rollover, the last bars before a weekend, or an event you have specific reason to believe sits
inside the hold. If nothing about the clock is wrong, allow at 1.0 and say so plainly. An advisor
that finds a reason every time is an advisor with no information in it.""",
    ),
    (
        "arbiter",
        """You are the arbiter. You are shown the same trade and what the other advisors said.

Your job is NOT to average them. It is to check that any cut or refusal rests on something
actually visible in the trade, and to strike down an objection that does not. You may be more
permissive than your colleagues, never less: the desk takes the minimum of all verdicts, so a
refusal you disagree with still stands. Say clearly which objection you accept and which you
think is noise, because that sentence is what gets scored later.""",
    ),
]

SCHEMA = """
Answer with JSON and nothing else:

  {"size_factor": <number between 0 and 1>, "reason": "<one sentence, under 200 characters>"}

1.0 means take the trade as the strategy sized it. 0.0 means refuse it. Anything between makes it
smaller by that fraction. There is no way to ask for more than 1.0 and a larger number is clamped.
"""


def brief(entry: dict) -> str:
    """The trade, as the panel sees it. Facts only; no opinion of mine in it."""
    bars = entry.get("bars") or []
    tail = bars[-24:]
    rows = "\n".join(
        f"  {dt.datetime.utcfromtimestamp(t / 1000):%Y-%m-%d %H:%MZ}  O {o:g}  H {h:g}  L {lo:g}  C {c:g}"
        for t, o, h, lo, c in tail
    )
    sig = entry.get("signal_bar") or (0, 0, 0, 0, 0)
    stop = entry.get("stop")
    target = entry.get("target")
    return f"""RUN        {entry['run']}  ({entry.get('label') or 'no label'})
INSTRUMENT {entry['market']}:{entry['tf']}
STRATEGY   {entry['strategy']}  params {json.dumps(entry.get('params', {}), sort_keys=True)}
FILTERS    {', '.join(entry.get('filters') or []) or 'none'}

THE TRADE  {entry['side']} — fills at the open of the next bar
  stop     {stop if stop is not None else 'none'}
  target   {target if target is not None else 'none (the strategy manages its own exit)'}
  the strategy's own reason: {entry.get('reason', '')}

SIGNAL BAR {dt.datetime.utcfromtimestamp(sig[0] / 1000):%Y-%m-%d %H:%MZ}  close {sig[4]:g}

THIS BOOK SO FAR
  advised    {entry.get('book_trades', 0)} trades, net ${entry.get('book_net_usd', 0):+.2f}
  unadvised  {entry.get('shadow_trades', 0)} trades, net ${entry.get('shadow_net_usd', 0):+.2f}
  (the second is the same strategy with no advisor; the gap is what advice has cost or saved)

LAST {len(tail)} BARS, oldest first
{rows}
"""


def ask_cli(prompt: str, model: str, timeout: float) -> str:
    """One decision through the Claude Code CLI, on the account's own plan.

    Runs from a scratch directory on purpose. Started inside the repository it
    would pick up `CLAUDE.md`, the agents and the skills, and a question about
    forty candles would arrive carrying a coding agent's whole working
    context — slower, dearer, and no longer the question that was asked.

    An empty reply RAISES, for the reason the OpenAI branch does: an empty
    string parses downstream as a stand-aside, so a broken call would look
    exactly like a model that declined, forever, while the log showed it
    working.
    """
    exe = shutil.which("claude")
    if not exe:
        raise RuntimeError("no `claude` on PATH; the plan is reached through the Claude Code CLI")
    argv = [
        exe, "-p", "--model", model,
        "--allowed-tools", "",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--system-prompt", CLI_SYSTEM_PROMPT,
        "--output-format", "json",
    ]
    try:
        done = subprocess.run(
            argv, input=prompt.encode("utf-8"), stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            timeout=timeout, cwd=tempfile.gettempdir(),
        )
    except subprocess.TimeoutExpired as e:
        raise RuntimeError(f"{model} did not answer within {timeout:.0f}s") from e
    if done.returncode != 0:
        # BOTH streams. This branch reported stderr alone, and when the CLI
        # started failing it failed with an EMPTY stderr — thirty-two dead
        # calls in a row and not one word anywhere saying why. An error path
        # that can throw away the only evidence is not an error path.
        blob = (done.stdout.decode("utf-8", "replace") + chr(10)
                + done.stderr.decode("utf-8", "replace")).strip()
        raise RuntimeError(
            f"claude exited {done.returncode} [{exe}]: {blob[:400] if blob else 'both streams empty'}"
        )
    try:
        out = json.loads(done.stdout.decode("utf-8", "replace"))
    except ValueError as e:
        raise RuntimeError(f"claude returned unreadable JSON: {done.stdout[:200]!r}") from e
    if out.get("is_error"):
        raise RuntimeError(f"claude reported an error: {str(out.get('result'))[:300]}")
    text = str(out.get("result") or "")
    if not text.strip():
        raise RuntimeError(f"{model} returned no text through the CLI")
    return text


def ask_codex(prompt: str, model: str, timeout: float) -> str:
    """One decision through the Codex CLI, on the account's ChatGPT plan.

    UNVERIFIED as of 2026-09-15: `codex login status` says "Not logged in" on
    this machine, and the login is a browser flow that cannot be completed
    headlessly. The shape below is read from `codex exec --help` of the
    installed binary (0.148.0-alpha.15), not guessed — but no reply has ever
    come back through it, so treat the first live run as the test.

    Locked down the same way the Claude path is, because the same thing is
    true of both: this is an agent runtime being used as a completion, and
    everything it can reach that is not the model is a liability.

      --sandbox read-only     it may not write
      --ignore-user-config    the desktop app's plugins stay out of it
      --ignore-rules          no execpolicy file changes behaviour under us
      --ephemeral             no session files; every bar asked cold
      --skip-git-repo-check   the scratch cwd is not a repository
      -C <scratch>            started in the repo it would read the repo

    The final message is taken from `--output-last-message`, a file, rather
    than scraped out of the JSONL event stream: one documented value beats
    parsing a log whose shape is free to change.
    """
    exe = codex_bin()
    # `codex/gpt-5.6-sol` names the route and the model in one string: the
    # prefix is what sent it here (a bare `gpt-*` still means the metered API),
    # and Codex itself must not see it. Keeping the prefix on the badge is
    # deliberate — the desk then says which model AND which account paid for
    # it, and those are two different facts about the same book.
    if model.startswith("codex/"):
        model = model.split("/", 1)[1]
    with tempfile.TemporaryDirectory() as work:
        out_file = os.path.join(work, "last.txt")
        argv = [
            exe, "exec",
            "--model", model,
            "--sandbox", "read-only",
            "--ignore-user-config",
            "--ignore-rules",
            "--ephemeral",
            "--skip-git-repo-check",
            "--color", "never",
            "-C", work,
            "-o", out_file,
            "-",
        ]
        try:
            done = subprocess.run(
                argv, input=prompt.encode("utf-8"), stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                timeout=timeout, cwd=work,
            )
        except subprocess.TimeoutExpired as e:
            raise RuntimeError(f"{model} did not answer within {timeout:.0f}s") from e
        if done.returncode != 0:
            # Codex prints a banner first and the reason last, and on a missing
            # login it retries ten times before saying anything useful. Scan
            # BOTH streams and name the common case, or the log at 3am says
            # "exited 1" over a workdir listing.
            blob = (done.stdout.decode("utf-8", "replace") + chr(10)
                    + done.stderr.decode("utf-8", "replace"))
            low = blob.lower()
            if "401" in blob or "unauthorized" in low or "not logged in" in low:
                raise RuntimeError(
                    f"codex is not authenticated (401). Run `{exe} login` once, "
                    "in an interactive terminal - it opens a browser."
                )
            tail = chr(10).join(line for line in blob.splitlines() if line.strip())[-400:]
            raise RuntimeError(f"codex exited {done.returncode}: ...{tail}")
        try:
            with io.open(out_file, encoding="utf-8") as fh:
                text = fh.read()
        except OSError as e:
            raise RuntimeError(f"codex wrote no final message: {e}") from e
    if not text.strip():
        raise RuntimeError(f"{model} returned no text through the Codex CLI")
    return text


def ask(prompt: str, model: str, provider: str, api_key: str, timeout: float) -> tuple[str, int]:
    """One model call, either provider. Returns (text, latency_ms); raises on failure."""
    started = time.monotonic()
    spec = PROVIDERS[provider]
    url = spec["url"]

    if spec.get("cli") == "claude":
        text = ask_cli(prompt, model, timeout)
    elif spec.get("cli") == "codex":
        text = ask_codex(prompt, model, timeout)
    elif provider == "anthropic":
        out = post_json(
            url,
            {"model": model, "max_tokens": 512, "messages": [{"role": "user", "content": prompt}]},
            timeout=timeout,
            headers={"x-api-key": api_key, "anthropic-version": "2023-06-01"},
        )
        text = "".join(b.get("text", "") for b in out.get("content", []) if b.get("type") == "text")
    else:
        body = {
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "max_completion_tokens": OPENAI_MAX_TOKENS,
        }
        if model.lower().startswith(REASONING_PREFIXES):
            body["reasoning_effort"] = REASONING_EFFORT
        head = {"Authorization": f"Bearer {api_key}"}

        # Two parameters here are newer than some of the models that accept
        # this endpoint, and which model takes which is a list that would be
        # wrong the week after it was written. So: send the current spelling,
        # and on a 400 that names a parameter, drop or rename that one and try
        # once more. At most two wasted requests on the first call of a run.
        out = None
        for attempt in range(3):
            try:
                out = post_json(url, body, timeout=timeout, headers=head)
                break
            except urllib.error.HTTPError as e:
                detail = e.read().decode("utf-8", "replace")
                if e.code != 400 or attempt == 2:
                    raise RuntimeError(f"{e.code}: {detail[:300]}") from e
                if "reasoning_effort" in detail and "reasoning_effort" in body:
                    body.pop("reasoning_effort")
                elif "max_completion_tokens" in detail and "max_completion_tokens" in body:
                    body["max_tokens"] = body.pop("max_completion_tokens")
                else:
                    raise RuntimeError(f"400: {detail[:300]}") from e
        choices = (out or {}).get("choices") or []
        text = (choices[0].get("message", {}).get("content") or "") if choices else ""
        if not text.strip():
            # Named rather than passed on as an empty string, so the log says
            # what went wrong instead of showing an advisor that agreed.
            spent = (out or {}).get("usage", {}).get("completion_tokens_details", {}).get("reasoning_tokens")
            raise RuntimeError(
                f"{model} returned no text"
                + (f"; it spent {spent} tokens reasoning and had none left to answer with" if spent else "")
            )

    return text, int((time.monotonic() - started) * 1000)


def parse(text: str) -> tuple[float, str]:
    """The number and the sentence, or a full-size allow.

    A reply this cannot read is NOT treated as a refusal. A parser bug that
    silently stopped the desk trading would look exactly like a quiet market,
    and the whole design says the failure mode of an advisor is no advice.
    """
    start, end = text.find("{"), text.rfind("}")
    if start < 0 or end <= start:
        return 1.0, f"unparseable reply, allowed at full size: {text[:120]!r}"
    try:
        obj = json.loads(text[start : end + 1])
        factor = float(obj.get("size_factor", 1.0))
    except (ValueError, TypeError) as e:
        return 1.0, f"unparseable reply, allowed at full size: {e}"
    if factor != factor:  # NaN
        return 1.0, "reply was not a number, allowed at full size"
    return max(0.0, min(MAX_FACTOR, factor)), str(obj.get("reason", ""))[:200]


def rules_only(entry: dict) -> tuple[float, str]:
    """The panel with no model in it: a stand-in, and a control.

    Worth keeping permanently rather than deleting once the models work. It is
    the thing a model advisor has to beat — if a panel of three models cannot
    outscore four lines of arithmetic, the log will say so, and that is exactly
    the kind of question this whole apparatus exists to answer.
    """
    if entry.get("book_net_usd", 0.0) <= -300.0:
        return 0.0, "this book is already 300 down; no new risk today"
    if entry.get("stop") is None:
        return 0.5, "no stop on the entry; half size"
    return 1.0, "nothing in the rules objects"


def consult(entry: dict, panel: list[dict], timeout: float) -> tuple[float, str, list[dict]]:
    """Every agent in turn. Returns (verdict, reason, transcript).

    `panel` is the agents that have a usable key, each carrying its own model,
    provider and key. An empty panel is the arithmetic control.
    """
    facts = brief(entry)
    transcript: list[dict] = []

    if not panel:
        factor, reason = rules_only(entry)
        transcript.append(
            {
                "agent": "rules",
                "model": "none",
                "prompt": facts,
                "response": json.dumps({"size_factor": factor, "reason": reason}),
                "latency_ms": 0,
                "size_factor": factor,
                "reason": reason,
            }
        )
        return factor, reason, transcript

    said: list[str] = []
    for agent in panel:
        earlier = ""
        if said:
            earlier = "\n\nWHAT THE OTHER ADVISORS SAID\n" + "\n".join(said)
        prompt = f"{agent['role']}\n\n{facts}{earlier}\n{SCHEMA}"
        try:
            text, ms = ask(prompt, agent["model"], agent["provider"], agent["key"], timeout)
            factor, reason = parse(text)
        except Exception as e:  # noqa: BLE001
            # One agent failing is not the panel failing. It is recorded as a
            # full-size allow so it cannot silently become a veto, and the
            # error is kept in the transcript where a reader will find it.
            text, ms, factor, reason = f"ERROR: {type(e).__name__}: {e}", 0, 1.0, "agent unavailable"
        transcript.append(
            {
                "agent": agent["name"],
                # The model that actually answered, not the one configured: a
                # per-agent override or a fallback must be visible in the log,
                # because the whole point of keeping this field is to be able
                # to ask later which model said what.
                "model": agent["model"],
                "prompt": prompt,
                "response": text,
                "latency_ms": ms,
                "size_factor": factor,
                "reason": reason,
            }
        )
        said.append(f"  {agent['name']}: {factor:.2f} — {reason}")

    # The minimum, not the mean: any one advisor can refuse and none can
    # overrule a refusal. See the module docstring.
    worst = min(transcript, key=lambda t: t["size_factor"])
    return worst["size_factor"], f"{worst['agent']}: {worst['reason']}", transcript


def dry_log(entry: dict, factor: float, reason: str, transcript: list[dict]) -> None:
    """Record a verdict the desk never saw, in the place it would have gone.

    The server writes `advice.jsonl` when a verdict is posted; a dry run posts
    nothing, so it writes its own line here. Same file, same shape, same reader
    — with `applied: false` and `dry_run: true`, which is the truth and is what
    stops a rehearsal being counted as a decision.
    """
    root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    path = os.path.join(root, "data", "paper", entry["run"], "advice.jsonl")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    record = {
        "kind": "consultation",
        "at": int(time.time() * 1000),
        "intent_id": entry["intent_id"],
        "pending_now": entry["intent_id"],
        "applied": False,
        "dry_run": True,
        "size_factor": factor,
        "raw_size_factor": factor,
        "reason": reason,
        "transcript": transcript,
    }
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(record) + chr(10))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--api", default="http://127.0.0.1:8138")
    ap.add_argument("--poll", type=float, default=20.0, help="seconds between polls")
    ap.add_argument("--timeout", type=float, default=45.0, help="seconds for one model call")
    ap.add_argument("--dry-run", action="store_true",
                    help="decide, print and log locally; post nothing, so no book can be changed")
    ap.add_argument("--rules-only", action="store_true", help="no model calls; use the arithmetic control")
    ap.add_argument("--once", action="store_true", help="one pass, then exit")
    ap.add_argument("--model", default=DEFAULT_MODEL,
                    help=f"the model every agent uses unless overridden (default {DEFAULT_MODEL})")
    ap.add_argument("--provider", choices=sorted(PROVIDERS), default=None,
                    help="force the API for a model name the prefix rule does not recognise")
    ap.add_argument("--agent-model", action="append", default=[], metavar="AGENT=MODEL",
                    help="give one agent its own model, e.g. risk=gpt-4o. Repeatable, and a mixed "
                         "panel is the point: the log records which model said what.")
    args = ap.parse_args()

    overrides = {}
    for pair in args.agent_model:
        if "=" not in pair:
            sys.exit(f"--agent-model wants AGENT=MODEL, got {pair!r}")
        who, what = pair.split("=", 1)
        if who not in {name for name, _ in AGENTS}:
            sys.exit(f"no agent `{who}`; the panel is {', '.join(name for name, _ in AGENTS)}")
        overrides[who] = what

    panel: list[dict] = []
    if not args.rules_only:
        for name, role in AGENTS:
            model = overrides.get(name, args.model)
            try:
                provider = args.provider or provider_of(model)
            except ValueError as e:
                sys.exit(str(e))
            env = PROVIDERS[provider]["env"]
            # A keyless provider (the plan, through the CLI) has nothing to
            # look up; only a metered one can be missing its key.
            key = os.environ.get(env) if env else ""
            # `env` None means the provider needs no key at all (a plan, through
            # a CLI). Only a METERED provider can be missing one — testing the
            # empty key alone dropped every plan-backed agent and silently fell
            # the whole panel back to the arithmetic control.
            if env and not key:
                # Named, not guessed at: a panel silently one agent short is a
                # panel whose verdicts mean something different from what the
                # log will say they mean.
                print(f"  {name}: no {env} for {model} — this agent will not run", flush=True)
                continue
            panel.append({"name": name, "role": role, "model": model, "provider": provider, "key": key})

    if not args.rules_only and not panel:
        print("no usable key for any agent; falling back to the arithmetic control", flush=True)

    who = "rules only" if not panel else ", ".join(f"{a['name']}:{a['model']}" for a in panel)
    print(
        f"advisor: polling {args.api}/api/paper/pending every {args.poll:g}s; {who}"
        f"{' (dry run, posting nothing)' if args.dry_run else ''}",
        flush=True,
    )

    seen: set[str] = set()
    while True:
        try:
            entries = get_json(f"{args.api}/api/paper/pending")
        except Exception as e:  # noqa: BLE001
            print(f"pending: {type(e).__name__}: {e}", flush=True)
            entries = []

        for entry in entries:
            intent = entry["intent_id"]
            # Already answered, by this process or another. Asking twice would
            # cost model calls and could only overwrite one verdict with
            # another about the same unchanged facts.
            if intent in seen or entry.get("advised"):
                continue
            factor, reason, transcript = consult(entry, panel, args.timeout)
            seen.add(intent)
            stamp = dt.datetime.now(dt.timezone.utc).strftime("%H:%M:%SZ")
            verdict = "VETO" if factor <= 0 else ("cut" if factor < 1 else "allow")
            print(f"{stamp} {entry['run']:16s} {entry['side']:5s} {verdict:5s} {factor:.2f} — {reason}", flush=True)
            if args.dry_run:
                # A dry run that leaves no trace defeats its own purpose. The
                # point of running the panel without power is to find out
                # whether it is sane BEFORE it can refuse a real entry, and
                # that judgement needs the same record a live run would leave.
                # Written beside the run's own log, with `applied: false` and a
                # `dry_run` flag, so advisor_review.py reads it exactly like
                # any other consultation and nothing here can be mistaken for
                # something the desk acted on.
                dry_log(entry, factor, reason, transcript)
                continue
            try:
                reply = post_json(
                    f"{args.api}/api/paper/advice",
                    {
                        "run": entry["run"],
                        "intent_id": intent,
                        "size_factor": factor,
                        "reason": reason,
                        "transcript": transcript,
                    },
                )
                if not reply.get("accepted"):
                    # The intent filled while the panel was thinking. Logged on
                    # the server either way; here it is a latency measurement.
                    print(f"   too late for {intent}; the bar had already filled", flush=True)
            except Exception as e:  # noqa: BLE001
                print(f"   advice POST failed: {type(e).__name__}: {e}", flush=True)

        # Keep the memory of what has been answered from growing without bound
        # over a long run; an intent id names a bar and never comes back.
        if len(seen) > 4000:
            seen = set(list(seen)[-1000:])
        if args.once:
            return 0
        time.sleep(args.poll)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(0)
