//! The world's GitHub Actions, as far as a pull request's checks go: running a failed job
//! again, or every failed job of a workflow run. Each pull request's checks are one workflow
//! run, `number × 1000`, and its jobs follow it in order (`number × 1000 + 1`, …), so the ids
//! the GraphQL API hands out lead back here.

use serde_json::json;

use super::State;
use super::rest::{Answer, not_found};

/// The workflow run of pull request `number`'s checks.
pub(super) fn run_id(number: u64) -> u64 {
    number * 1000
}

/// The job of check `index` (0-based) on pull request `number`.
pub(super) fn job_id(number: u64, index: usize) -> u64 {
    run_id(number) + index as u64 + 1
}

/// Answers the Actions paths, or None for any other request.
pub(super) fn answer(s: &mut State, method: &str, path: &str) -> Option<Answer> {
    let parts: Vec<&str> = path.split('?').next().unwrap_or(path).trim_matches('/').split('/').collect();
    let (owner, name, kind, id, verb) = match parts.as_slice() {
        ["repos", owner, name, "actions", kind @ ("jobs" | "runs"), id, verb] => (*owner, *name, *kind, *id, *verb),
        _ => return None,
    };
    if method != "POST" {
        return Some(not_found());
    }
    let Ok(id) = id.parse::<u64>() else { return Some(not_found()) };
    let number = id / 1000;
    let repo = format!("{owner}/{name}");
    let Some(p) = super::graphql::find_mut(s, owner, name, number) else { return Some(not_found()) };
    let rerun: Vec<usize> = match (kind, verb) {
        ("jobs", "rerun") => {
            let index = usize::try_from(id % 1000).unwrap_or(0).checked_sub(1);
            match index.filter(|i| *i < p.checks.len()) {
                Some(i) if p.checks[i].queued => return Some(already_running()),
                Some(i) => vec![i],
                None => return Some(not_found()),
            }
        }
        ("runs", "rerun-failed-jobs") if id % 1000 == 0 => {
            (0..p.checks.len()).filter(|i| !p.checks[*i].passed && !p.checks[*i].queued).collect()
        }
        _ => return Some(not_found()),
    };
    if rerun.is_empty() {
        return Some(already_running());
    }
    let names: Vec<String> = rerun
        .iter()
        .map(|i| {
            p.checks[*i].queued = true;
            p.checks[*i].name.clone()
        })
        .collect();
    s.reruns.extend(names.into_iter().map(|c| (repo.clone(), number, c)));
    Some((201, json!({}).to_string()))
}

fn already_running() -> Answer {
    (403, json!({ "message": "This job is already running or has no failed jobs to re-run" }).to_string())
}
