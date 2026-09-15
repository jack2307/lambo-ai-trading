//! What the research loop is doing, read from the files it writes.
//!
//! The loop (`/research`, see `.claude/skills/research`) leaves everything on
//! disk: a hypothesis file with a `Status:` line, a batch file, run receipts
//! under `docs/research/runs/<id>/`, the backlog, the decision records. This
//! endpoint reads those and nothing else, so what the dashboard shows is what
//! is written down — the same thing a reviewer would read — and not a
//! separate account of events that could drift from it.
//!
//! Parsing is deliberately plain: the receipts are the `search` binary's own
//! printed tables, and the rows are read by whitespace-split columns. If the
//! table format changes, the test at the bottom fails before the dashboard
//! goes blank.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Research {
    /// Newest modification time across everything read, ms since the epoch.
    pub updated_at: i64,
    pub hypotheses: Vec<Hypothesis>,
    pub backlog: Backlog,
    pub decisions: Vec<Decision>,
    /// What the scouting team has proposed, and what the gates said about it.
    /// See `docs/research/SCOUTING.md`.
    pub scouting: Vec<Proposal>,
}

/// One idea, before it is a hypothesis.
///
/// Read from `docs/research/scouting/*.json` and passed through almost
/// untouched — the one thing this reader computes is `status`, because a role
/// that could set its own proposal's status could promote it, and none of them
/// can. Everything else is what the agents wrote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub proposed_at: i64,
    #[serde(default)]
    pub scout: ScoutFields,
    #[serde(default)]
    pub verdicts: Vec<Verdict>,
    /// Derived here from the verdicts, never read from the file.
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub registered_as: Option<String>,
    #[serde(default)]
    pub file: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoutFields {
    #[serde(default)]
    pub mechanism: String,
    #[serde(default)]
    pub why_unarbitraged: String,
    #[serde(default)]
    pub data_needed: Vec<String>,
    #[serde(default)]
    pub falsifier_sketch: String,
    #[serde(default)]
    pub closest_known: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub role: String,
    pub verdict: String,
    #[serde(default)]
    pub at: i64,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Feasibility fills these; the others leave them empty.
    #[serde(default)]
    pub instrument: String,
    #[serde(default)]
    pub sample: String,
}

/// A verdict that kills a proposal outright, whoever said it.
fn is_fatal(v: &str) -> bool {
    matches!(v.to_ascii_uppercase().as_str(), "CLOSED" | "BLOCKED" | "BROKEN")
}

/// The status, from the verdicts alone.
///
/// Derived rather than stored so that no role can promote its own work: a
/// proposal is shortlisted only when BOTH gates have spoken and neither
/// objected, and one fatal verdict is enough to reject it however many
/// approvals it collected first.
fn status_of(p: &Proposal) -> String {
    if p.registered_as.as_ref().is_some_and(|r| !r.is_empty()) {
        return "registered".to_string();
    }
    if p.verdicts.iter().any(|v| is_fatal(&v.verdict)) {
        return "rejected".to_string();
    }
    let gate = |role: &str| p.verdicts.iter().any(|v| v.role.eq_ignore_ascii_case(role));
    if gate("historian") && gate("feasibility") {
        return "shortlisted".to_string();
    }
    "proposed".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hypothesis {
    pub id: String,
    pub claim: String,
    /// The `Status:` line, verbatim: registered, implemented, in-sample run,
    /// out-of-sample run, decided.
    pub status: String,
    pub registered: String,
    pub in_sample: Option<String>,
    pub out_of_sample: Option<String>,
    pub batch: Vec<BatchEntry>,
    pub runs: Runs,
    pub modified_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEntry {
    pub label: String,
    pub base: String,
    pub why: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Runs {
    pub in_sample: Option<Run>,
    pub out_of_sample: Option<Run>,
    pub direction: Vec<DirectionNull>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub rows: Vec<RunRow>,
    pub survivors: Vec<String>,
    /// The verdict line has been printed; a run without it is still going.
    pub concluded: bool,
    pub modified_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunRow {
    pub label: String,
    pub base: String,
    pub trades: usize,
    pub profit_factor: f64,
    pub expectancy: f64,
    pub null_p50: f64,
    pub null_p95: f64,
    pub percentile: f64,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DirectionNull {
    pub base: String,
    pub trades: Option<usize>,
    pub actual_pf: f64,
    pub percentile: f64,
    pub outside: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backlog {
    pub open: Vec<BacklogItem>,
    pub closed: Vec<BacklogItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BacklogItem {
    pub title: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub file: String,
    pub date: String,
    pub title: String,
}

pub async fn research(State(state): State<Arc<AppState>>) -> Result<Json<Research>, ApiError> {
    Ok(Json(read_research(&state.docs)))
}

/// Everything under `docs/` the loop writes, as one snapshot.
#[must_use]
pub fn read_research(docs: &Path) -> Research {
    let mut updated_at = 0i64;
    let mut hypotheses: Vec<Hypothesis> = list_files(&docs.join("hypotheses"), "md")
        .into_iter()
        .filter(|p| p.file_stem().is_some_and(|s| s != "TEMPLATE"))
        .filter_map(|p| read_hypothesis(docs, &p))
        .collect();
    // In flight first, then newest.
    hypotheses.sort_by(|a, b| {
        let a_done = a.status.starts_with("decided");
        let b_done = b.status.starts_with("decided");
        a_done.cmp(&b_done).then(b.modified_at.cmp(&a.modified_at))
    });
    for h in &hypotheses {
        updated_at = updated_at.max(h.modified_at);
    }
    let backlog_path = docs.join("research").join("BACKLOG.md");
    updated_at = updated_at.max(mtime(&backlog_path));
    let backlog = std::fs::read_to_string(&backlog_path).map(|t| parse_backlog(&t)).unwrap_or_default();

    let mut decisions: Vec<Decision> = list_files(&docs.join("decisions"), "md")
        .into_iter()
        .filter(|p| p.file_stem().is_some_and(|s| s != "TEMPLATE"))
        .filter_map(|p| {
            updated_at = updated_at.max(mtime(&p));
            let file = p.file_name()?.to_string_lossy().to_string();
            let text = std::fs::read_to_string(&p).ok()?;
            let title = text.lines().next()?.trim_start_matches('#').trim().to_string();
            Some(Decision { date: file.get(..10).unwrap_or("").to_string(), file, title })
        })
        .collect();
    decisions.sort_by(|a, b| b.file.cmp(&a.file));

    let mut scouting: Vec<Proposal> = list_files(&docs.join("research").join("scouting"), "json")
        .into_iter()
        .filter_map(|p| {
            updated_at = updated_at.max(mtime(&p));
            // A half-written file is an agent that is still typing, not a
            // corrupt repository: skip it and pick it up on the next poll
            // rather than failing the whole screen.
            let text = std::fs::read_to_string(&p).ok()?;
            let mut parsed: Proposal = serde_json::from_str(&text).ok()?;
            parsed.file = p.file_name()?.to_string_lossy().to_string();
            parsed.status = status_of(&parsed);
            Some(parsed)
        })
        .collect();
    // Live work first, then the graveyard, newest within each.
    let rank = |s: &str| match s {
        "shortlisted" => 0,
        "registered" => 1,
        "proposed" => 2,
        _ => 3,
    };
    scouting.sort_by(|a, b| {
        rank(&a.status).cmp(&rank(&b.status)).then(b.proposed_at.cmp(&a.proposed_at))
    });

    Research { updated_at, hypotheses, backlog, decisions, scouting }
}

fn read_hypothesis(docs: &Path, md: &Path) -> Option<Hypothesis> {
    let id = md.file_stem()?.to_string_lossy().to_string();
    let text = std::fs::read_to_string(md).ok()?;
    let first = text.lines().next().unwrap_or("").trim_start_matches('#').trim();
    let claim = first.split_once(':').map_or(first, |(_, c)| c.trim()).to_string();
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(&format!("**{name}:**")))
            .map(|v| v.split(" — ").next().unwrap_or(v).trim().to_string())
            .unwrap_or_default()
    };
    let mut modified_at = mtime(md);

    let toml_path = md.with_extension("toml");
    let (in_sample, out_of_sample, batch) = std::fs::read_to_string(&toml_path)
        .ok()
        .and_then(|t| toml::from_str::<BatchToml>(&t).ok())
        .map(|b| {
            let run = b.run.unwrap_or_default();
            (
                run.in_sample,
                run.out_of_sample,
                b.hypothesis.into_iter().map(|h| BatchEntry { label: h.label, base: h.base, why: h.why }).collect(),
            )
        })
        .unwrap_or((None, None, Vec::new()));
    modified_at = modified_at.max(mtime(&toml_path));

    let runs_dir = docs.join("research").join("runs").join(&id);
    let mut runs = Runs::default();
    for (name, slot) in [("in-sample.txt", 0), ("out-of-sample.txt", 1)] {
        let p = runs_dir.join(name);
        if let Ok(t) = std::fs::read_to_string(&p) {
            let m = mtime(&p);
            modified_at = modified_at.max(m);
            let run = parse_run(&t, m);
            if slot == 0 { runs.in_sample = Some(run) } else { runs.out_of_sample = Some(run) }
        }
    }
    for p in list_files(&runs_dir, "txt") {
        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if let Some(base) = name.strip_prefix("direction-")
            && let Ok(t) = std::fs::read_to_string(&p)
        {
            modified_at = modified_at.max(mtime(&p));
            if let Some(d) = parse_direction(base, &t) {
                runs.direction.push(d);
            }
        }
    }

    Some(Hypothesis {
        id,
        claim,
        status: field("Status"),
        registered: field("Registered"),
        in_sample,
        out_of_sample,
        batch,
        runs,
        modified_at,
    })
}

#[derive(Debug, Default, Deserialize)]
struct RunToml {
    in_sample: Option<String>,
    out_of_sample: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HypothesisToml {
    label: String,
    base: String,
    #[serde(default)]
    why: String,
}

#[derive(Debug, Deserialize)]
struct BatchToml {
    run: Option<RunToml>,
    #[serde(default)]
    hypothesis: Vec<HypothesisToml>,
}

/// The `--mode=hypotheses` table: rows begin after the `hypothesis` header,
/// continuation lines (filters and reasons) are indented and skipped.
pub fn parse_run(text: &str, modified_at: i64) -> Run {
    let mut rows = Vec::new();
    let mut in_table = false;
    let mut survivors = Vec::new();
    let mut concluded = false;
    for line in text.lines() {
        if line.starts_with("hypothesis ") {
            in_table = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix("Survivors: ") {
            survivors = rest.split(" — ").next().unwrap_or("").split(", ").map(|s| s.trim().to_string()).collect();
            concluded = true;
            continue;
        }
        if line.starts_with("Nothing survived") {
            concluded = true;
            continue;
        }
        if !in_table || line.trim().is_empty() || line.starts_with(' ') {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 10 {
            continue;
        }
        let num = |s: &str| s.trim_end_matches('%').parse::<f64>().unwrap_or(f64::NAN);
        let Ok(trades) = cols[2].parse::<usize>() else { continue };
        rows.push(RunRow {
            label: cols[0].to_string(),
            base: cols[1].to_string(),
            trades,
            profit_factor: num(cols[3]),
            expectancy: num(cols[4]),
            null_p50: num(cols[5]),
            null_p95: num(cols[6]),
            percentile: num(cols[8]),
            verdict: cols[9..].join(" "),
        });
    }
    Run { rows, survivors, concluded, modified_at }
}

/// The `--mode=null-dir` receipt: the actual profit factor and where it fell.
pub fn parse_direction(base: &str, text: &str) -> Option<DirectionNull> {
    let actual_line = text.lines().find(|l| l.trim_start().starts_with("actual:"))?;
    let cols: Vec<&str> = actual_line.split_whitespace().collect();
    let actual_pf = cols.get(1)?.parse::<f64>().ok()?;
    let percentile = cols.iter().find(|c| c.ends_with("th") || c.ends_with("st") || c.ends_with("nd") || c.ends_with("rd"))
        .and_then(|c| c.trim_end_matches(char::is_alphabetic).parse::<f64>().ok())?;
    let trades = text
        .lines()
        .find(|l| l.starts_with("== direction control"))
        .and_then(|l| l.split(", ").nth(1))
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse().ok());
    Some(DirectionNull { base: base.to_string(), trades, actual_pf, percentile, outside: text.contains("Outside its own null") })
}

/// `- [ ] **title.** note` / `- [x] **title.** note`.
pub fn parse_backlog(text: &str) -> Backlog {
    let mut backlog = Backlog::default();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let (open, rest) = if let Some(r) = trimmed.strip_prefix("- [ ] ") {
            (true, r)
        } else if let Some(r) = trimmed.strip_prefix("- [x] ") {
            (false, r)
        } else {
            continue;
        };
        // Withdrawn items are struck through in the file; they read as closed.
        let withdrawn = rest.starts_with("~~");
        let plain = |t: &str| t.replace("~~", "").replace('`', "").replace("**", "").trim().to_string();
        let (title, note) = match rest.trim_start_matches("~~").strip_prefix("**").and_then(|r| r.split_once("**")) {
            Some((t, n)) => (plain(t).trim_end_matches('.').to_string(), plain(n)),
            None => (plain(rest), String::new()),
        };
        let item = BacklogItem { title, note };
        if open && !withdrawn { backlog.open.push(item) } else { backlog.closed.push(item) }
    }
    backlog
}

fn list_files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == ext))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hypotheses_table_is_read_row_by_row_and_the_verdict_line_closes_it() {
        let text = "\
== hypotheses `x`: 2 declared ==

hypothesis   base               trades  OOS PF  expect null p50 null p95    swap$   pct  verdict
ict-B-balanced ict-sweep-mss-fvg      62   1.261   0.145    0.888    1.058        0  100%  SURVIVES
                                MoTuWeThFr + NY 01:00-05:00  — the expert's default preset
ict-A-tight  ict-sweep-mss-fvg      33   0.917  -0.051    0.888    1.058        0   62%  fail: profit factor 0.917 < 1.2; expectancy -0.051R < 0.05R

Survivors: ict-B-balanced/ict-sweep-mss-fvg — worth a decision record
";
        let run = parse_run(text, 5);
        assert_eq!(run.rows.len(), 2);
        assert_eq!(run.rows[0].label, "ict-B-balanced");
        assert_eq!(run.rows[0].trades, 62);
        assert_eq!(run.rows[0].percentile, 100.0);
        assert_eq!(run.rows[0].verdict, "SURVIVES");
        assert!(run.rows[1].verdict.starts_with("fail:"));
        assert_eq!(run.survivors, vec!["ict-B-balanced/ict-sweep-mss-fvg".to_string()]);
        assert!(run.concluded);

        let unfinished = parse_run("hypothesis   base trades\n", 1);
        assert!(!unfinished.concluded && unfinished.rows.is_empty());
    }

    #[test]
    fn the_direction_receipt_yields_the_percentile_and_the_side_of_the_null() {
        let text = "== direction control: ict-sweep-mss-fvg, 198 trades, 2000 coin-flip assignments ==\n\n  actual: 1.045  ->  79th percentile  (p = 0.210)\n\n  Inside its own null.\n";
        let d = parse_direction("ict-sweep-mss-fvg", text).unwrap();
        assert_eq!(d.trades, Some(198));
        assert_eq!(d.actual_pf, 1.045);
        assert_eq!(d.percentile, 79.0);
        assert!(!d.outside);
    }

    #[test]
    fn the_backlog_splits_open_from_closed_and_keeps_the_reason() {
        let text = "## Open
- [ ] **ORB.** the range is where flow was absorbed
- [ ] ~~**Asia.**~~ Withdrawn
- [ ] `run_null_control` skips **options**
## Closed
- [x] **ICT.** see record
";
        let b = parse_backlog(text);
        assert_eq!(b.open[0], BacklogItem { title: "ORB".into(), note: "the range is where flow was absorbed".into() });
        assert_eq!(b.open[1], BacklogItem { title: "run_null_control skips options".into(), note: String::new() });
        assert_eq!(b.open.len(), 2, "a struck-through item is not open");
        assert_eq!(b.closed.len(), 2);
    }
}
