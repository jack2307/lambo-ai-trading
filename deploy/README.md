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
