//! Advisor credentials, from the desk.
//!
//! The advisor pays for a verdict one of two ways: a metered API key, or a
//! subscription reached through a vendor CLI that is already signed in. This
//! module is the desk's view of which is configured, and the one control on it
//! that can create spending power.
//!
//! # Why this does not simply take a key and store it
//!
//! `Settings.tsx` states the rule this desk is built on: the page is served
//! over plain HTTP on a loopback port with **no authentication of any kind**,
//! so anything it can switch on, anything that reaches that port can switch on.
//! Its riskier controls are therefore facts with a reason beside them, never
//! inputs.
//!
//! An API key is exactly such a control — it is spending power, and a browser
//! is not a place to mint it unguarded. So storing one costs a **setup code**:
//! six digits generated once per process and printed on `fd-api`'s own console
//! at startup. Reading it means standing where the server runs. That keeps the
//! rule intact — a riskier change costs a word typed by someone who is awake —
//! while still letting the change be made from the screen.
//!
//! Removing a key needs no code. It can only ever reduce capability, which is
//! the safe direction the page already moves in freely.
//!
//! # Why it shells out to Python
//!
//! `py/live/advisor_setup.py` already knows what a credential is, which
//! providers take one, how to prove one with a real call, and how to edit one
//! section of `config/local.toml` without disturbing the Telegram token beside
//! it. Reimplementing five provider adapters here would put those rules in two
//! places, and the second copy is always the one that goes stale.
//!
//! **A key is never passed as an argument.** It goes over the child's stdin,
//! because argv is visible in a process list to every user on the machine.
//! Nothing in this module ever returns key material: the status rows carry a
//! mask, and that is all they ever carry.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;

use crate::error::ApiError;
use crate::state::AppState;

static SETUP_CODE: OnceLock<String> = OnceLock::new();

/// The six digits that buy one credential write, fixed for the life of the
/// process and printed where only the operator can read it.
///
/// Seeded from `RandomState`, which the standard library seeds from the OS per
/// process. That is a real source of randomness and costs no dependency; it is
/// not a cryptographic token and does not need to be, because it guards a
/// loopback port against a page that wandered in, not against someone who can
/// already read the console.
pub fn setup_code() -> &'static str {
    SETUP_CODE.get_or_init(|| {
        let mut h = RandomState::new().build_hasher();
        h.write(b"fd-api advisor setup");
        format!("{:06}", h.finish() % 1_000_000)
    })
}

/// The repository root, from the config directory the state already knows.
///
/// `config_dir` is usually the relative `config`, whose parent is the EMPTY
/// path — not `None`. Handing that to `current_dir` fails with a Windows error
/// 123 that names no path at all, which is how this cost twenty minutes the
/// first time. An empty parent means "where the process already is".
fn root(state: &AppState) -> PathBuf {
    match state.config_dir.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// Which Python runs the setup tool.
///
/// `FD_PYTHON` first so a machine with several can say which; then the usual
/// names. Resolved per call rather than cached: a desk left running while the
/// interpreter is reinstalled should notice.
fn python() -> String {
    if let Ok(p) = std::env::var("FD_PYTHON") {
        if !p.trim().is_empty() {
            return p;
        }
    }
    // `py` is the Windows launcher and is the one that is actually present on
    // a machine where `python` is the Store's stub; it is tried last so a real
    // interpreter on PATH still wins.
    for candidate in ["python3", "python", "py"] {
        if Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return candidate.to_string();
        }
    }
    "python".to_string()
}

/// Run `advisor_setup.py --json <args>` and parse the one object it prints.
///
/// `stdin` is written to the child when given; that is how a key travels.
fn run(state: &AppState, args: &[&str], stdin: Option<&str>) -> Result<Value, ApiError> {
    let dir = root(state);
    let script = dir.join("py").join("live").join("advisor_setup.py");
    if !script.exists() {
        return Err(ApiError::Internal(format!(
            "advisor_setup.py not found at {}",
            script.display()
        )));
    }
    let mut cmd = Command::new(python());
    cmd.arg(&script)
        .arg("--json")
        .args(args)
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });

    let mut child = cmd.spawn().map_err(|e| {
        // Name the interpreter AND the directory. The first version said only
        // "could not start python" and the real fault was an empty working
        // directory, which that message could not have pointed at.
        ApiError::Internal(format!(
            "could not start `{}` in {}: {e} (set FD_PYTHON to an interpreter)",
            python(),
            dir.display()
        ))
    })?;
    if let Some(text) = stdin {
        child
            .stdin
            .as_mut()
            .ok_or_else(|| ApiError::Internal("no stdin on the child".into()))?
            .write_all(text.as_bytes())
            .map_err(|e| ApiError::Internal(format!("writing to the child: {e}")))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| ApiError::Internal(format!("waiting on the child: {e}")))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    // The tool prints exactly one JSON object on the last non-empty line. Taking
    // the LAST one rather than the whole stream so a warning printed above it
    // does not make a working call look broken.
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or("");
    serde_json::from_str::<Value>(line).map_err(|_| {
        let err = String::from_utf8_lossy(&out.stderr);
        ApiError::Internal(format!(
            "advisor_setup said nothing readable (exit {:?}): {}",
            out.status.code(),
            if err.trim().is_empty() {
                stdout.trim().chars().take(300).collect::<String>()
            } else {
                err.trim().chars().take(300).collect::<String>()
            }
        ))
    })
}

/// `GET /api/advisor/credentials` — what is configured, and from where.
///
/// Masks only. No branch of this ever reads a stored key out to the browser.
pub async fn credentials(State(state): State<Arc<AppState>>) -> Result<Json<Value>, ApiError> {
    let mut view = run(&state, &["status"], None)?;
    // The code is NOT sent. Only whether one is needed, so the screen can say
    // where to find it instead of pretending the field is optional.
    if let Some(obj) = view.as_object_mut() {
        obj.insert("setup_code_required".into(), Value::Bool(true));
    }
    Ok(Json(view))
}

#[derive(Deserialize)]
pub struct TestBody {
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
}

/// `POST /api/advisor/credentials/test` — one real call, to prove a credential.
///
/// Spends a little: a metered provider bills a few tokens and a plan draws on
/// its quota. That is the point — a credential nobody has exercised is a
/// credential nobody knows about.
pub async fn test(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TestBody>,
) -> Result<Json<Value>, ApiError> {
    let mut args = vec!["test", body.provider.as_str()];
    if let Some(m) = body.model.as_deref() {
        args.push("--model");
        args.push(m);
    }
    Ok(Json(run(&state, &args, None)?))
}

#[derive(Deserialize)]
pub struct StoreBody {
    pub provider: String,
    pub key: String,
    pub setup_code: String,
}

/// `POST /api/advisor/credentials` — store an API key, against the setup code.
///
/// The key is proven with a real call inside the Python tool before it is
/// written, and is refused if it does not answer. Nothing is echoed back but a
/// mask.
pub async fn store(
    State(state): State<Arc<AppState>>,
    Json(body): Json<StoreBody>,
) -> Result<Json<Value>, ApiError> {
    if body.setup_code.trim() != setup_code() {
        // Deliberately the same message whether the code is wrong or missing:
        // a form that says which is closer is a form that can be searched.
        return Err(ApiError::BadRequest(
            "wrong setup code — it is printed on the fd-api console at startup".into(),
        ));
    }
    if body.key.trim().is_empty() {
        return Err(ApiError::BadRequest("no key given".into()));
    }
    Ok(Json(run(
        &state,
        &["key", body.provider.as_str(), "--stdin"],
        Some(body.key.trim()),
    )?))
}

#[derive(Deserialize)]
pub struct ClearBody {
    pub provider: String,
}

/// `POST /api/advisor/credentials/clear` — remove a stored key.
///
/// No setup code: this can only ever take capability away, which is the
/// direction this page is already allowed to move in.
pub async fn clear(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ClearBody>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(run(&state, &["clear", body.provider.as_str()], None)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setup_code_is_six_digits_and_does_not_change_within_a_process() {
        let a = setup_code();
        assert_eq!(a.len(), 6, "a six digit code is what the console prints");
        assert!(a.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(a, setup_code(), "it must be stable or the form can never be filled in");
    }
}
