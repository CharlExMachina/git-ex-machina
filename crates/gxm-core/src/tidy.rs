//! Tidying up merged branches: which of a repository's branches, here and on `origin`, are
//! done with (their work is in the default branch), which stay and why, and deleting the ones
//! that are done.
//!
//! A branch is done when every commit on it is in the default branch, when its pull request
//! merged at the commit it's still at (a squash leaves no trace in git), or when it carries the
//! same changes as commits already in the default branch (rebased, or cherry-picked). Nothing
//! that holds work found nowhere else is ever offered.

use std::path::Path;

use crate::env::Env;
use crate::forge::model::{PullState, PullSummary};
use crate::git::{self, GitError};

/// A branch, where it is, and what becomes of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    /// Its commit here, if this computer has it.
    pub local: Option<String>,
    /// Its commit on `origin`, as last fetched.
    pub remote: Option<String>,
    pub fate: Fate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fate {
    Done(Done),
    Kept(Kept),
}

/// Why a branch's work is already in the default branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Done {
    /// Every commit on it is in the default branch.
    InBase,
    /// Its pull request merged at the commit it's at.
    Pull(u64),
    /// Its commits' changes are in the default branch (rebased or cherry-picked).
    SameChanges,
}

/// Why a branch stays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// It's checked out.
    Current,
    /// Its pull request is still open.
    Open(u64),
    /// It has no commits of its own yet.
    Fresh,
    /// This many of its commits are in no remote branch: they exist only here.
    OnlyHere(u32),
    /// This many of its commits aren't in the default branch.
    Unmerged(u32),
}

impl Branch {
    pub fn done(&self) -> bool {
        matches!(self.fate, Fate::Done(_))
    }

    /// Where it is, as a person says it: `local and origin`, `origin`, `local`.
    pub fn place(&self) -> &'static str {
        match (self.local.is_some(), self.remote.is_some()) {
            (true, true) => "local and origin",
            (false, true) => "origin",
            _ => "local",
        }
    }

    /// Why it can go, or why it stays, in a few words.
    pub fn reason(&self) -> String {
        match self.fate {
            Fate::Done(Done::InBase) => "in the default branch".to_owned(),
            Fate::Done(Done::Pull(n)) => format!("#{n} merged"),
            Fate::Done(Done::SameChanges) => "same changes, rebased".to_owned(),
            Fate::Kept(Kept::Current) => "checked out".to_owned(),
            Fate::Kept(Kept::Open(n)) => format!("#{n} is still open"),
            Fate::Kept(Kept::Fresh) => "no commits of its own yet".to_owned(),
            Fate::Kept(Kept::OnlyHere(1)) => "1 commit only on this computer".to_owned(),
            Fate::Kept(Kept::OnlyHere(n)) => format!("{n} commits only on this computer"),
            Fate::Kept(Kept::Unmerged(1)) => "1 commit not in the default branch".to_owned(),
            Fate::Kept(Kept::Unmerged(n)) => format!("{n} commits not in the default branch"),
        }
    }
}

/// The repository's branches other than `default`, each with its fate, done ones first.
/// `pulls` are the provider's pull requests (empty without one): a squash merge is only
/// visible there.
pub fn survey(env: &Env, repo: &Path, default: &str, pulls: &[PullSummary]) -> Result<Vec<Branch>, GitError> {
    let base = ["refs/remotes/origin/", "refs/heads/"]
        .iter()
        .map(|p| format!("{p}{default}"))
        .find(|r| git::run(env, repo, &["rev-parse", "--verify", "--quiet", r]).is_ok())
        .ok_or_else(|| GitError::Parse { what: "the default branch", line: default.to_owned() })?;
    let base_tip = git::run(env, repo, &["rev-parse", &base])?.trim().to_owned();
    let current =
        git::run(env, repo, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok().map(|s| s.trim().to_owned());
    let refs = |prefix: &str| -> Result<Vec<(String, String)>, GitError> {
        let out = git::run(env, repo, &["for-each-ref", "--format=%(refname)%00%(objectname)", prefix])?;
        Ok(out
            .lines()
            .filter_map(|l| l.split_once('\0'))
            .map(|(r, sha)| (r.strip_prefix(prefix).unwrap_or(r).to_owned(), sha.to_owned()))
            .filter(|(name, _)| name != default && name != "HEAD")
            .collect())
    };
    let mut branches: Vec<Branch> = Vec::new();
    for (name, sha) in refs("refs/heads/")? {
        branches.push(Branch { name, local: Some(sha), remote: None, fate: Fate::Kept(Kept::Fresh) });
    }
    for (name, sha) in refs("refs/remotes/origin/")? {
        match branches.iter_mut().find(|b| b.name == name) {
            Some(b) => b.remote = Some(sha),
            None => branches.push(Branch { name, local: None, remote: Some(sha), fate: Fate::Kept(Kept::Fresh) }),
        }
    }
    for b in &mut branches {
        b.fate = fate(env, repo, b, &base, &base_tip, current.as_deref(), pulls)?;
    }
    branches.sort_by_key(|b| (!b.done(), b.name.clone()));
    Ok(branches)
}

fn fate(
    env: &Env,
    repo: &Path,
    b: &Branch,
    base: &str,
    base_tip: &str,
    current: Option<&str>,
    pulls: &[PullSummary],
) -> Result<Fate, GitError> {
    if current == Some(b.name.as_str()) {
        return Ok(Fate::Kept(Kept::Current));
    }
    let mine = pulls.iter().filter(|p| p.head_ref == b.name);
    if let Some(p) = mine.clone().find(|p| matches!(p.state, PullState::Open | PullState::Draft)) {
        return Ok(Fate::Kept(Kept::Open(p.number)));
    }
    // Every copy of the branch must be done for the branch to be.
    let mut done = None;
    for tip in b.local.iter().chain(&b.remote) {
        let this = if tip == base_tip {
            return Ok(Fate::Kept(Kept::Fresh));
        } else if git::run(env, repo, &["merge-base", "--is-ancestor", tip, base]).is_ok() {
            Done::InBase
        } else if let Some(p) = mine.clone().find(|p| p.state == PullState::Merged && p.head_sha == *tip) {
            Done::Pull(p.number)
        } else if same_changes(env, repo, base, tip)? {
            Done::SameChanges
        } else {
            let only_here =
                if Some(tip) == b.local.as_ref() { count(env, repo, &[tip, "--not", "--remotes"])? } else { 0 };
            return Ok(Fate::Kept(if only_here > 0 {
                Kept::OnlyHere(only_here)
            } else {
                Kept::Unmerged(count(env, repo, &[&format!("{base}..{tip}")])?)
            }));
        };
        // A pull request's merge says the most; "in the default branch" the least.
        done = Some(match (done, this) {
            (Some(Done::Pull(n)), _) | (_, Done::Pull(n)) => Done::Pull(n),
            (Some(Done::SameChanges), _) | (_, Done::SameChanges) => Done::SameChanges,
            _ => Done::InBase,
        });
    }
    Ok(done.map_or(Fate::Kept(Kept::Fresh), Fate::Done))
}

/// Every commit on `tip` but not in `base` has a twin in `base` with the same changes.
fn same_changes(env: &Env, repo: &Path, base: &str, tip: &str) -> Result<bool, GitError> {
    let out = git::run(env, repo, &["cherry", base, tip])?;
    Ok(!out.trim().is_empty() && out.lines().all(|l| l.starts_with('-')))
}

fn count(env: &Env, repo: &Path, range: &[&str]) -> Result<u32, GitError> {
    let mut args = vec!["rev-list", "--count"];
    args.extend_from_slice(range);
    let n = git::run(env, repo, &args)?;
    n.trim().parse().map_err(|_| GitError::Parse { what: "a commit count", line: n })
}

/// What [`delete`] removed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Deleted {
    pub local: Vec<String>,
    pub remote: Vec<String>,
}

impl Deleted {
    /// `Deleted 4 branches`, counting a branch once wherever it was.
    pub fn sentence(&self) -> String {
        let mut names: Vec<&String> = self.local.iter().chain(&self.remote).collect();
        names.sort();
        names.dedup();
        match names.len() {
            1 => format!("Deleted {}", names[0]),
            n => format!("Deleted {n} branches"),
        }
    }
}

/// Deletes `branches` (done ones only): on `origin` with one push, then here. Each copy goes
/// only if it's still at the commit the survey saw, so nothing that moved since is lost.
pub fn delete(env: &Env, repo: &Path, branches: &[Branch]) -> Result<Deleted, GitError> {
    let mut deleted = Deleted::default();
    let done: Vec<&Branch> = branches.iter().filter(|b| b.done()).collect();
    let remote: Vec<(&str, &str)> =
        done.iter().filter_map(|b| b.remote.as_deref().map(|sha| (b.name.as_str(), sha))).collect();
    if !remote.is_empty() {
        let mut args: Vec<String> = vec!["push".into(), "--porcelain".into()];
        args.extend(remote.iter().map(|(name, sha)| format!("--force-with-lease=refs/heads/{name}:{sha}")));
        args.push("origin".into());
        args.extend(remote.iter().map(|(name, _)| format!(":refs/heads/{name}")));
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        git::run(env, repo, &args)?;
        deleted.remote = remote.iter().map(|(name, _)| (*name).to_owned()).collect();
    }
    for b in &done {
        if let Some(sha) = &b.local {
            git::run(env, repo, &["update-ref", "-d", &format!("refs/heads/{}", b.name), sha])?;
            // Its tracking settings go with it, as `git branch -d` does; there may be none.
            git::run(env, repo, &["config", "--remove-section", &format!("branch.{}", b.name)]).ok();
            deleted.local.push(b.name.clone());
        }
    }
    Ok(deleted)
}

/// After a pull request merged on the provider: deletes the local copy of its branch when
/// it's still at the commit that merged and isn't checked out. True when it went.
pub fn after_merge(env: &Env, repo: &Path, branch: &str, merged_at: &str) -> Result<bool, GitError> {
    let current = git::run(env, repo, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok();
    if current.as_deref().map(str::trim) == Some(branch) {
        return Ok(false);
    }
    let Ok(tip) = git::run(env, repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]) else {
        return Ok(false);
    };
    if tip.trim() != merged_at {
        return Ok(false);
    }
    git::run(env, repo, &["update-ref", "-d", &format!("refs/heads/{branch}"), merged_at])?;
    git::run(env, repo, &["config", "--remove-section", &format!("branch.{branch}")]).ok();
    Ok(true)
}

/// The default branch as this repository knows it without asking the provider: what
/// `origin/HEAD` points at, else `main`, else `master`.
pub fn default_branch(env: &Env, repo: &Path) -> String {
    if let Ok(head) = git::run(env, repo, &["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"]) {
        return head.trim().trim_start_matches("origin/").to_owned();
    }
    let has = |b: &str| git::run(env, repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{b}")]).is_ok();
    if !has("main") && has("master") { "master".to_owned() } else { "main".to_owned() }
}
