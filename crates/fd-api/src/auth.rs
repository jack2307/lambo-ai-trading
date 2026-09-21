//! A password on the second listener, for the tunnel.
//!
//! # Why there are two listeners and not one guard
//!
//! `fd-api` has served `127.0.0.1:8138` with **no authentication of any kind**
//! since it was written, and that is not an oversight: the port is the bus
//! between the desk's own processes. Nine `ai_trader.py` instances, one
//! `mt5_executor.py` and four `mt5_bars.py` pollers call it constantly and
//! carry no credentials. Putting a password in front of that port stops the
//! desk trading. So 8138 keeps exactly the behaviour it has.
//!
//! The owner wants the same screens from his own machine, through a
//! **Cloudflare Tunnel**, with full access — not a read-only view. `cloudflared`
//! runs **on this machine** and dials out; it reaches `fd-api` over loopback
//! like everything else.
//!
//! **That is the whole trap.** A single listener with a rule of the form
//! "require a session unless the peer is 127.0.0.1" authenticates *nobody*:
//! every request that arrives from the internet through the tunnel is handed to
//! us by a local process and wears a loopback address. The rule would read as
//! safe and be worth nothing. Any future change here must be checked against
//! that sentence first.
//!
//! So the separation is by **port**, which the tunnel cannot forge:
//!
//! * **8138** — loopback, unauthenticated, unchanged. The desk's own processes.
//! * **8139 by default** — loopback, and **every request needs a session, with
//!   no exemption for anybody**. This is the one `cloudflared` is pointed at.
//!
//! Neither is bound to a public interface. Nothing here opens a socket to the
//! internet; the tunnel makes the outbound connection.
//!
//! # What is stored, and what is not
//!
//! `config/local.toml` — the gitignored file that already holds the DeepSeek
//! key — gains an `[auth]` section with a random per-install salt, an iteration
//! count and a PBKDF2-HMAC-SHA256 verifier. **There is no plaintext password
//! anywhere and nothing in git.** [`gitignored`] is checked before a write, the
//! same refusal `py/live/advisor_setup.py` makes before it stores a key.
//!
//! Sessions live in memory and are **gone when the process restarts**. That is
//! deliberate — a session that survives a deploy is a session nobody can
//! revoke by restarting — and the operator will meet it every time the desk is
//! redeployed: he logs in again. Nothing else breaks.

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use ring::rand::SecureRandom;
use serde::Deserialize;

/* ------------------------------------------------------------- the numbers */

/// The port the authed listener uses when nothing says otherwise.
///
/// One above the desk's own 8138, so `netstat` on this machine reads as a pair
/// and nobody has to look up which is which.
pub const DEFAULT_AUTH_PORT: u16 = 8139;

/// The cookie the session travels in.
pub const COOKIE: &str = "fd_session";

/// The shortest password `--set-password` will accept.
///
/// Twelve, not eight. The number that matters is not "how long is long" but
/// what the lockout below leaves an attacker: about 96 guesses a day, 35,000 a
/// year. Eight characters of anything a person invents is inside a wordlist far
/// smaller than that; twelve, chosen as four words or as a manager's output, is
/// not. This is the one half of the pair the operator controls, so it is stated
/// as a refusal rather than as advice.
pub const MIN_PASSWORD_LEN: usize = 12;

/// PBKDF2-HMAC-SHA256 iterations.
///
/// 600,000 is OWASP's standing recommendation for PBKDF2-HMAC-SHA256 and costs
/// this machine roughly two tenths of a second per attempt. That number is
/// doing two jobs: it prices an offline attack on the stored verifier if
/// `config/local.toml` ever leaves the machine, and it prices an online one
/// here, because it is paid on every single login attempt before the lockout
/// even gets a turn.
///
/// The count is **stored beside the hash** rather than assumed, so raising this
/// constant later does not silently invalidate a password already set. A
/// verifier written at 600,000 keeps verifying at 600,000; the operator moves
/// to the new number by running `--set-password` again.
pub const PBKDF2_ITERATIONS: u32 = 600_000;

/// How long a session is good for.
///
/// Twelve hours: the operator signs in at the start of a session at the desk
/// and is not thrown out in the middle of watching a book, which is when a
/// re-login is most likely to be typed carelessly or postponed by leaving a
/// tab open. It is an **absolute** expiry from the moment of login, not a
/// sliding one — a cookie that renews itself on use can be kept alive forever
/// by whoever holds it.
pub const SESSION_TTL: Duration = Duration::from_secs(12 * 60 * 60);

/// Failed logins allowed before the lockout starts counting.
///
/// Five, because a person who has just typed a long password wrong twice will
/// try it twice more before they go and look it up, and a scheme that punishes
/// that teaches the operator to store the password somewhere convenient.
pub const FREE_ATTEMPTS: u32 = 5;

/// The first lockout, doubling with each further failure.
pub const LOCKOUT_BASE_SECS: u64 = 5;

/// The ceiling on the lockout.
///
/// Fifteen minutes. Past this the doubling buys nothing an attacker notices and
/// starts to cost the operator a real outage after a fat-fingered evening.
///
/// **What this costs an attacker.** Five free guesses, then 5s, 10s, 20s, 40s,
/// 80s, 160s, 320s, 640s and 900s from the fourteenth onward: 96 guesses a day,
/// about 35,000 a year, every one of them also paying the 600,000 PBKDF2
/// iterations above. **What it costs the owner:** somebody who can reach the
/// tunnel can hold the lockout at fifteen minutes indefinitely and keep him
/// out. That is the deliberate direction of the trade — being locked out of a
/// web page while the desk keeps trading is recoverable, and he still has 8138
/// from the machine itself over RDP. A guesser getting in is not recoverable.
pub const LOCKOUT_MAX_SECS: u64 = 900;

/// The section in `config/local.toml`.
const SECTION: &str = "auth";

/// Bytes of salt. Per install, from the OS, and its only job is to make one
/// precomputed table useless against every flowdesk in the world at once.
const SALT_LEN: usize = 16;

/// SHA-256's output, and so the verifier's length.
const HASH_LEN: usize = ring::digest::SHA256_OUTPUT_LEN;

/// Bytes in a session token. 256 bits from the OS; guessing one is not a
/// threat model, it is arithmetic.
const TOKEN_LEN: usize = 32;

/// The login page, served by the authed listener and used as the body of every
/// 401 that asked for HTML.
const LOGIN_PAGE: &str = include_str!("login.html");

fn b64() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// URL-safe and unpadded, because the value goes in a cookie and `=`, `/` and
/// `+` are all characters a cookie parser somewhere has opinions about.
fn b64_token() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

/* --------------------------------------------------------- the stored hash */

/// The `[auth]` section of `config/local.toml`, parsed.
///
/// Never `Serialize`, and never reachable from [`crate::AppState`], so no route
/// can render it by accident. The only things that ever leave this struct are
/// `true` and `false`.
#[derive(Clone)]
pub struct StoredPassword {
    salt: Vec<u8>,
    hash: Vec<u8>,
    iterations: NonZeroU32,
}

impl std::fmt::Debug for StoredPassword {
    /// No salt, no hash, no length. A `{:?}` in a log line is exactly how a
    /// verifier ends up somewhere it can be attacked offline.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StoredPassword(<redacted>)")
    }
}

impl StoredPassword {
    /// Derive a fresh verifier for `password`, with a new random salt.
    ///
    /// Refuses a short password and says the minimum, because the caller is a
    /// person at a console and "invalid" is not something a person can act on.
    pub fn create(password: &str) -> Result<Self, String> {
        // Counted in characters, not bytes: a passphrase with an accent in it
        // must not be measured as longer than it reads.
        let len = password.chars().count();
        if len < MIN_PASSWORD_LEN {
            return Err(format!(
                "that password is {len} characters. The minimum is {MIN_PASSWORD_LEN} — this one \
                 guards a desk that can stop a live book, and the lockout only buys time against \
                 a password worth guessing at."
            ));
        }
        let mut salt = vec![0u8; SALT_LEN];
        ring::rand::SystemRandom::new()
            .fill(&mut salt)
            .map_err(|_| "the OS would not give us random bytes for a salt".to_string())?;
        let iterations = NonZeroU32::new(PBKDF2_ITERATIONS).expect("a non-zero iteration count");
        let hash = derive(password, &salt, iterations);
        Ok(Self { salt, hash, iterations })
    }

    /// Is this the password?
    ///
    /// `ring::pbkdf2::verify` re-derives and compares in constant time. It is
    /// not a `==` on two byte slices and must never be rewritten as one.
    #[must_use]
    pub fn verify(&self, password: &str) -> bool {
        ring::pbkdf2::verify(
            ring::pbkdf2::PBKDF2_HMAC_SHA256,
            self.iterations,
            &self.salt,
            password.as_bytes(),
            &self.hash,
        )
        .is_ok()
    }

    /// Read `[auth]` out of `config/local.toml`.
    ///
    /// Three outcomes, and the middle one is the whole point:
    ///
    /// * `Ok(None)` — no file, or no `[auth]` section. Nothing was configured.
    /// * `Err(_)` — a section that is there but incomplete or unreadable. An
    ///   absent value is **absent**, never a default; a half-written `[auth]`
    ///   must not resolve to "no password needed".
    /// * `Ok(Some(_))` — a verifier.
    pub fn load(config_dir: &Path) -> Result<Option<Self>, String> {
        let path = config_dir.join("local.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("reading {}: {e}", path.display())),
        };
        Self::from_toml(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The parsing half of [`Self::load`], split out so it can be tested
    /// without a file on disk.
    pub fn from_toml(text: &str) -> Result<Option<Self>, String> {
        let root: toml::Value = toml::from_str(text).map_err(|e| format!("parsing: {e}"))?;
        let Some(section) = root.get(SECTION) else { return Ok(None) };
        let table = section
            .as_table()
            .ok_or_else(|| format!("[{SECTION}] is not a table"))?;
        // An empty `[auth]` is a section the operator wrote and then did not
        // fill in. Treating it as absent would be the same mistake as treating
        // a missing hash as "no password wanted".
        if table.is_empty() {
            return Err(format!(
                "[{SECTION}] is empty. Run `fd-api --set-password`, or delete the section."
            ));
        }
        let field = |key: &str| -> Result<String, String> {
            table
                .get(key)
                .ok_or_else(|| format!("[{SECTION}] has no `{key}`. Run `fd-api --set-password`."))?
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("[{SECTION}].{key} must be a base64 string"))
        };
        let salt = b64()
            .decode(field("salt")?)
            .map_err(|e| format!("[{SECTION}].salt is not base64: {e}"))?;
        let hash = b64()
            .decode(field("hash")?)
            .map_err(|e| format!("[{SECTION}].hash is not base64: {e}"))?;
        if salt.is_empty() {
            return Err(format!("[{SECTION}].salt is empty"));
        }
        if hash.len() != HASH_LEN {
            return Err(format!(
                "[{SECTION}].hash is {} bytes; PBKDF2-HMAC-SHA256 produces {HASH_LEN}",
                hash.len()
            ));
        }
        let raw = table
            .get("pbkdf2_iterations")
            .ok_or_else(|| {
                format!("[{SECTION}] has no `pbkdf2_iterations`. Run `fd-api --set-password`.")
            })?
            .as_integer()
            .ok_or_else(|| format!("[{SECTION}].pbkdf2_iterations must be a whole number"))?;
        // Zero is not "use the default". A verifier that says zero iterations
        // is a corrupt verifier, and the only safe reading of it is a refusal.
        let iterations = u32::try_from(raw)
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(|| format!("[{SECTION}].pbkdf2_iterations is {raw}, which is not a usable count"))?;
        Ok(Some(Self { salt, hash, iterations }))
    }

    /// The three lines this writes into `[auth]`.
    fn toml_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("pbkdf2_iterations", self.iterations.to_string()),
            ("salt", format!("\"{}\"", b64().encode(&self.salt))),
            ("hash", format!("\"{}\"", b64().encode(&self.hash))),
        ]
    }

    /// Write the verifier into `config/local.toml`, leaving every other line
    /// byte-identical.
    ///
    /// Line-based rather than a TOML round-trip, for the reason
    /// `advisor_setup.py` gives for doing the same thing: a parser that
    /// rewrites the file drops the comments, and the comments in `config/` are
    /// load bearing here.
    ///
    /// Refuses unless `config/local.toml` is gitignored. There is no argument
    /// for writing a password verifier into a tracked file, and the check is
    /// cheap enough to make every time rather than once in a comment.
    pub fn save(&self, repo_root: &Path, config_dir: &Path) -> Result<(), String> {
        if !gitignored(repo_root) {
            return Err(format!(
                "refusing to write: config/local.toml is not in {}/.gitignore. Add it first — a \
                 password verifier does not go in a tracked file.",
                repo_root.display()
            ));
        }
        let path = config_dir.join("local.toml");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let pairs = self.toml_pairs();
        let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let out = set_section_keys(&existing, SECTION, &refs);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        std::fs::write(&path, out).map_err(|e| format!("writing {}: {e}", path.display()))
    }
}

/// PBKDF2-HMAC-SHA256, the one place the derivation is written.
fn derive(password: &str, salt: &[u8], iterations: NonZeroU32) -> Vec<u8> {
    let mut out = vec![0u8; HASH_LEN];
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        salt,
        password.as_bytes(),
        &mut out,
    );
    out
}

/// Is `config/local.toml` ignored by git?
///
/// Deliberately the same text match `py/live/advisor_setup.py` makes, so the
/// two refusals agree about what counts.
#[must_use]
pub fn gitignored(repo_root: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(repo_root.join(".gitignore")) else { return false };
    text.lines()
        .any(|l| matches!(l.trim(), "config/local.toml" | "/config/local.toml"))
}

/// Set several keys inside `[section]`, leaving every other line untouched.
///
/// A key already in the section is replaced where it stands; a key that is
/// missing is appended to the end of the section; a section that does not exist
/// is appended to the file.
fn set_section_keys(existing: &str, section: &str, pairs: &[(&str, &str)]) -> String {
    let header = format!("[{section}]");
    let mut out: Vec<String> = Vec::new();
    let mut inside = false;
    let mut written = vec![false; pairs.len()];

    let flush_remaining = |out: &mut Vec<String>, written: &mut Vec<bool>| {
        for (i, (key, value)) in pairs.iter().enumerate() {
            if !written[i] {
                out.push(format!("{key} = {value}"));
                written[i] = true;
            }
        }
    };

    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if inside {
                flush_remaining(&mut out, &mut written);
            }
            inside = trimmed == header;
            out.push(line.to_string());
            continue;
        }
        if inside {
            if let Some(i) = pairs.iter().position(|(key, _)| is_assignment(line, key)) {
                if !written[i] {
                    out.push(format!("{} = {}", pairs[i].0, pairs[i].1));
                    written[i] = true;
                }
                // The old line is dropped rather than kept above the new one.
                continue;
            }
        }
        out.push(line.to_string());
    }
    if inside {
        flush_remaining(&mut out, &mut written);
    }
    if written.iter().any(|w| !w) {
        if out.last().is_some_and(|l| !l.trim().is_empty()) {
            out.push(String::new());
        }
        out.push(header);
        flush_remaining(&mut out, &mut written);
    }
    let mut text = out.join("\n");
    while text.ends_with('\n') {
        text.pop();
    }
    text.push('\n');
    text
}

/// Does this line assign `key`? `key = ...`, with whatever spacing.
fn is_assignment(line: &str, key: &str) -> bool {
    let rest = line.trim_start();
    rest.strip_prefix(key)
        .is_some_and(|tail| tail.trim_start().starts_with('='))
}

/* ------------------------------------------------------------- the sessions */

/// One live session.
struct Session {
    token: [u8; TOKEN_LEN],
    expires: Instant,
}

/// Everything one guess or one cookie touches, under one lock.
struct Live {
    sessions: Vec<Session>,
    /// Consecutive failures since the last success. Reset by a success, never
    /// by time — a patient attacker must not be able to wait the counter off.
    failures: u32,
    locked_until: Option<Instant>,
    /// Is a PBKDF2 running right now?
    ///
    /// **This is the answer to a denial of service, not to guessing.** The
    /// 600,000 iterations that price an attacker's guess are 600,000
    /// iterations of this machine's CPU, and the lockout cannot arm until the
    /// sixth guess has *finished*. Without this flag a burst of a thousand
    /// simultaneous attempts would all pass the lockout check together and
    /// then all run — two hundred core-seconds of work on a four-core VPS
    /// that is also carrying nine trading books.
    ///
    /// So one at a time, and the rest are turned away immediately without
    /// doing the work and **without counting as failures**: a flood must not
    /// be able to lock the operator out either.
    verifying: bool,
}

/// The authed listener's whole security state.
pub struct Auth {
    password: StoredPassword,
    live: Mutex<Live>,
    rng: ring::rand::SystemRandom,
}

/// Why a login did not produce a session.
pub enum LoginError {
    /// Too many failures. Carries what is left to wait.
    LockedOut(Duration),
    /// Another attempt is being checked. Not a failure, and not counted as
    /// one — see [`Live::verifying`].
    Busy,
    /// Wrong password, and nothing more specific than that is ever said.
    Wrong,
}

impl Auth {
    #[must_use]
    pub fn new(password: StoredPassword) -> Self {
        Self {
            password,
            live: Mutex::new(Live {
                sessions: Vec::new(),
                failures: 0,
                locked_until: None,
                verifying: false,
            }),
            rng: ring::rand::SystemRandom::new(),
        }
    }

    /// The lockout after `failures` consecutive wrong answers.
    ///
    /// Free up to [`FREE_ATTEMPTS`], then [`LOCKOUT_BASE_SECS`] doubling each
    /// time to [`LOCKOUT_MAX_SECS`].
    #[must_use]
    pub fn backoff(failures: u32) -> Option<Duration> {
        let over = failures.checked_sub(FREE_ATTEMPTS)?;
        if over == 0 {
            return None;
        }
        // Shifted, then capped. `min(20)` keeps the shift itself in range —
        // 5 << 20 is already four hundred times the cap.
        let secs = (LOCKOUT_BASE_SECS << (over - 1).min(20)).min(LOCKOUT_MAX_SECS);
        Some(Duration::from_secs(secs))
    }

    /// Check a password and, if it is right, mint a session.
    ///
    /// **Call this off the async runtime.** The PBKDF2 inside takes about two
    /// tenths of a second of solid CPU, which is the point of it; run on a
    /// tokio worker it would stall every other request that worker is carrying,
    /// and one guess per worker would be a denial of service against the desk
    /// itself. [`login`] hands it to `spawn_blocking`.
    pub fn login(&self, password: &str) -> Result<String, LoginError> {
        let now = Instant::now();
        {
            let mut live = self.live.lock().expect("auth state");
            if let Some(until) = live.locked_until {
                if until > now {
                    return Err(LoginError::LockedOut(until - now));
                }
            }
            if live.verifying {
                return Err(LoginError::Busy);
            }
            live.verifying = true;
        }
        // Deliberately outside the lock: the PBKDF2 must not hold a mutex that
        // every authenticated request also takes. `verifying` above is what
        // keeps that from becoming a way to spend the machine's whole CPU.
        let ok = self.password.verify(password);

        let mut live = self.live.lock().expect("auth state");
        live.verifying = false;
        if !ok {
            live.failures = live.failures.saturating_add(1);
            if let Some(wait) = Self::backoff(live.failures) {
                live.locked_until = Some(Instant::now() + wait);
            }
            return Err(LoginError::Wrong);
        }
        live.failures = 0;
        live.locked_until = None;

        let mut token = [0u8; TOKEN_LEN];
        self.rng.fill(&mut token).expect("the OS random source");
        let encoded = b64_token().encode(token);
        let expires = Instant::now() + SESSION_TTL;
        live.sessions.retain(|s| s.expires > now);
        live.sessions.push(Session { token, expires });
        Ok(encoded)
    }

    /// Does this `Cookie:` header carry a live session?
    ///
    /// **Constant time by construction**, in both of the ways this can go
    /// wrong.
    ///
    /// The bytes are compared with [`same`], which is `subtle`'s
    /// `ConstantTimeEq` and not `==`. And the loop does not stop when it
    /// matches: `|=` on a `bool` is not `||` and does not short-circuit, so
    /// every live session is compared on every request. A version that
    /// returned early on the first hit would be constant-time per comparison
    /// and still leak, through the total, how far down the list a presented
    /// token sits.
    #[must_use]
    pub fn is_authenticated(&self, cookie_header: Option<&str>) -> bool {
        let Some(raw) = cookie_header.and_then(|h| cookie_value(h, COOKIE)) else { return false };
        let Ok(bytes) = b64_token().decode(raw) else { return false };
        if bytes.len() != TOKEN_LEN {
            return false;
        }
        let now = Instant::now();
        let mut live = self.live.lock().expect("auth state");
        live.sessions.retain(|s| s.expires > now);
        let mut hit = false;
        for session in &live.sessions {
            hit |= same(&session.token, &bytes);
        }
        hit
    }

    /// Drop the session this cookie names, if it names one.
    ///
    /// Needs no password: it can only ever remove capability, and only the
    /// capability whose token the caller already holds.
    pub fn logout(&self, cookie_header: Option<&str>) {
        let Some(raw) = cookie_header.and_then(|h| cookie_value(h, COOKIE)) else { return };
        let Ok(bytes) = b64_token().decode(raw) else { return };
        let now = Instant::now();
        let mut live = self.live.lock().expect("auth state");
        live.sessions.retain(|s| s.expires > now && !same(&s.token, &bytes));
    }

    /// How many sessions are live right now. For tests and for the startup
    /// line; carries no token.
    #[must_use]
    pub fn session_count(&self) -> usize {
        let now = Instant::now();
        let mut live = self.live.lock().expect("auth state");
        live.sessions.retain(|s| s.expires > now);
        live.sessions.len()
    }
}

/// Are these the same bytes, in time that does not depend on how many of them
/// match?
///
/// `subtle`, not `ring::constant_time`: ring deprecated
/// `verify_slices_are_equal` in 0.17 with the note that it makes "no promises
/// regarding side channels", and a comparison whose own author will not
/// promise that is not the comparison to guard a session token with. `subtle`
/// is a crate whose entire purpose is this, and it was already in the tree.
/// The password itself is still checked by `ring::pbkdf2::verify`, which is
/// not deprecated and does its own comparison internally.
///
/// **Never rewrite this as `a == b`.** `==` on two slices returns on the first
/// differing byte, which turns guessing a 32-byte token into guessing 32
/// bytes one at a time.
#[must_use]
fn same(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    a.ct_eq(b).into()
}

/// Pull one cookie out of a `Cookie:` header.
fn cookie_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key.trim() == name).then(|| value.trim())
    })
}

/* -------------------------------------------------------------- the routes */

/// Paths the guard lets past without a session.
///
/// Three, and the list is written out rather than computed so that adding a
/// fourth is a decision somebody makes on purpose. Everything else on this
/// listener — every route, every unknown path, the SPA itself — needs a
/// session.
fn is_public(path: &str) -> bool {
    matches!(path, "/login" | "/api/auth/login" | "/api/auth/logout")
}

/// The layer that makes the second listener the second listener.
///
/// There is **no exemption for a loopback peer**, and there must never be one:
/// `cloudflared` is a local process, so every request off the internet arrives
/// with a loopback address and such a rule would let all of them through. See
/// this module's header.
pub async fn guard(State(auth): State<Arc<Auth>>, request: Request, next: Next) -> Response {
    if is_public(request.uri().path()) {
        return next.run(request).await;
    }
    let cookie = request
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    if auth.is_authenticated(cookie.as_deref()) {
        return next.run(request).await;
    }
    unauthorised(request.headers())
}

/// The answer to every request that has no session.
///
/// **401 and the page, never a redirect.** A 302 to `/login` from a path that
/// exists and a 404 from one that does not is a map of the API handed to anyone
/// who asks; this way `/api/paper/stop` and `/api/paper/stpo` are the same
/// three hundred bytes. No `WWW-Authenticate` either — it would pop the
/// browser's own basic-auth box in front of the page.
fn unauthorised(headers: &HeaderMap) -> Response {
    let wants_html = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|accept| accept.contains("text/html"));
    if wants_html {
        (
            StatusCode::UNAUTHORIZED,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            LOGIN_PAGE,
        )
            .into_response()
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "sign in first" })),
        )
            .into_response()
    }
}

/// `GET /login` — the page, for a person who arrived at a signed-out tab.
pub async fn page() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        LOGIN_PAGE,
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct LoginBody {
    pub password: String,
}

/// `POST /api/auth/login`.
pub async fn login(State(auth): State<Arc<Auth>>, Json(body): Json<LoginBody>) -> Response {
    // Off the runtime: the PBKDF2 is two tenths of a second of CPU and would
    // otherwise be spent on a thread that is also carrying the desk's traffic.
    let worker = Arc::clone(&auth);
    let result = tokio::task::spawn_blocking(move || worker.login(&body.password)).await;
    match result {
        Ok(Ok(token)) => {
            let cookie = format!(
                // `Secure` even though this listener speaks plain HTTP: what the
                // browser sees is the tunnel's HTTPS, and browsers treat
                // 127.0.0.1 as a secure origin too, so nothing is locked out by
                // it. `SameSite=Strict` because no other site has any business
                // making a request to this desk on the operator's behalf.
                "{COOKIE}={token}; HttpOnly; SameSite=Strict; Secure; Path=/; Max-Age={}",
                SESSION_TTL.as_secs()
            );
            (StatusCode::OK, [(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "ok": true })))
                .into_response()
        }
        Ok(Err(LoginError::LockedOut(left))) => {
            let secs = left.as_secs().max(1);
            (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, secs.to_string())],
                Json(serde_json::json!({
                    "error": format!(
                        "too many failed sign-ins. Try again in {secs}s — the wait doubles with \
                         each failure, up to {}m.", LOCKOUT_MAX_SECS / 60
                    )
                })),
            )
                .into_response()
        }
        Ok(Err(LoginError::Busy)) => (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, "1".to_string())],
            Json(serde_json::json!({
                "error": "another sign-in is being checked. Try again in a moment."
            })),
        )
            .into_response(),
        // One sentence for a wrong password and for no password at all. A form
        // that distinguishes them is a form that can be searched.
        Ok(Err(LoginError::Wrong)) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "wrong password" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("the sign-in worker died: {e}") })),
        )
            .into_response(),
    }
}

/// `POST /api/auth/logout` — drop this session and clear the cookie.
pub async fn logout(State(auth): State<Arc<Auth>>, headers: HeaderMap) -> Response {
    auth.logout(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let cleared = format!("{COOKIE}=; HttpOnly; SameSite=Strict; Secure; Path=/; Max-Age=0");
    (StatusCode::OK, [(header::SET_COOKIE, cleared)], Json(serde_json::json!({ "ok": true })))
        .into_response()
}

/* --------------------------------------------- whether to open the listener */

/// What `main` should do about the second listener.
///
/// Three outcomes and no fourth. In particular there is no outcome that opens
/// the listener without a password: [`Refused`](Tunnel::Refused) is what a
/// missing or broken `[auth]` produces, and the caller's only options for it
/// are to say so and not bind.
pub enum Tunnel {
    /// Nobody asked for it. This is what every machine that has not been
    /// deliberately switched over gets, and it is today's behaviour exactly.
    Off,
    /// Asked for, and ready to serve.
    Ready { port: u16, auth: Arc<Auth> },
    /// Asked for and **not** ready. The listener must not be opened.
    Refused { port: u16, why: String },
}

/// Decide, from the config directory and the command line.
///
/// Requested means an explicit switch: `--auth`, `--auth-port=N`, or
/// `enabled = true` in `[auth]`. Setting a password does **not** by itself
/// open a port — adding a listener to a machine that trades a funded account
/// is a decision somebody makes on purpose, not a side effect of another
/// command.
pub fn tunnel(config_dir: &Path, cli_port: Option<u16>, cli_flag: bool) -> Tunnel {
    let asked_on_the_line = cli_flag || cli_port.is_some();
    let path = config_dir.join("local.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            // Unreadable. If nobody asked for the listener that is not this
            // function's business; if somebody did, it is fatal to the
            // listener and to nothing else.
            return if asked_on_the_line {
                Tunnel::Refused {
                    port: cli_port.unwrap_or(DEFAULT_AUTH_PORT),
                    why: format!("reading {}: {e}", path.display()),
                }
            } else {
                Tunnel::Off
            };
        }
    };
    let (enabled, config_port) = match switches(&text) {
        Ok(pair) => pair,
        Err(why) => {
            return Tunnel::Refused {
                port: cli_port.unwrap_or(DEFAULT_AUTH_PORT),
                why: format!("{}: {why}", path.display()),
            };
        }
    };
    let port = cli_port.or(config_port).unwrap_or(DEFAULT_AUTH_PORT);
    if !asked_on_the_line && !enabled {
        return Tunnel::Off;
    }
    match StoredPassword::from_toml(&text) {
        Ok(Some(password)) => Tunnel::Ready { port, auth: Arc::new(Auth::new(password)) },
        Ok(None) => Tunnel::Refused {
            port,
            why: format!(
                "{} has no [{SECTION}] password. Set one with `fd-api --set-password`.",
                path.display()
            ),
        },
        Err(why) => Tunnel::Refused { port, why: format!("{}: {why}", path.display()) },
    }
}

/// `[auth].enabled` and `[auth].port`, if they are there.
///
/// Neither is allowed to be the wrong type quietly: `enabled = "yes"` is a
/// person who meant `true`, and reading it as absent would leave the listener
/// off while its owner believed it on.
fn switches(text: &str) -> Result<(bool, Option<u16>), String> {
    let root: toml::Value = toml::from_str(text).map_err(|e| format!("parsing: {e}"))?;
    let Some(table) = root.get(SECTION).and_then(toml::Value::as_table) else {
        return Ok((false, None));
    };
    let enabled = match table.get("enabled") {
        None => false,
        Some(v) => v
            .as_bool()
            .ok_or_else(|| format!("[{SECTION}].enabled must be true or false"))?,
    };
    let port = match table.get("port") {
        None => None,
        Some(v) => Some(
            v.as_integer()
                .and_then(|n| u16::try_from(n).ok())
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("[{SECTION}].port must be a port number"))?,
        ),
    };
    Ok((enabled, port))
}

/* ----------------------------------------------------- fd-api --set-password */

/// The variable `--set-password` will take a password from when one is set.
pub const PASSWORD_ENV: &str = "FD_AUTH_PASSWORD";

/// `fd-api --set-password`, whole.
///
/// In the binary rather than in a second tool, because a second tool is a
/// second thing to ship, to find on the VPS, and to keep in step with the
/// format this one reads.
pub fn set_password_command(repo_root: &Path, config_dir: &Path) -> Result<(), String> {
    // Checked before anything is asked for, so a machine that cannot safely
    // store a password never sees a prompt for one.
    if !gitignored(repo_root) {
        return Err(format!(
            "config/local.toml is not in {}/.gitignore. Nothing was asked for and nothing was \
             written — add the line first.",
            repo_root.display()
        ));
    }
    let (password, from_env) = read_password()?;
    let stored = StoredPassword::create(&password)?;
    stored.save(repo_root, config_dir)?;
    println!(
        "Written to {}: a random {SALT_LEN}-byte salt and a PBKDF2-HMAC-SHA256 verifier at \
         {} iterations. The password itself is not stored anywhere.",
        config_dir.join("local.toml").display(),
        PBKDF2_ITERATIONS
    );
    if from_env {
        println!(
            "It came from {PASSWORD_ENV}. Clear it now — `Remove-Item Env:{PASSWORD_ENV}` in \
             PowerShell, `unset {PASSWORD_ENV}` in a shell — and remember the command that set \
             it is in your history."
        );
    }
    println!(
        "Restart fd-api with --auth (or `enabled = true` under [{SECTION}]) to open the \
         authenticated listener on {DEFAULT_AUTH_PORT}. Anyone already signed in is signed out \
         by that restart."
    );
    Ok(())
}

/// Get the password, and say whether it came from the environment.
fn read_password() -> Result<(String, bool), String> {
    if let Ok(value) = std::env::var(PASSWORD_ENV) {
        if !value.is_empty() {
            return Ok((value, true));
        }
    }
    let interactive = std::io::IsTerminal::is_terminal(&std::io::stdin());
    let first = read_line_hidden("New desk password: ", interactive)?;
    if interactive {
        // Typed blind, so it is typed twice. A mistyped password that nobody
        // can reproduce is an operator locked out of his own desk.
        let again = read_line_hidden("Again: ", interactive)?;
        if again != first {
            return Err("those two did not match. Nothing was written.".into());
        }
    }
    Ok((first, false))
}

/// One line from stdin, without echoing it if that can be arranged.
fn read_line_hidden(prompt: &str, interactive: bool) -> Result<String, String> {
    use std::io::{BufRead, Write};
    let quiet = if interactive { Echo::off() } else { None };
    if interactive {
        if quiet.is_none() {
            println!(
                "(this console will show what you type — pipe the password in on stdin, or set \
                 {PASSWORD_ENV}, if that matters where you are sitting)"
            );
        }
        print!("{prompt}");
        std::io::stdout().flush().ok();
    }
    let mut line = String::new();
    let read = std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| format!("reading the password: {e}"))?;
    if interactive {
        // The newline the console did not echo.
        println!();
    }
    if read == 0 {
        return Err("no password on stdin".into());
    }
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}

/// The console's echo, off for as long as this is alive.
#[cfg(windows)]
struct Echo(u32);

#[cfg(windows)]
impl Echo {
    fn off() -> Option<Self> {
        use windows_sys::Win32::System::Console::{
            ENABLE_ECHO_INPUT, GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE, SetConsoleMode,
        };
        // SAFETY: three console calls on the process's own standard input.
        // Each is checked, and the mode read here is the one Drop puts back.
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            let mut mode: u32 = 0;
            if GetConsoleMode(handle, &mut mode) == 0 {
                return None;
            }
            if SetConsoleMode(handle, mode & !ENABLE_ECHO_INPUT) == 0 {
                return None;
            }
            Some(Self(mode))
        }
    }
}

#[cfg(windows)]
impl Drop for Echo {
    /// Put it back. A command that leaves a console unable to echo is a
    /// command nobody runs twice.
    fn drop(&mut self) {
        use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE, SetConsoleMode};
        // SAFETY: restoring the mode read in `off`, on the same handle.
        unsafe {
            SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), self.0);
        }
    }
}

/// Everywhere else there is no console API here without a new dependency, so
/// the command says so rather than pretending.
#[cfg(not(windows))]
struct Echo;

#[cfg(not(windows))]
impl Echo {
    fn off() -> Option<Self> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "correct horse battery staple";

    #[test]
    fn a_short_password_is_refused_and_the_minimum_is_named() {
        let err = StoredPassword::create("short").expect_err("eleven characters is not enough");
        assert!(err.contains(&MIN_PASSWORD_LEN.to_string()), "the refusal must say the minimum: {err}");
        // Exactly one under, to pin the boundary rather than the idea of one.
        let eleven = "a".repeat(MIN_PASSWORD_LEN - 1);
        assert!(StoredPassword::create(&eleven).is_err());
        assert!(StoredPassword::create(&"a".repeat(MIN_PASSWORD_LEN)).is_ok());
    }

    #[test]
    fn the_right_password_verifies_and_a_wrong_one_does_not() {
        let stored = StoredPassword::create(GOOD).expect("a verifier");
        assert!(stored.verify(GOOD));
        assert!(!stored.verify("correct horse battery stapl"));
        assert!(!stored.verify("correct horse battery staple "));
        assert!(!stored.verify(""));
    }

    #[test]
    fn two_installs_of_the_same_password_do_not_share_a_hash() {
        // The salt is per install. Without this, one precomputed table would
        // work against every flowdesk at once.
        let a = StoredPassword::create(GOOD).expect("a");
        let b = StoredPassword::create(GOOD).expect("b");
        assert_ne!(a.salt, b.salt);
        assert_ne!(a.hash, b.hash);
        assert!(a.verify(GOOD) && b.verify(GOOD));
    }

    #[test]
    fn the_verifier_survives_a_round_trip_through_the_file_format() {
        let stored = StoredPassword::create(GOOD).expect("a verifier");
        let pairs = stored.toml_pairs();
        let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let text = set_section_keys("", SECTION, &refs);
        let back = StoredPassword::from_toml(&text).expect("parses").expect("a section");
        assert!(back.verify(GOOD));
        assert!(!back.verify("something else"));
        assert_eq!(back.iterations, stored.iterations);
    }

    #[test]
    fn no_auth_section_is_absent_and_a_broken_one_is_an_error() {
        // Absent, which is honest: nothing was configured.
        assert!(StoredPassword::from_toml("").expect("empty parses").is_none());
        assert!(
            StoredPassword::from_toml("[advisor]\ndeepseek_api_key = \"sk-x\"\n")
                .expect("parses")
                .is_none()
        );
        // Present but unusable. Each of these must be an ERROR, never `None`:
        // `None` is what the caller reads as "no password wanted".
        for broken in [
            "[auth]\n",
            "[auth]\nsalt = \"AAAA\"\n",
            "[auth]\nsalt = \"AAAA\"\nhash = \"AAAA\"\n",
            "[auth]\nsalt = \"AAAA\"\nhash = \"AAAA\"\npbkdf2_iterations = 0\n",
            "[auth]\nsalt = \"not base64 at all!!\"\nhash = \"AAAA\"\npbkdf2_iterations = 1000\n",
        ] {
            let got = StoredPassword::from_toml(broken);
            assert!(got.is_err(), "a half-written [auth] must refuse, not read as absent: {broken:?}");
        }
    }

    #[test]
    fn the_stored_iteration_count_is_used_rather_than_the_constant() {
        // Raising PBKDF2_ITERATIONS later must not invalidate a password that
        // was set at the old number. The file's count is authoritative.
        let text = format!(
            "[auth]\npbkdf2_iterations = 1000\nsalt = \"{}\"\nhash = \"{}\"\n",
            b64().encode(b"sixteen bytes!!!"),
            b64().encode(derive(GOOD, b"sixteen bytes!!!", NonZeroU32::new(1000).unwrap())),
        );
        let stored = StoredPassword::from_toml(&text).expect("parses").expect("a section");
        assert_eq!(stored.iterations.get(), 1000);
        assert!(stored.verify(GOOD));
    }

    #[test]
    fn debug_never_prints_the_verifier() {
        let stored = StoredPassword::create(GOOD).expect("a verifier");
        let shown = format!("{stored:?}");
        assert!(!shown.contains(&b64().encode(&stored.hash)));
        assert!(!shown.contains(&b64().encode(&stored.salt)));
    }

    #[test]
    fn writing_the_section_leaves_every_other_line_alone() {
        let before = "# a comment that is load bearing\n[advisor]\ndeepseek_api_key = \"sk-x\"\n\n[telegram]\nbot_token = \"t\"\n";
        let after = set_section_keys(before, SECTION, &[("salt", "\"AAAA\""), ("hash", "\"BBBB\"")]);
        assert!(after.contains("# a comment that is load bearing"));
        assert!(after.contains("deepseek_api_key = \"sk-x\""));
        assert!(after.contains("bot_token = \"t\""));
        assert!(after.contains("[auth]"));
        assert!(after.contains("salt = \"AAAA\""));
        // And a second write replaces rather than duplicates.
        let again = set_section_keys(&after, SECTION, &[("salt", "\"CCCC\""), ("hash", "\"DDDD\"")]);
        assert_eq!(again.matches("salt =").count(), 1, "one salt, not two");
        assert!(again.contains("salt = \"CCCC\""));
        assert!(!again.contains("AAAA"));
        assert!(again.contains("deepseek_api_key = \"sk-x\""));
        assert_eq!(again.matches("[auth]").count(), 1);
    }

    #[test]
    fn writing_into_an_existing_auth_section_keeps_its_place_in_the_file() {
        let before = "[auth]\nsalt = \"OLD\"\n\n[telegram]\nbot_token = \"t\"\n";
        let after = set_section_keys(before, SECTION, &[("salt", "\"NEW\""), ("hash", "\"H\"")]);
        let salt_at = after.find("salt = \"NEW\"").expect("the salt");
        let telegram_at = after.find("[telegram]").expect("telegram");
        assert!(salt_at < telegram_at, "the key must stay in its own section:\n{after}");
        assert!(after.find("hash = \"H\"").expect("the hash") < telegram_at);
        assert!(after.contains("bot_token = \"t\""));
    }

    #[test]
    fn a_session_is_accepted_only_with_its_own_cookie() {
        let auth = Auth::new(StoredPassword::create(GOOD).expect("a verifier"));
        assert!(!auth.is_authenticated(None), "no cookie is not a session");
        assert!(!auth.is_authenticated(Some("")));
        let token = auth.login(GOOD).ok().expect("the right password mints a session");
        assert!(auth.is_authenticated(Some(&format!("{COOKIE}={token}"))));
        // Alongside other cookies, which is how a browser will actually send it.
        assert!(auth.is_authenticated(Some(&format!("theme=dark; {COOKIE}={token}; tz=7"))));
        // A token of the right shape that was never minted.
        let forged = b64_token().encode([7u8; TOKEN_LEN]);
        assert!(!auth.is_authenticated(Some(&format!("{COOKIE}={forged}"))));
        // The right bytes under the wrong name.
        assert!(!auth.is_authenticated(Some(&format!("session={token}"))));
        // Truncated, lengthened, and not base64 at all.
        assert!(!auth.is_authenticated(Some(&format!("{COOKIE}={}", &token[..token.len() - 1]))));
        assert!(!auth.is_authenticated(Some(&format!("{COOKIE}={token}A"))));
        assert!(!auth.is_authenticated(Some(&format!("{COOKIE}=!!!not base64"))));
    }

    #[test]
    fn a_wrong_password_mints_nothing() {
        let auth = Auth::new(StoredPassword::create(GOOD).expect("a verifier"));
        assert!(auth.login("not the password at all").is_err());
        assert_eq!(auth.session_count(), 0, "a failed sign-in must leave no session behind");
        assert!(auth.login("").is_err());
        assert_eq!(auth.session_count(), 0);
    }

    #[test]
    fn logging_out_drops_that_session_and_only_that_one() {
        let auth = Auth::new(StoredPassword::create(GOOD).expect("a verifier"));
        let first = auth.login(GOOD).ok().expect("one");
        let second = auth.login(GOOD).ok().expect("two");
        assert_eq!(auth.session_count(), 2);
        auth.logout(Some(&format!("{COOKIE}={first}")));
        assert!(!auth.is_authenticated(Some(&format!("{COOKIE}={first}"))));
        assert!(auth.is_authenticated(Some(&format!("{COOKIE}={second}"))));
        // A logout with no cookie, or a forged one, drops nothing.
        auth.logout(None);
        auth.logout(Some(&format!("{COOKIE}={}", b64_token().encode([9u8; TOKEN_LEN]))));
        assert_eq!(auth.session_count(), 1);
    }

    #[test]
    fn failures_get_slower_and_a_success_clears_the_count() {
        assert_eq!(Auth::backoff(0), None);
        assert_eq!(Auth::backoff(FREE_ATTEMPTS), None, "the free attempts are free");
        assert_eq!(Auth::backoff(FREE_ATTEMPTS + 1), Some(Duration::from_secs(5)));
        assert_eq!(Auth::backoff(FREE_ATTEMPTS + 2), Some(Duration::from_secs(10)));
        assert_eq!(Auth::backoff(FREE_ATTEMPTS + 3), Some(Duration::from_secs(20)));
        // Capped, and the shift does not overflow however far it is pushed.
        assert_eq!(Auth::backoff(FREE_ATTEMPTS + 40), Some(Duration::from_secs(LOCKOUT_MAX_SECS)));
        assert_eq!(Auth::backoff(u32::MAX), Some(Duration::from_secs(LOCKOUT_MAX_SECS)));

        let auth = Auth::new(StoredPassword::create(GOOD).expect("a verifier"));
        for _ in 0..FREE_ATTEMPTS {
            assert!(matches!(auth.login("wrong wrong wrong"), Err(LoginError::Wrong)));
        }
        // The sixth failure arms the lockout, and the seventh attempt - even
        // with the RIGHT password - is refused while it holds.
        assert!(matches!(auth.login("wrong wrong wrong"), Err(LoginError::Wrong)));
        match auth.login(GOOD) {
            Err(LoginError::LockedOut(left)) => {
                assert!(left <= Duration::from_secs(LOCKOUT_BASE_SECS));
            }
            _ => panic!("the lockout must hold against the right password too, or it is not a lockout"),
        }
    }

    #[test]
    fn a_success_resets_the_failure_count() {
        let auth = Auth::new(StoredPassword::create(GOOD).expect("a verifier"));
        for _ in 0..FREE_ATTEMPTS {
            assert!(auth.login("wrong wrong wrong").is_err());
        }
        assert!(auth.login(GOOD).is_ok(), "the free attempts must not have armed anything");
        // Back to a full allowance rather than one failure from a lockout.
        for _ in 0..FREE_ATTEMPTS {
            assert!(matches!(auth.login("wrong wrong wrong"), Err(LoginError::Wrong)));
        }
    }

    #[test]
    fn a_flood_of_guesses_does_not_buy_a_flood_of_pbkdf2() {
        // The lockout cannot arm until the sixth guess has FINISHED, so
        // without a bound on concurrency a burst would all pass the lockout
        // check together and all run the 600,000 iterations. On the desk's
        // VPS that is the whole machine, while nine books are trading.
        let auth = Arc::new(Auth::new(StoredPassword::create(GOOD).expect("a verifier")));
        let threads: Vec<_> = (0..16)
            .map(|_| {
                let auth = Arc::clone(&auth);
                std::thread::spawn(move || matches!(auth.login("wrong wrong wrong"), Err(LoginError::Busy)))
            })
            .collect();
        let turned_away = threads
            .into_iter()
            .map(|handle| handle.join().expect("a thread"))
            .filter(|was_busy| *was_busy)
            .count();
        assert!(turned_away > 0, "sixteen at once and every one of them ran the hash");

        // And being turned away is NOT a failure: a flood must not be able to
        // lock the operator out of his own desk.
        let live = auth.live.lock().expect("state");
        assert!(
            live.failures <= 16 - turned_away as u32,
            "a rejected attempt was counted as a wrong password"
        );
        assert!(!live.verifying, "the flag must not be left set");
    }

    #[test]
    fn an_expired_session_stops_working() {
        let auth = Auth::new(StoredPassword::create(GOOD).expect("a verifier"));
        let token = auth.login(GOOD).ok().expect("a session");
        assert!(auth.is_authenticated(Some(&format!("{COOKIE}={token}"))));
        // Age it by hand rather than waiting twelve hours.
        {
            let mut live = auth.live.lock().expect("state");
            for s in &mut live.sessions {
                s.expires = Instant::now() - Duration::from_secs(1);
            }
        }
        assert!(!auth.is_authenticated(Some(&format!("{COOKIE}={token}"))));
        assert_eq!(auth.session_count(), 0, "an expired session is swept, not kept");
    }

    #[test]
    fn only_three_paths_are_public() {
        assert!(is_public("/login"));
        assert!(is_public("/api/auth/login"));
        assert!(is_public("/api/auth/logout"));
        for guarded in [
            "/",
            "/index.html",
            "/api/version",
            "/api/paper/stop",
            "/api/paper/pause",
            "/api/paper/start",
            "/api/paper/open",
            "/api/paper/intent",
            "/api/paper/pending/act",
            "/api/advisor/credentials/clear",
            "/login/",
            "/LOGIN",
            "/api/auth/login/../../api/paper/stop",
        ] {
            assert!(!is_public(guarded), "{guarded} must need a session");
        }
    }

    /// A config directory with `local.toml` holding `text`.
    fn config_dir_with(text: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::write(dir.path().join("local.toml"), text).expect("write local.toml");
        dir
    }

    #[test]
    fn nobody_asking_means_no_second_listener() {
        let dir = config_dir_with("[advisor]\ndeepseek_api_key = \"sk-x\"\n");
        assert!(matches!(tunnel(dir.path(), None, false), Tunnel::Off));
        // And a machine with no local.toml at all, which is most of them.
        let empty = tempfile::tempdir().expect("a temp dir");
        assert!(matches!(tunnel(empty.path(), None, false), Tunnel::Off));
    }

    #[test]
    fn a_missing_hash_refuses_the_listener_rather_than_opening_it() {
        // The one that matters. Every way of asking for the listener without
        // a password must land on Refused — never Ready, and never Off with
        // the port quietly bound by something else.
        let empty = tempfile::tempdir().expect("a temp dir");
        for (port, flag) in [(None, true), (Some(9000), false), (Some(9000), true)] {
            match tunnel(empty.path(), port, flag) {
                Tunnel::Refused { why, .. } => {
                    assert!(why.contains("--set-password"), "the refusal must say the fix: {why}");
                }
                _ => panic!("no password must refuse the listener, not open it"),
            }
        }
        // Switched on in the file, with no password beside it.
        let dir = config_dir_with("[auth]\nenabled = true\n");
        match tunnel(dir.path(), None, false) {
            Tunnel::Refused { port, why } => {
                assert_eq!(port, DEFAULT_AUTH_PORT);
                assert!(why.contains("pbkdf2_iterations") || why.contains("salt") || why.contains("hash"), "{why}");
            }
            _ => panic!("`enabled = true` with no verifier must refuse"),
        }
    }

    #[test]
    fn a_password_and_a_switch_open_it_on_the_configured_port() {
        let stored = StoredPassword::create(GOOD).expect("a verifier");
        let pairs = stored.toml_pairs();
        let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let body = set_section_keys("[auth]\nenabled = true\nport = 8200\n", SECTION, &refs);
        let dir = config_dir_with(&body);

        match tunnel(dir.path(), None, false) {
            Tunnel::Ready { port, auth } => {
                assert_eq!(port, 8200, "the port comes from the config");
                assert!(auth.login(GOOD).is_ok());
            }
            _ => panic!("a verifier and `enabled = true` is the ready case"),
        }
        // The command line outranks the file.
        match tunnel(dir.path(), Some(8500), false) {
            Tunnel::Ready { port, .. } => assert_eq!(port, 8500),
            _ => panic!("ready"),
        }
        // A password with nothing switched on is still off: setting a
        // password must not open a port by itself.
        let quiet = config_dir_with(&set_section_keys("", SECTION, &refs));
        assert!(matches!(tunnel(quiet.path(), None, false), Tunnel::Off));
        assert!(matches!(tunnel(quiet.path(), None, true), Tunnel::Ready { .. }));
    }

    #[test]
    fn a_switch_of_the_wrong_type_is_refused_rather_than_read_as_off() {
        assert!(!switches("").expect("empty").0, "nothing in the file means off");
        assert_eq!(switches("[auth]\nenabled = true\n").expect("bool"), (true, None));
        assert_eq!(switches("[auth]\nport = 9001\n").expect("port"), (false, Some(9001)));
        // `enabled = "yes"` is somebody who meant `true`. Reading it as off
        // would leave the listener shut while its owner believed it open.
        assert!(switches("[auth]\nenabled = \"yes\"\n").is_err());
        assert!(switches("[auth]\nport = 0\n").is_err());
        assert!(switches("[auth]\nport = 99999\n").is_err());
    }

    #[test]
    fn the_login_page_stands_on_its_own() {
        // It is the body of a 401 on a desk whose bundle may never have
        // loaded, so it may not reach for one.
        assert!(LOGIN_PAGE.contains("<form"));
        assert!(!LOGIN_PAGE.contains("<script src"), "no external script");
        assert!(!LOGIN_PAGE.contains("rel=\"stylesheet\""), "no external stylesheet");
        assert!(!LOGIN_PAGE.contains("/assets/"), "nothing from the bundle");
        assert!(LOGIN_PAGE.contains("/api/auth/login"));
    }
}
