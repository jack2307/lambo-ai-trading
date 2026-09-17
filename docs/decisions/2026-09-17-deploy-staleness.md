# Every artefact the deploy moves can be stale, and nothing on the far side disagrees

**Written:** 2026-09-17
**Status:** DRAFT — a specification, not a change. No code in this document has
been written and nothing here has been deployed. It names where an old artefact
can reach the VPS, or be started there, while every check that follows passes.
**Prompted by:** a build made tonight with `cargo` missing from PATH.
`ls target/release/fd-api.exe` showed a plausible 13 MB binary. It was from
before the day's changes and only its mtime said so.

## Why mtime is not evidence

It is worse than no evidence, and this is the reason the incident is worth a
document. `Copy-Item` stamps the DESTINATION with the time of the copy. So a
binary built three days ago and copied to the server tonight has tonight's
mtime on the server, and the one number a reader instinctively checks says
"fresh" precisely when the file is not. The same is true of `ui/dist` copied as
a directory, and of a zip unpacked on the far side.

## What the deploy actually moves, and how each part travels

| Artefact | Travels by | In git? | Can differ from HEAD? |
|---|---|---|---|
| `target/release/fd-api.exe` | copied by hand | **no** (`.gitignore:/target`) | yes, silently |
| `ui/dist/` | copied by hand | **no** (`.gitignore:ui/dist/`) | yes, silently |
| `py/live/*.py` | `git pull` | yes | no — but the RUNNING process is older |
| `config/default.toml` | `git pull` | yes | no |
| `config/guards.toml` | never | **no**, by design | always; it is the runtime layer |
| `config/local.toml` | never | **no** | always, if it exists |
| installed Python deps | never | `requirements.txt` is | yes |
| `deploy/flowdesk-*.zip` | copied | **no** (`.gitignore`) | yes; dated names invite the wrong one |

Two of the three things that decide what the desk DOES — the binary and the
client — are the two that `git pull` cannot touch. The command that reports a
version range (`update.ps1`, `$before -> $after`) is reporting the one artefact
class that was never in question.

## Findings

Each is: where an old artefact survives, and the evidence that would have
caught it. Ranked by what it costs.

### 1. The readiness probe proves something is listening, not that it is new

`deploy/update.ps1` starts `target\release\fd-api.exe` and then polls
`http://127.0.0.1:8138/api/paper/status` for up to 30 s, accepting any 200.
A stale binary answers that endpoint perfectly — it is the same endpoint it has
always served. The probe cannot fail for the reason we care about.

**Evidence that would catch it:** probe `GET /api/version` instead, and require
the returned git hash to EQUAL the hash the deploy just pulled. A 200 is not
the check; equality is.

### 2. A running `.exe` cannot be overwritten on Windows, and the failure is quiet in a by-hand sequence

The staged sequence is pull → swap `fd-api.exe` and `ui/dist` → restart the
task → restart executors. If fd-api is still running when the swap happens,
the copy fails with "being used by another process". In a hand-typed sequence
that line scrolls past; the restart then restarts the OLD binary, and every
check afterwards passes because the old binary is healthy.

**Evidence:** hash the destination file after the copy and compare it to the
source. Not "the copy command was typed" — copy, then verify. Better, and it
subsumes this: check the version of what is ANSWERING (finding 1), which is
true regardless of which file on disk the scheduled task actually points at.

### 3. `ui/dist` is read from disk at runtime, so a stale client is served forever

`crates/fd-api/src/main.rs:19` — `let ui = PathBuf::from(arg("ui", "ui/dist"))`.
The bundle is not embedded in the binary; it is a directory the process reads.
A stale `ui/dist` is internally consistent — `index.html` references its own
hashed bundle and that file is present — so nothing 404s, nothing warns, and
the desk serves last week's client from this week's binary.

Browser caching is NOT the mechanism here and should not be blamed for it:
`crates/fd-api/src/lib.rs:97` sends `no-cache` for HTML only, which is correct —
hashed assets are immutable, so the reload path is sound. A stale dist defeats
that by being stale at the source. (An open tab that is never reloaded is a
separate, real, and much smaller problem.)

**Evidence:** the client's equivalent of a version endpoint is the bundle hash
in the SERVED `index.html`. Fetch `/`, extract `src="/assets/index-*.js"`, and
compare against the filename present in the `ui/dist` that was just built. This
is exactly the check that was done by hand tonight, and it caught a real error —
the first attempt grepped the OLDEST `index-*.js` in the directory, because
`ls | head -1` is not "newest" and twelve generations are kept. Compare against
the REFERENCED name, never against a directory listing.

### 4. `-SkipBuild` is the short path to deploying an old binary

By design, and defensible — but combined with findings 1 and 2 it means the
flag turns off the only step that would have made the artefact current, while
leaving every downstream check green.

**Evidence:** run the version check unconditionally, INCLUDING under
`-SkipBuild` and `-NoRestart`. A deploy that deliberately skips the build
should still say out loud which build is running.

### 5. The executors keep running the code they started with, and the deploy says so nowhere

`update.ps1` restarts pollers, AI traders and telegram, and deliberately does
NOT start the mirrors — a good decision, argued in the file. The consequence is
that after a deploy the executors are either DOWN or running pre-deploy Python.
Nothing prints which.

This is not hypothetical: `ae2bdbd` (lot_scale in the snapshot) and `728a4a4`
(the one-sided join rule) are both committed and neither is in effect until an
executor restart, which is gated on the owner.

**Evidence:** each executor already writes a `started` line to its
`executor.jsonl` at launch. Add the git hash to it, and have the deploy print,
per account and book, the hash of the executor currently running. "Three
mirrors are on 024817b, one is on 9685e2e" is the sentence that would have been
missing.

### 6. Three config layers never travel, and one of them outranks the file that does

`config/guards.toml` is gitignored on purpose: it is the layer the desk edits at
runtime, and committing it would make two files claim the same authority. That
is right, and it also means the running guard values are NOT the ones a reader
sees in `config/default.toml` after a pull.

**Evidence:** the deploy should print `guards.toml`'s content hash and mtime,
and say plainly that these override the committed defaults. Same for
`config/local.toml` if present — its existence should be stated, because a file
whose whole purpose is to change behaviour invisibly should not also be invisible
in the deploy log.

### 7. Python dependencies are never reconciled

`deploy/requirements.txt` is tracked and changes with `git pull`; nothing
installs it during an update. A new import fails at runtime, in a process that
may be a mirror.

**Evidence:** `pip install -r deploy/requirements.txt` during the update, with
its output shown. "Already satisfied" is a fast, honest no-op; silence is not.

### 8. The packaged zip is a fourth artefact path with a dated name

`deploy/pack.ps1` builds, stages `target/release/fd-api.exe` and `ui/dist`, and
zips. `deploy/flowdesk-20260917-0026.zip` is sitting in the tree now. A dated
filename is an invitation to ship the wrong one, and unpacking it restores both
un-versioned artefacts at once.

**Evidence:** `pack.ps1` should write the git hash into the zip (a `VERSION`
file beside the exe) and the far side should refuse a zip whose hash is not the
one expected.

## The check, specified

### `GET /api/version`

Being added by another session. This document only says what the deploy does
with it.

Fields needed: the git hash the binary was built from, whether the tree was
dirty at build time, and the build timestamp. Dirty matters — a binary built
from uncommitted edits has a hash that is true and a content that is not, and
that is a state the deploy should name rather than pass.

**Where it is read:** `deploy/update.ps1`, replacing the readiness poll. The
loop still waits for the endpoint to answer at all; the ACCEPTANCE condition
changes from "answered" to "answered, and `hash == $after`", where `$after` is
already captured at the pull step.

**What it compares against:** `git rev-parse HEAD` on the server, after the
pull. That pair is precisely a5's question — the VPS pulls source and runs a
binary copied separately, so source HEAD and running binary can silently
differ. This is the check, and it is a check on the ENDPOINT rather than on the
file, so it also covers a scheduled task pointing at a copy nobody swapped.

**What it does on mismatch:** refuse, print both hashes and the build time, and
name the two likely causes in order — the binary was not rebuilt, or the copy
failed because the old one was still running. It must NOT auto-rebuild: this
runs on a machine that trades, and the correct response to "the thing running
is not the thing you think" is to stop and tell someone.

### The client's equivalent

No endpoint needed. After the swap, fetch `/` and extract the referenced
`assets/index-*.js`; require that exact filename to exist in `ui/dist`. Compare
the reference, never a directory listing — twelve generations are kept and the
oldest sorts first.

A stronger version, if `-45` is willing: have the build write the git hash into
`ui/dist/version.json` and have the client display it, so a person looking at
the desk can see which client they have without a terminal. The desk already
shows three clocks per entry for the same reason — a number that is invisible is
a number nobody checks.

## What this does not cover

- It does not propose changing `update.ps1`. That file is another session's
  tonight, and this is a spec, not a patch.
- It says nothing about whether the deploy SHOULD build on the server. Both the
  build-here path (`update.ps1`) and the copy-artefacts path are in use, and the
  findings above apply to both; the version check is what makes either honest.
- It does not address MT5 terminal or RDP session staleness, which is
  `start-desk.ps1`'s territory and a different failure family (a logged-off
  session kills the desk silently — see that file's header).
- It assumes `/api/version` reports the hash the BINARY was built from, not the
  hash of the checkout it is running in. If it reports the latter it is
  self-fulfilling and checks nothing; that distinction is the whole value of
  the endpoint and should be stated in its own implementation.
