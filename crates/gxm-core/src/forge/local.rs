//! The git side of a pull request: which commits in the history belong to which pull request,
//! and what a pull request changes (its commits, files and diff), read from the repository.

use std::collections::HashMap;
use std::path::Path;

use super::model::{PullState, PullSummary};
use crate::diff::{self, Hunk};
use crate::env::Env;
use crate::git::{self, GitError};
use crate::history::{self, FileChange};

/// The most commits a pull request is followed through when matching it to the history.
const MAX_COMMITS: &str = "--max-count=250";

/// Which pull request each commit belongs to: an open one's own commits (those not yet on its
/// base), and a merged one's merge commit with the commits it brought in. A pull request whose
/// commits aren't in the repository (not fetched yet) matches nothing.
pub fn commit_pulls(env: &Env, repo: &Path, pulls: &[PullSummary]) -> HashMap<String, u64> {
    let mut by_commit = HashMap::new();
    for p in pulls {
        let commits = match (p.state, &p.merge_commit) {
            (PullState::Merged, Some(merge)) => {
                let brought = rev_list(env, repo, &[&format!("{merge}^2"), &format!("^{merge}^1")]);
                brought.map(|mut c| {
                    c.push(merge.clone());
                    c
                })
            }
            (PullState::Open | PullState::Draft, _) => {
                let base = base_ref(env, repo, &p.base_ref);
                rev_list(env, repo, &[&p.head_sha, &format!("^{base}")])
            }
            _ => continue,
        };
        for c in commits.unwrap_or_default() {
            // A commit in two pull requests (a stacked one) belongs to the one listed first.
            by_commit.entry(c).or_insert(p.number);
        }
    }
    by_commit
}

fn rev_list(env: &Env, repo: &Path, revs: &[&str]) -> Result<Vec<String>, GitError> {
    let out = git::run(env, repo, &[&["rev-list", MAX_COMMITS], revs].concat())?;
    Ok(out.lines().map(str::to_owned).collect())
}

/// The base branch as the remote has it (`origin/main`), else the local one.
fn base_ref(env: &Env, repo: &Path, base: &str) -> String {
    let remote = format!("origin/{base}");
    let exists = git::run(env, repo, &["rev-parse", "--verify", "--quiet", &format!("{remote}^{{commit}}")]).is_ok();
    if exists { remote } else { base.to_owned() }
}

/// What a pull request changes, as git computes it from the commits in the repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Its commits, oldest first, as (id, subject).
    pub commits: Vec<(String, String)>,
    pub files: Vec<FileChange>,
    /// Every hunk of every file, by path.
    pub diffs: HashMap<String, Vec<Hunk>>,
}

/// The pull request's diff from where it left its base, as the provider shows it (`base...head`).
pub fn changes(env: &Env, repo: &Path, p: &PullSummary) -> Result<Changes, GitError> {
    let (from, to) = match (&p.merge_commit, p.state) {
        (Some(merge), PullState::Merged) => (format!("{merge}^1"), format!("{merge}^2")),
        _ => (base_ref(env, repo, &p.base_ref), p.head_sha.clone()),
    };
    let base = git::run(env, repo, &["merge-base", &from, &to])?.trim().to_owned();
    let log = git::run(env, repo, &["log", "--reverse", "--format=%H%x1f%s", &format!("{base}..{to}")])?;
    let commits = log.lines().filter_map(|l| l.split_once('\u{1f}')).map(|(a, b)| (a.to_owned(), b.to_owned())).collect();
    let files = history::range_changes(env, repo, &base, &to)?;
    let mut diffs = HashMap::new();
    for f in &files {
        diffs.insert(f.path.clone(), diff::range_hunks(env, repo, &base, &to, &f.path)?);
    }
    Ok(Changes { commits, files, diffs })
}
