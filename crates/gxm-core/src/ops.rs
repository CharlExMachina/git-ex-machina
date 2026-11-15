//! The git operations the toolbar runs: talking to the remote (fetch, pull, push), the working
//! tree (stage, unstage, discard, commit) and history (checkout, cherry-pick, revert).

use std::path::Path;

use crate::env::Env;
use crate::git::{self, GitError};

/// Where the changes "Discard all" sets aside go, so they can be brought back.
pub const DISCARD_STASH: &str = "Discarded in git-ex-machina";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Fetch,
    Pull,
    Push,
    StageAll,
    UnstageAll,
    /// Sets every uncommitted change aside in a stash (untracked files too): nothing is lost.
    DiscardAll,
    /// Commits what's staged, with this message.
    Commit(String),
    /// Checks out a branch by name, or a commit by id (detached).
    Checkout(String),
    /// Applies these commits on top of HEAD, oldest first.
    CherryPick(Vec<String>),
    /// Adds a commit undoing this one.
    Revert(String),
}

impl Op {
    /// The name a button or a message gives it.
    pub fn verb(&self) -> &'static str {
        match self {
            Op::Fetch => "Fetch",
            Op::Pull => "Pull",
            Op::Push => "Push",
            Op::StageAll => "Stage all",
            Op::UnstageAll => "Unstage all",
            Op::DiscardAll => "Discard all",
            Op::Commit(_) => "Commit",
            Op::Checkout(_) => "Checkout",
            Op::CherryPick(_) => "Cherry-pick",
            Op::Revert(_) => "Revert",
        }
    }

    fn args(&self) -> Vec<&str> {
        match self {
            Op::Fetch => vec!["fetch", "--prune", "--quiet"],
            // Fast-forward only: a pull never makes a merge commit behind the person's back.
            Op::Pull => vec!["pull", "--ff-only", "--quiet"],
            Op::Push => vec!["push", "--quiet"],
            Op::StageAll => vec!["add", "--all"],
            Op::UnstageAll => vec!["reset", "--quiet"],
            Op::DiscardAll => vec!["stash", "push", "--include-untracked", "--quiet", "--message", DISCARD_STASH],
            Op::Commit(message) => vec!["commit", "--quiet", "--message", message],
            Op::Checkout(target) => vec!["checkout", "--quiet", target],
            Op::CherryPick(ids) => ["cherry-pick"].into_iter().chain(ids.iter().map(String::as_str)).collect(),
            Op::Revert(id) => vec!["revert", "--no-edit", id],
        }
    }
}

/// Runs `op` in `repo`. Fetch, pull and push block on the network, so the app runs every
/// operation off the UI thread.
pub fn run(env: &Env, repo: &Path, op: &Op) -> Result<(), GitError> {
    if *op == Op::Push && !has_upstream(env, repo) {
        return publish(env, repo);
    }
    git::run(env, repo, &op.args()).map(|_| ())
}

/// Whether the checked-out branch tracks a remote branch.
pub fn has_upstream(env: &Env, repo: &Path) -> bool {
    git::run(env, repo, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"]).is_ok()
}

/// Pushes a branch that has never been pushed, and makes the remote branch its upstream, so the
/// next push and pull just work.
fn publish(env: &Env, repo: &Path) -> Result<(), GitError> {
    let remote = publish_remote(env, repo)?;
    git::run(env, repo, &["push", "--quiet", "--set-upstream", &remote, "HEAD"]).map(|_| ())
}

/// Where a new branch goes: `remote.pushDefault`, else `origin`, else the only remote.
fn publish_remote(env: &Env, repo: &Path) -> Result<String, GitError> {
    let configured = git::run(env, repo, &["config", "--get", "remote.pushDefault"]).unwrap_or_default();
    if !configured.trim().is_empty() {
        return Ok(configured.trim().to_owned());
    }
    let out = git::run(env, repo, &["remote"])?;
    let remotes: Vec<&str> = out.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let refuse = |why: &str| GitError::Failed { args: "push".to_owned(), code: None, stderr: format!("fatal: {why}") };
    match remotes.as_slice() {
        _ if remotes.contains(&"origin") => Ok("origin".to_owned()),
        [one] => Ok((*one).to_owned()),
        [] => Err(refuse("this repository has no remote to push to")),
        _ => Err(refuse("there are several remotes and none is origin: set remote.pushDefault to choose")),
    }
}
