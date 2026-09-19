# Running the desk on a server

Two things move to a server and one thing stays behind.

**Moves:** the live side — the API, the paper books, the AI traders, the
pollers that read MetaTrader, the mirrors that copy a book into a broker
account, the Telegram watch. All of it has to be up when the market is, which
is the whole reason for the move.

**Stays:** research. Backtest sweeps are the only part that wants many cores,
they run for minutes not for weeks, and they read a 394 MB Parquet store the
live desk never touches. A server that could also sweep would be a bigger
server paid for by the month to do something for an hour a week.

## The server has to be Windows

Nine files import `MetaTrader5`, which exists only on Windows and only talks to
a running MetaTrader terminal. Two of them are load-bearing: `mt5_bars.py` is
where every bar comes from, and `mt5_executor.py` is the only code in this
repository that can place an order.

That rules out splitting the desk across a cheap Linux box and a Windows one.
The pollers are the feed, so the Windows machine would stay in the critical
path, and the network hop between them would become a way for the desk to go
blind that it does not currently have.

## First time

A fresh Windows Server has no git, and `bootstrap.ps1` lives in the
repository that needs git to clone. That circle has to be broken by hand
exactly once: copy `deploy\first-run.ps1` across - dragging it over an RDP
session is fine - and run it from an **elevated** PowerShell.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File first-run.ps1
```

It installs git (resolved from the releases API, not a pinned URL that
rots), clones to `C:\flowdesk`, and hands over to the bootstrap. A private
repository asks for a GitHub login at the clone and the credential manager
opens a browser, which is why this wants an RDP session rather than a
headless one.

Every machine after the first is just:

```powershell
git clone <repo> C:\flowdesk
cd C:\flowdesk
powershell -NoProfile -ExecutionPolicy Bypass -File deploy\bootstrap.ps1 -WithToolchain
```

Run it from an **elevated** PowerShell. It installs Python 3.9.13 into
`C:\Python39` exactly — every launcher hard-codes that path — plus the pinned
packages, Rust, and the directory layout. Then it checks the things that have
actually gone wrong before and prints what is left.

Five of those it cannot do, because they need credentials or a decision:

1. **Install MetaTrader 5 from Vantage's own download page**, not MetaQuotes'.
   The broker build already knows the server names.
2. **Copy that install to `C:\MT5-demo`** for the demo account, and copy
   `servers.dat` across with it. One terminal holds one account, so reading
   live prices while mirroring into a demo needs two installations. A copied
   terminal starts with an almost empty server list, cannot resolve the
   broker's server name, and then the login *does nothing*: no error, no
   dialog, no log line. The bootstrap checks the file size and warns, because
   this one cost an afternoon.
3. **Log both terminals in, and turn on Algo Trading.** Without it every
   `order_send` returns `10027 AutoTrading disabled by client` while the
   account, the login and the connection are all fine. Put
   `[Experts] AllowLiveTrading=1` in the start config so it survives a restart.
4. **Log in the Claude and Codex CLIs.** They authenticate against the account
   *plan* through a browser, not with an API key.
5. **Copy `config\local.toml`** by hand. It holds the Telegram token and the
   DeepSeek key, and it is deliberately in neither the repository nor the zip.

Then write `config\accounts.toml` for this machine. The copy from home travels
as `accounts.toml.from-home` so its comments arrive without anyone starting
executors against terminal paths that do not exist here.

## Every time after that

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File deploy\update.ps1
```

Pulls, rebuilds, restarts. The order matters and the script keeps it: the
executors go down **first** and come up **last**. An executor reading an
fd-api that is mid-restart sees no book at all, and its rule for "the book is
flat and the account is not" is to close the position — so a restart with the
mirrors still running would flatten every live trade on the way past.

It deliberately does **not** start the mirrors. Sending real orders after an
unattended rebuild should be a decision someone makes while reading the
output, not something a script did at three in the morning.

## Sunday reopen

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File deploy\sunday-reopen.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File deploy\sunday-reopen.ps1 -Restart
```

**When.** After the Sunday reopen at 21:00 UTC, when a change to
`mt5_executor.py` has to reach the running mirrors — they keep the Python they
were launched with, and `update.ps1 -ServerOnly` says so and does not restart
them. It exists for one such change in particular: the weekend backstop
`--weekend-flat`, which is merged and does not exist until the executors
restart. `docs/decisions/2026-09-19-weekend-flat-never-fires.md` is the whole
story and this script is the second half of it.

**Where the cut is typed.** `[prices] weekend_flat` in
`config\accounts.toml`, currently `"20:45"`, and nowhere else.
`start_executors.ps1` reads that key itself and passes
`--weekend-flat=<value>` to every executor it starts; this script reads the
same key for the value it then holds those processes to. So the November DST
change — 20:45Z becomes 21:45Z once New York's close moves to 22:00Z, before
Friday 6 November — is **one edit in one file**. An absent key means the
executors keep their own argparse default and nothing is passed, which is how
a registry written before the key behaves; a malformed one makes `accounts.py`
refuse, and the launcher stops before it has killed a single executor.

**What it refuses to do.** It will not act while the executors' own weekend
window is in force — Friday from the cut, all Saturday, Sunday before 21:00
UTC — because a mirror started inside that window closes every position it
finds into a shut market, fails, and retries at every poll until Monday. That
refusal has **no override**. And it will not act on a clock alone: the reopen
slips, and a holiday Sunday has a clock and no tape. So it also wants a quote
that has *changed* within the last three minutes — `/api/paper/m1`'s
`last_tick_ms`, which a duplicate does not advance, so a poller republishing a
frozen weekend quote cannot satisfy it — on top of a live bar on
`/api/paper/status`. `-AllowStaleFeed` overrides that second check and nothing
else.

Without `-Restart` it reports and stops: what the account holds, with each
position's entry, stop and unrealised P&L **in the account's own currency**
(USC on the cent account, where 100 USC is one dollar), which book each one
belongs to, what the books themselves think they hold, and the weekend gap
that has just happened — Friday's close against the first price of the new
week, in points and in ATR units, against the stop the books size to.

**`-Restart` sends real orders.** It re-launches through
`start_executors.ps1 -Account vantage-cent -AllowReal -Live`, detached through
`Win32_Process.Create` because a session-bound launch is why every AI trader
died at 17:00Z on 2026-09-18. Afterwards it verifies that every executor came
back, that the count matches the registry's mirrored books, that each one
**adopted** the position it found rather than opening a second one, and that
layer two is in force — and it fails loudly, per book, rather than quietly.

`deploy\sunday-reopen-selftest.ps1` drives the two refusals with injected
clocks and quote ages, against the same table of instants as section 13 of
`py\live\size_guard_selftest.py`, and drives the registry key through the real
`accounts.py` — present, absent, and malformed. It touches no network, no
process and no account.

## Cutting over from the home desk

The one rule: **two machines must never mirror one account.** Each launcher
holds a system-wide mutex, which stops two copies on one machine and does
nothing at all across two. Two executors on one account is two copies of every
order.

1. Set the server up and run the mirrors with `-DryRun`. Watch a bar or two:
   the books it reconciles against should match what the home desk shows.
2. Stop the home desk.
3. Copy `data\paper` across. Not before — it changes every bar, and a copy
   taken hours earlier would put the server a day behind the account it is
   about to mirror.
4. Start the server's mirrors with `-Live`.

## Without git

`deploy\pack.ps1` builds at home and packs 22 MB, for a machine that cannot
reach the repository. It leaves out the 29 GB of debug build output, the
`node_modules`, and the historical Parquet, and it strips `local.toml` and
`guards.toml` — the second because it is this machine's *runtime override* of
the risk rules, and a server starting under rules `config/default.toml` does
not state is exactly what every guard change is recorded to prevent.
