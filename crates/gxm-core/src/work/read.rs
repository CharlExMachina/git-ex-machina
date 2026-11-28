//! Reading a repository's work: its checkouts from git ([`Local`], quick, no network), its open
//! pull requests from the provider with what git adds to them ([`Open`]), and the two put
//! together ([`Work`]). Every call blocks, so the app makes them off the UI thread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::conflicts::{self, Conflict};
use super::rules::{self, Entry};
use super::worktree::{self, Worktree};
use crate::env::Env;
use crate::forge::model::{Mergeable, Pull, PullSummary, User};
use crate::forge::{Forge, ForgeError};
use crate::git::{self, GitError};
use crate::tidy;

/// What git alone says: the default branch and every checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Local {
    pub repo: PathBuf,
    /// The default branch's name: `main`.
    pub base: String,
    /// What it's compared with: `origin/main`, or `main` without a remote copy.
    pub base_ref: String,
    pub worktrees: Vec<Worktree>,
}

/// What the provider says, and what git works out from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
    pub viewer: User,
    /// Open pull requests in full, drafts included.
    pub pulls: Vec<Pull>,
    /// Every branch a pull request came from, merged and closed ones too.
    pub branches: Vec<String>,
    /// For each pull request that conflicts with its base: in what, and since which merge.
    pub conflicts: HashMap<u64, Conflict>,
    /// Lines added and removed, when its commits are here.
    pub stats: HashMap<u64, (u32, u32)>,
}

/// A repository's work, as the list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Work {
    pub local: Local,
    /// None until the provider answers, or without one.
    pub open: Option<Open>,
    pub entries: Vec<Entry>,
}

impl Local {
    pub fn read(env: &Env, repo: &Path) -> Result<Local, GitError> {
        let base = tidy::default_branch(env, repo);
        let remote = format!("origin/{base}");
        let has_remote = git::run(env, repo, &["rev-parse", "--verify", "--quiet", &remote]).is_ok();
        let base_ref = if has_remote { remote } else { base.clone() };
        let worktrees = worktree::list(env, repo, &base_ref)?;
        Ok(Local { repo: repo.to_path_buf(), base, base_ref, worktrees })
    }

    /// The checkout on `branch`, if there is one.
    pub fn on_branch(&self, branch: &str) -> Option<&Worktree> {
        self.worktrees.iter().find(|w| w.branch.as_deref() == Some(branch))
    }
}

impl Open {
    /// Asks `forge` for the viewer and the open pull requests, then git for their conflicts
    /// and sizes.
    pub fn read(env: &Env, local: &Local, forge: &dyn Forge) -> Result<Open, ForgeError> {
        let viewer = forge.viewer()?;
        let pulls = forge.open_pulls()?;
        let all: Vec<PullSummary> = forge.pulls()?;
        let merged: Vec<PullSummary> = all.iter().filter(|p| p.merge_commit.is_some()).cloned().collect();
        let mut branches: Vec<String> = all.iter().map(|p| p.head_ref.clone()).collect();
        branches.sort();
        branches.dedup();
        let repo = &local.repo;
        let mut conflicts = HashMap::new();
        let mut stats = HashMap::new();
        for p in &pulls {
            let head = &p.summary.head_sha;
            let here = git::run(env, repo, &["cat-file", "-e", &format!("{head}^{{commit}}")]).is_ok();
            if p.mergeable == Mergeable::Conflicts
                && let Some(c) = conflicts::find(env, repo, &local.base_ref, &local.base, head, &merged)
            {
                conflicts.insert(p.summary.number, c);
            }
            if here
                && let Ok(out) = git::run(env, repo, &["diff", "--shortstat", &format!("{}...{head}", local.base_ref)])
            {
                stats.insert(p.summary.number, shortstat(&out));
            }
        }
        Ok(Open { viewer, pulls, branches, conflicts, stats })
    }

    pub fn pull(&self, number: u64) -> Option<&Pull> {
        self.pulls.iter().find(|p| p.summary.number == number)
    }
}

impl Work {
    /// The list from what's known so far.
    pub fn new(local: Local, open: Option<Open>) -> Work {
        let entries = match &open {
            Some(o) => {
                let branches: Vec<&str> = o.branches.iter().map(String::as_str).collect();
                rules::entries(Some(&o.viewer.login), &o.pulls, &branches, &local.worktrees)
            }
            None => rules::entries(None, &[], &[], &local.worktrees),
        };
        Work { local, open, entries }
    }

    /// Reads it all, asking `forge` when there is one.
    pub fn read(env: &Env, repo: &Path, forge: Option<&dyn Forge>) -> Result<Work, String> {
        let local = Local::read(env, repo).map_err(|e| e.to_string())?;
        let open = match forge {
            Some(f) => Some(Open::read(env, &local, f).map_err(|e| format!("{}: {e}", f.provider().name()))?),
            None => None,
        };
        Ok(Work::new(local, open))
    }

    pub fn pull(&self, number: u64) -> Option<&Pull> {
        self.open.as_ref()?.pull(number)
    }

    /// The conflicts git found for pull request `number`.
    pub fn conflict(&self, number: u64) -> Option<&Conflict> {
        self.open.as_ref()?.conflicts.get(&number)
    }

    /// How many rows each section has.
    pub fn count(&self, section: rules::Section) -> usize {
        self.entries.iter().filter(|e| e.section == section).count()
    }
}

/// ` 3 files changed, 184 insertions(+), 22 deletions(-)` → (184, 22).
fn shortstat(out: &str) -> (u32, u32) {
    let mut added = 0;
    let mut removed = 0;
    for part in out.split(',') {
        let mut words = part.split_whitespace();
        let (Some(n), Some(what)) = (words.next().and_then(|n| n.parse().ok()), words.next()) else { continue };
        if what.starts_with("insertion") {
            added = n;
        } else if what.starts_with("deletion") {
            removed = n;
        }
    }
    (added, removed)
}

#[cfg(test)]
mod tests {
    #[test]
    fn shortstat_reads_added_and_removed_lines() {
        assert_eq!(super::shortstat(" 3 files changed, 184 insertions(+), 22 deletions(-)\n"), (184, 22));
        assert_eq!(super::shortstat(" 1 file changed, 1 insertion(+)\n"), (1, 0));
        assert_eq!(super::shortstat(""), (0, 0));
    }
}
