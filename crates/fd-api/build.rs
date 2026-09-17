//! Bake the commit this binary is BUILT FROM into the binary.
//!
//! WHY THIS IS A BUILD SCRIPT AND NOT A RUNTIME `git rev-parse`.
//!
//! The deploy copies `target/release/fd-api.exe` to a server that separately
//! runs `git pull`, so the source on the far side and the binary running there
//! can differ with nothing anywhere disagreeing. That is the whole reason the
//! version endpoint exists (`docs/decisions/2026-09-17-deploy-staleness.md`).
//!
//! A binary that asked git at RUNTIME would report the checkout it happens to
//! be sitting in, which on the server is exactly the thing already known and
//! never the thing in doubt. It would answer "up to date" in precisely the
//! case the check is for. A build script is the only place that sees the
//! commit the CODE came from, so this is not a stylistic choice.
//!
//! When git cannot be reached - a source tarball, a build container with no
//! `.git` - the value is the literal `unknown`, never a guess and never an
//! empty string. The deploy compares for equality, so `unknown` fails the
//! comparison loudly instead of matching something by accident.

use std::process::Command;

fn main() {
    // Re-run when HEAD moves. Without these the stamp is baked once and then
    // cached across commits, which would make this file lie in exactly the
    // situation it exists to catch.
    //
    // Both are needed and neither is enough: `.git/HEAD` changes on a branch
    // switch, and the ref file changes when a commit lands on the branch you
    // are already on - which is the ordinary case.
    let git_dir = find_git_dir();
    if let Some(dir) = &git_dir {
        println!("cargo:rerun-if-changed={}/HEAD", dir.display());
        if let Ok(head) = std::fs::read_to_string(dir.join("HEAD")) {
            if let Some(reference) = head.strip_prefix("ref: ").map(str::trim) {
                println!("cargo:rerun-if-changed={}/{reference}", dir.display());
            }
        }
    }
    println!("cargo:rerun-if-changed=build.rs");

    let hash = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());

    // Whether the tree had uncommitted changes when this was built.
    //
    // A binary built from edits on top of a commit carries a hash that is TRUE
    // and a content that is not, and that is the state most likely to be
    // running on a server during an incident. The deploy is told so it can say
    // so; it is not an error here.
    let dirty = git(&["status", "--porcelain"]).map(|s| !s.trim().is_empty()).unwrap_or(false);

    // Seconds since the epoch, UTC. Not a formatted string: this crate has no
    // date library, and a number the client formats cannot be misread as a
    // local time the way a bare `2026-09-17 21:40` can. The unit is in the
    // field name, per docs/decisions/2026-09-17-unit-carrying.md.
    let built_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    println!("cargo:rustc-env=FD_GIT_HASH={hash}");
    println!("cargo:rustc-env=FD_GIT_DIRTY={dirty}");
    println!("cargo:rustc-env=FD_BUILT_AT_MS={built_at_ms}");
}

/// The `.git` directory for this crate, walking up from the manifest.
///
/// Walks rather than assuming a fixed depth, because this crate is built both
/// from the workspace root and from a `git worktree`, where `.git` is a FILE
/// pointing elsewhere rather than a directory.
fn find_git_dir() -> Option<std::path::PathBuf> {
    let mut dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").ok()?);
    loop {
        let candidate = dir.join(".git");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if candidate.is_file() {
            // A worktree: `.git` holds `gitdir: <path>`.
            let contents = std::fs::read_to_string(&candidate).ok()?;
            let path = contents.strip_prefix("gitdir:")?.trim();
            return Some(std::path::PathBuf::from(path));
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
