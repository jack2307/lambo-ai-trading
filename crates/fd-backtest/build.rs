//! Capture the commit this crate was built from, for the argv sidecar.
//!
//! `agent/hold-cap` measured two independent `--release` builds of one commit
//! producing byte-identical output, so **the reproduction base is the commit**,
//! not a particular `search.exe` on a particular disk. A sidecar that named a
//! binary path would name the wrong thing, and a sidecar that named nothing
//! would leave the reader guessing — which is the whole defect it exists to
//! close.
//!
//! No `rerun-if-changed` is emitted on purpose: cargo's default is to re-run
//! this script when any file in the package changes, and narrowing it to
//! `build.rs` would freeze the commit at whatever it was the first time. The
//! captured value is still a BUILD-time fact and the sidecar labels it as one.
//!
//! When `git` cannot be reached the variables are simply not set, and the
//! sidecar prints `null` rather than a placeholder — `null != 0 != []`.

fn main() {
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git").args(args).output().ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    if let Some(head) = git(&["rev-parse", "HEAD"]) {
        if head.is_empty() {
            return;
        }
        let dirty = git(&["status", "--porcelain"]).map_or("unknown", |s| if s.is_empty() { "clean" } else { "dirty" });
        println!("cargo::rustc-env=FD_GIT_COMMIT={head}");
        println!("cargo::rustc-env=FD_GIT_DIRTY={dirty}");
    }
}
