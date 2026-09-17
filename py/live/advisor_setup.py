"""Configure how the advisor authenticates: a metered API key, or a plan through a CLI.

    python py/live/advisor_setup.py                 # the menu
    python py/live/advisor_setup.py status
    python py/live/advisor_setup.py key anthropic   # 1 - paste a key
    python py/live/advisor_setup.py login claude-cli  # 2 - sign in to the plan
    python py/live/advisor_setup.py test anthropic
    python py/live/advisor_setup.py clear anthropic
    python py/live/advisor_setup.py logout claude-cli

Two ways to pay, and the advisor already speaks both: a **metered** provider
bills per token against an API key, and a **plan** provider spends a
subscription through a vendor CLI that is already signed in. The panel picks a
provider from the model's name (`claude-*` -> anthropic, `codex/gpt-5` ->
codex-cli), so this file configures the credential, never the routing.

Three things it does that a plain key prompt does not, each because of
something in this repository:

  * **It proves the credential before storing it.** A key that is saved and
    does not work is worse than no key: the panel drops that agent with one
    line in a log and goes on deciding with fewer voices than the record says.
    So every save makes one real call first and refuses on failure.

  * **It edits one section of `config/local.toml` and leaves the rest alone.**
    That file already holds the Telegram bot token. Rewriting it to store an
    API key would be a config tool that destroys configuration.

  * **It checks the key is reachable from where it is used.** `key_for` only
    consults the file for a provider that declares a `config_section`, and the
    panel used to read `os.environ` directly and bypass `key_for` entirely.
    Both are fixed in `advisor.py`; `status` re-checks the second one rather
    than trusting that it stays fixed.

Secrets are never printed: a stored key is shown masked, and the prompt does
not echo.
"""

from __future__ import annotations

import argparse
import getpass
import io
import json
import os
import re
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import advisor  # noqa: E402

ROOT = advisor.ROOT
LOCAL = os.path.join(ROOT, "config", "local.toml")

#: The vendor CLIs, and how each one is asked about its own session. Taken from
#: `claude auth --help` on 2026-09-17 rather than assumed; `codex` is not
#: installed on this machine and its shape is recorded from the Codex docs and
#: marked unverified, the same way `advisor.ask_codex` marks it.
CLI_TOOLS = {
    "claude-cli": {
        "bin": "claude",
        "status": ["auth", "status", "--json"],
        "login": ["auth", "login"],
        "logout": ["auth", "logout"],
        "verified": True,
    },
    "codex-cli": {
        "bin": "codex",
        "status": ["login", "status"],
        "login": ["login"],
        "logout": ["logout"],
        "verified": False,
    },
}

#: What to call when proving a credential works. Cheap on purpose: this is an
#: auth check, not a capability check, and any non-empty reply passes.
TEST_MODEL = {
    "anthropic": "claude-haiku-4-5-20251001",
    "openai": "gpt-5",
    "deepseek": "deepseek-flash",
    "claude-cli": "haiku",
    "codex-cli": "codex/gpt-5",
}

PROBE = "Reply with the single word OK and nothing else."


def kind(provider: str) -> str:
    return "plan" if advisor.PROVIDERS[provider].get("cli") else "metered"


def mask(secret: str) -> str:
    if not secret:
        return "-"
    if len(secret) <= 12:
        return "*" * len(secret)
    return f"{secret[:6]}...{secret[-4:]} ({len(secret)} chars)"


# ------------------------------------------------------------------ the file


def read_local() -> list[str]:
    try:
        return io.open(LOCAL, encoding="utf-8").read().splitlines()
    except OSError:
        return []


def write_local(lines: list[str]) -> None:
    os.makedirs(os.path.dirname(LOCAL), exist_ok=True)
    io.open(LOCAL, "w", encoding="utf-8", newline="\n").write("\n".join(lines).rstrip() + "\n")


def set_value(section: str, key: str, value: str) -> None:
    """Set `key` inside `[section]`, leaving every other line byte-identical.

    Line-based rather than a TOML round-trip on purpose: a parser that rewrites
    the file would drop the comments, and the comments in `config/` are load
    bearing in this repository.
    """
    if '"' in value or "\n" in value:
        raise ValueError("a credential containing a quote or a newline is not storable here")
    header = f"[{section}]"
    out: list[str] = []
    inside = False
    written = False
    for line in read_local():
        s = line.strip()
        if s.startswith("[") and s.endswith("]"):
            if inside and not written:
                out.append(f'{key} = "{value}"')
                written = True
            inside = s == header
            out.append(line)
            continue
        if inside and re.match(rf"\s*{re.escape(key)}\s*=", line):
            if not written:
                out.append(f'{key} = "{value}"')
                written = True
            continue          # the old line is dropped, not kept above the new one
        out.append(line)
    if inside and not written:
        out.append(f'{key} = "{value}"')
        written = True
    if not written:
        if out and out[-1].strip():
            out.append("")
        out += [header, f'{key} = "{value}"']
    write_local(out)


def drop_value(section: str, key: str) -> bool:
    header = f"[{section}]"
    out: list[str] = []
    inside = False
    removed = False
    for line in read_local():
        s = line.strip()
        if s.startswith("[") and s.endswith("]"):
            inside = s == header
            out.append(line)
            continue
        if inside and re.match(rf"\s*{re.escape(key)}\s*=", line):
            removed = True
            continue
        out.append(line)
    if removed:
        write_local(out)
    return removed


def gitignored() -> bool:
    try:
        text = io.open(os.path.join(ROOT, ".gitignore"), encoding="utf-8").read()
    except OSError:
        return False
    return any(l.strip() in ("config/local.toml", "/config/local.toml") for l in text.splitlines())


# ------------------------------------------------------------------- the CLIs


def cli_status(provider: str) -> tuple[bool, str]:
    """(signed in, what the tool said). Never raises."""
    tool = CLI_TOOLS[provider]
    exe = shutil.which(tool["bin"])
    if not exe:
        return False, f"`{tool['bin']}` is not on PATH"
    try:
        done = subprocess.run([exe] + tool["status"], stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, timeout=30)
    except Exception as e:                                   # noqa: BLE001
        return False, f"{tool['bin']} {' '.join(tool['status'])} failed: {e}"
    blob = done.stdout.decode("utf-8", "replace").strip()
    try:
        info = json.loads(blob)
    except ValueError:
        ok = done.returncode == 0 and "not logged in" not in blob.lower()
        return ok, (blob.splitlines() or [""])[0][:120]
    if not info.get("loggedIn"):
        return False, "not signed in"
    bits = [str(info.get("authMethod") or "")]
    if info.get("subscriptionType"):
        bits.append(f"plan {info['subscriptionType']}")
    if info.get("email"):
        bits.append(str(info["email"]))
    return True, ", ".join(b for b in bits if b)


def cli_run(provider: str, what: str) -> int:
    """Hand the terminal to the vendor CLI for `login` / `logout`.

    Not captured, not piped, not timed out. A sign-in is a browser round trip
    with a code to paste back, and a subprocess whose stdio is a pipe cannot
    complete one — it hangs forever on a prompt nobody can see. So this gives
    the CLI the real terminal and gets out of the way.
    """
    tool = CLI_TOOLS[provider]
    exe = shutil.which(tool["bin"])
    if not exe:
        print(f"  `{tool['bin']}` is not on PATH — install it first, then run this again.")
        return 3
    argv = [exe] + tool[what]
    print(f"  handing the terminal to: {' '.join(argv)}")
    if not tool["verified"]:
        print(f"  note: this repository has never completed a {tool['bin']} sign-in; "
              f"the flags come from its docs, not from a run here.")
    try:
        return subprocess.call(argv)
    except KeyboardInterrupt:
        return 130


# --------------------------------------------------------------- the checking


def probe(provider: str, model: str | None, timeout: float, key: str | None = None) -> tuple[bool, str]:
    """One real call. Any non-empty answer is a pass.

    The question is deliberately trivial: this proves the credential is
    accepted and the model replies, which is what a setup tool can promise. It
    does not prove the model is any good at the panel's actual job.
    """
    model = model or TEST_MODEL.get(provider) or advisor.DEFAULT_MODEL
    if key is None:
        key = advisor.key_for(provider)
    if advisor.PROVIDERS[provider].get("env") and not key:
        return False, "no key to test"
    usage: dict = {}
    try:
        text, ms = advisor.ask(PROBE, model, provider, key, timeout, usage)
    except Exception as e:                                   # noqa: BLE001
        return False, str(e).splitlines()[0][:220]
    # `provider` is passed, and it is the whole difference between this line
    # being true and being a lie.
    #
    # `cost_of` decides by ROUTE and not by model name - the same
    # `claude-opus-5` is quota through the CLI and dollars through the API -
    # and without the third argument it falls back to deriving the route from
    # the name, which sends every Claude model to the plan. So a probe run as
    # `--provider anthropic` made one REAL metered call and then reported
    # "billed to a plan, not per token" at it.
    #
    # Small in money: one call, and the probe asks for a single word. Not
    # small in kind, and this is the worst file in the desk for it - a setup
    # tool is what someone runs BECAUSE they do not yet know what a route
    # costs. The number it prints is the answer to the question they came
    # with.
    cost = advisor.cost_of(model, usage, provider)
    bits = [f"{model} answered in {ms} ms"]
    if usage:
        bits.append(f"{usage.get('input', 0)} in / {usage.get('output', 0)} out")
    # Three states, not two. `cost is None` used to print "billed to a plan"
    # for both a plan call and a METERED call whose model has no entry in
    # advisor.PRICES - so a probe that spent money on an unpriced model said
    # it had spent none. The same shape as the executor's snapshot learning to
    # tell a demo from an account it cannot see: an answer nobody has is not
    # the same as a no.
    if cost is not None:
        bits.append(f"${cost:.5f}")
    elif kind(provider) == "plan":
        bits.append("billed to a plan, not per token")
    else:
        bits.append("metered, and this call was billed - no price for "
                    f"{model!r} in advisor.PRICES, so the amount is unknown")
    return True, "; ".join(bits)


# -------------------------------------------------------------- the commands


def survey() -> list[dict]:
    """One row per provider: how it would authenticate right now.

    No secret is ever in a row — only a mask. This is what the HTTP API serves
    to a browser, and a credential store that can hand a credential back is not
    a credential store.
    """
    rows = []
    for name in advisor.PROVIDERS:
        spec = advisor.PROVIDERS[name]
        row = {"provider": name, "kind": kind(name), "masked": "",
               "can_store": bool(spec.get("config_section"))}
        if kind(name) == "plan":
            ok, detail = cli_status(name)
            tool = CLI_TOOLS[name]
            row.update({"source": tool["bin"], "ready": ok, "detail": detail,
                        "login_command": f"{tool['bin']} {' '.join(tool['login'])}",
                        "verified": tool["verified"]})
        else:
            env = spec.get("env")
            from_env = os.environ.get(env or "") or ""
            stored = advisor.key_for(name)
            detail = ""
            if from_env:
                source, detail = f"env {env}", mask(from_env)
            elif stored:
                source, detail = "local.toml", mask(stored)
            else:
                source, detail = "-", f"no key (env {env} or config/local.toml)"
            if from_env and stored and from_env != stored:
                detail += "  [env wins over the stored key]"
            row.update({"source": source, "ready": bool(stored), "detail": detail,
                        "masked": mask(stored) if stored else "", "env": env,
                        "env_overrides": bool(from_env),
                        "login_command": ""})
        rows.append(row)
    return rows


def cmd_status(args) -> int:
    rows = survey()
    if getattr(args, "json", False):
        print(json.dumps({"file": LOCAL, "gitignored": gitignored(), "providers": rows}))
        return 0
    print(f"advisor credentials   (file: {LOCAL})")
    if not gitignored():
        print("  WARNING: config/local.toml is not in .gitignore. Do not store a key until it is.")
    print(f"  {'provider':<12s} {'kind':<8s} {'source':<14s} {'ready':<6s} detail")
    for r in rows:
        print(f"  {r['provider']:<12s} {r['kind']:<8s} {r['source']:<14s} "
              f"{'yes' if r['ready'] else 'NO':<6s} {r['detail']}")
    if args.test:
        print("\n  proving each ready credential with one real call:")
        for name in advisor.PROVIDERS:
            if kind(name) == "plan" and not cli_status(name)[0]:
                continue
            if kind(name) == "metered" and not advisor.key_for(name):
                continue
            ok, detail = probe(name, None, args.timeout)
            print(f"  {name:<12s} {'ok ' if ok else 'FAIL'} {detail}")
    return 0


def cmd_key(args) -> int:
    provider = args.provider
    as_json = getattr(args, "json", False)

    def done(code: int, stored: bool, detail: str) -> int:
        if as_json:
            print(json.dumps({"provider": provider, "stored": stored, "detail": detail}))
            return 0 if stored else code
        print(f"  {detail}")
        return code

    if kind(provider) == "plan":
        return done(2, False, f"{provider} is a plan, reached through "
                              f"`{CLI_TOOLS[provider]['bin']}`. Sign in instead — it takes no key.")
    section = advisor.PROVIDERS[provider].get("config_section")
    if not section:
        return done(2, False, f"{provider} declares no config_section in advisor.PROVIDERS, "
                              f"so a stored key would never be read. Fix that first.")
    if not gitignored():
        return done(2, False, "refusing to store a key: config/local.toml is not in .gitignore.")

    if args.stdin:
        key = sys.stdin.read().strip()
    else:
        print(f"paste the {provider} key (input is hidden; nothing is echoed):")
        key = getpass.getpass("  key: ").strip()
    if not key:
        return done(1, False, "nothing entered; unchanged.")

    if not as_json:
        print(f"  proving it with one call to {args.model or TEST_MODEL.get(provider)} before storing...")
    ok, detail = probe(provider, args.model, args.timeout, key=key)
    if not ok and not args.force:
        return done(1, False, f"NOT stored — {detail}. A key that does not answer would drop "
                              f"this agent from the panel with one line in a log.")

    set_value(section, "api_key", key)
    note = f"stored under [{section}] ({mask(key)}); {detail}"
    env = advisor.PROVIDERS[provider].get("env")
    if env and os.environ.get(env):
        note += f". NOTE: {env} is also set in this environment and WINS over the file"
    return done(0, True, note)


def cmd_login(args) -> int:
    provider = args.provider
    if kind(provider) != "plan":
        print(f"{provider} is metered — it needs a key, not a sign-in. Use `key {provider}`.")
        return 2
    ok, detail = cli_status(provider)
    print(f"  before: {'signed in' if ok else 'not signed in'} — {detail}")
    if ok and not args.force:
        print("  already signed in; nothing to do. Pass --force to sign in again.")
        return 0
    code = cli_run(provider, "login")
    ok, detail = cli_status(provider)
    print(f"  after:  {'signed in' if ok else 'not signed in'} — {detail}")
    if not ok:
        return code or 1
    ok2, d2 = probe(provider, args.model, args.timeout)
    print(f"  {'ok  ' if ok2 else 'FAIL'} {d2}")
    return 0 if ok2 else 1


def cmd_logout(args) -> int:
    if kind(args.provider) != "plan":
        return cmd_clear(args)
    cli_run(args.provider, "logout")
    ok, detail = cli_status(args.provider)
    print(f"  now: {'still signed in' if ok else 'signed out'} — {detail}")
    return 0


def cmd_clear(args) -> int:
    section = advisor.PROVIDERS[args.provider].get("config_section")
    env = advisor.PROVIDERS[args.provider].get("env")
    if not section:
        detail, gone = f"{args.provider} stores nothing in the file.", False
    else:
        gone = drop_value(section, "api_key")
        detail = f"{'removed' if gone else 'nothing stored'} under [{section}] in {LOCAL}"
        if env and os.environ.get(env):
            detail += f". NOTE: {env} is still set in this environment, so this provider still works"
    if getattr(args, "json", False):
        print(json.dumps({"provider": args.provider, "removed": gone, "detail": detail}))
        return 0
    print(f"  {detail}")
    return 0 if section else 1


def cmd_test(args) -> int:
    ok, detail = probe(args.provider, args.model, args.timeout)
    if getattr(args, "json", False):
        print(json.dumps({"provider": args.provider, "ok": ok, "detail": detail}))
        return 0
    print(f"  {args.provider}: {'ok  ' if ok else 'FAIL'} {detail}")
    return 0 if ok else 1


# ------------------------------------------------------------------ the menu


def pick(prompt: str, options: list[str]) -> str | None:
    for i, o in enumerate(options, 1):
        print(f"    {i}) {o}")
    raw = input(f"  {prompt} ").strip()
    if not raw.isdigit() or not (1 <= int(raw) <= len(options)):
        return None
    return options[int(raw) - 1]


def menu(args) -> int:
    while True:
        print("\nConfigure the advisor's credentials")
        print("  1) Enter an API key      (metered: billed per token)")
        print("  2) Sign in to a CLI      (a plan you already pay for)")
        print("  3) Status")
        print("  4) Test a provider with one real call")
        print("  5) Remove a key / sign out")
        print("  0) Quit")
        choice = input("  choose: ").strip()

        if choice in ("0", "q", ""):
            return 0
        if choice == "3":
            args.test = False
            cmd_status(args)
        elif choice == "1":
            metered = [p for p in advisor.PROVIDERS if kind(p) == "metered"]
            p = pick("which provider?", metered)
            if p:
                args.provider, args.stdin, args.force = p, False, False
                cmd_key(args)
        elif choice == "2":
            plans = [p for p in advisor.PROVIDERS if kind(p) == "plan"]
            p = pick("which CLI?", plans)
            if p:
                args.provider, args.force = p, False
                cmd_login(args)
        elif choice == "4":
            p = pick("which provider?", list(advisor.PROVIDERS))
            if p:
                args.provider = p
                cmd_test(args)
        elif choice == "5":
            p = pick("which provider?", list(advisor.PROVIDERS))
            if p:
                args.provider = p
                cmd_logout(args)
        else:
            print("  not a choice.")


def main() -> int:
    ap = argparse.ArgumentParser(description="configure how the advisor authenticates")
    ap.add_argument("--model", default=None, help="model to prove the credential with")
    ap.add_argument("--timeout", type=float, default=60.0)
    # The machine interface. `fd-api` shells out to this file rather than
    # reimplementing five provider adapters in Rust, so that the rules about
    # what a credential is and when it is proven live in exactly one place.
    ap.add_argument("--json", action="store_true", help="emit one JSON object instead of a table")
    sub = ap.add_subparsers(dest="cmd")

    s = sub.add_parser("status", help="what is configured, and from where")
    s.add_argument("--test", action="store_true", help="also make one real call per credential")
    # The same both-positions treatment as the subcommands below, and for the
    # same reason. `status` is built outside that loop, so leaving it out here
    # is exactly how `status --json` kept failing after the others were fixed.
    s.add_argument("--json", action="store_true",
                   default=argparse.SUPPRESS, help=argparse.SUPPRESS)
    s.add_argument("--model", default=argparse.SUPPRESS, help=argparse.SUPPRESS)
    s.add_argument("--timeout", type=float,
                   default=argparse.SUPPRESS, help=argparse.SUPPRESS)
    s.set_defaults(fn=cmd_status)

    for name, fn, helptext in (
        ("key", cmd_key, "store an API key for a metered provider"),
        ("login", cmd_login, "sign a vendor CLI in to its plan"),
        ("logout", cmd_logout, "sign a vendor CLI out"),
        ("clear", cmd_clear, "remove a stored API key"),
        ("test", cmd_test, "one real call, to prove a credential"),
    ):
        p = sub.add_parser(name, help=helptext)
        # `--json` is accepted BOTH before and after the subcommand. argparse
        # will not take a parent's flag after the subcommand, and a caller that
        # writes `status --json` gets "unrecognized arguments: --json" — a
        # failure whose message points at the flag rather than at its position,
        # which cost an afternoon to read correctly. Declaring it on both
        # parsers makes the order stop mattering.
        # `default=SUPPRESS` is the load-bearing half: without it the subparser
        # writes its OWN default into the same namespace and silently erases a
        # value the parent already parsed, so `--json status` would stop
        # working the moment `status --json` started. With it, the attribute is
        # set only when the flag is actually present.
        p.add_argument("--json", action="store_true",
                       default=argparse.SUPPRESS, help=argparse.SUPPRESS)
        p.add_argument("--model", default=argparse.SUPPRESS, help=argparse.SUPPRESS)
        p.add_argument("--timeout", type=float,
                       default=argparse.SUPPRESS, help=argparse.SUPPRESS)
        p.add_argument("provider", choices=sorted(advisor.PROVIDERS))
        if name == "key":
            p.add_argument("--stdin", action="store_true", help="read the key from stdin, not a prompt")
            p.add_argument("--force", action="store_true", help="store it even if the test call fails")
        if name == "login":
            p.add_argument("--force", action="store_true", help="sign in again even if already signed in")
        p.set_defaults(fn=fn)

    args = ap.parse_args()
    if not getattr(args, "cmd", None):
        args.test = False
        return menu(args)
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
