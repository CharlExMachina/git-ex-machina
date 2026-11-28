//! The git of resolving: the base fetched and merged into the worktree without committing,
//! every conflict read; resolutions written; the files read again after an agent; the merge
//! committed and the branch pushed; or the merge aborted.

use std::collections::HashMap;
use std::path::Path;

use super::lockfile::Lockfile;
use super::{By, File, Merge, Regenerated, Target, WhyPull, why};
use crate::env::Env;
use crate::forge::Forge;
use crate::forge::local;
use crate::forge::model::PullSummary;
use crate::git;
use crate::ops::{self, Op};

/// Why a commit and push didn't go through, and how far it got.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotPushed {
    /// Nothing was committed: the merge is still under way.
    Commit(String),
    /// The merge is committed here; the push failed.
    Push(String),
}

fn said(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn merging(env: &Env, dir: &Path) -> Option<String> {
    git::run(env, dir, &["rev-parse", "--quiet", "--verify", "MERGE_HEAD"]).ok().map(|s| s.trim().to_owned())
}

/// The files git still calls conflicted.
fn unmerged(env: &Env, dir: &Path) -> Result<Vec<String>, String> {
    let out = git::run(env, dir, &["diff", "--name-only", "--diff-filter=U"]).map_err(said)?;
    let mut files: Vec<String> = out.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect();
    files.dedup();
    Ok(files)
}

/// Fetches `target`'s base and merges it into the worktree, stopping before the commit; or,
/// when a merge of it is already under way there, picks that one up. `forge` names the pull
/// requests behind the base's changes.
pub fn begin(env: &Env, target: &Target, forge: Option<&dyn Forge>) -> Result<Merge, String> {
    let dir = &target.worktree;
    let head = git::run(env, dir, &["rev-parse", "HEAD"]).map_err(said)?.trim().to_owned();
    let base_commit = match merging(env, dir) {
        Some(theirs) => theirs,
        None => {
            let dirty = git::run(env, dir, &["status", "--porcelain", "--untracked-files=no"]).map_err(said)?;
            if !dirty.trim().is_empty() {
                return Err(format!(
                    "{} has uncommitted changes. Commit or stash them first, so aborting the merge can put everything back.",
                    target.worktree_name
                ));
            }
            git::run(env, dir, &["fetch", "--quiet", &target.remote, &target.base])
                .map_err(|e| format!("Couldn't fetch {}: {e}", target.base))?;
            let base = target.base_ref();
            let out = git::command(env, dir, &["merge", "--no-ff", "--no-commit", "--no-edit", &base])
                .output()
                .map_err(said)?;
            if merging(env, dir).is_none() {
                let words = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
                return Err(if out.status.success() {
                    format!("{} already has everything in {}: there's nothing to merge.", target.branch, target.base)
                } else {
                    format!("git couldn't merge {base}: {}", words.trim())
                });
            }
            git::run(env, dir, &["rev-parse", &base]).map_err(said)?.trim().to_owned()
        }
    };
    let mut merge = Merge {
        head,
        base_commit,
        files: Vec::new(),
        regenerated: Vec::new(),
        lockfiles: Vec::new(),
        elsewhere: Vec::new(),
    };
    for path in unmerged(env, dir)? {
        if let Some(kind) = Lockfile::of(&path) {
            match regenerate(env, dir, &path, kind) {
                Some(Ok(with)) => {
                    merge.regenerated.push(Regenerated { path, with });
                    continue;
                }
                Some(Err(_)) => {
                    // Back to the conflict git left, to resolve like any other file.
                    git::run(env, dir, &["checkout", "--merge", "--", &path]).map_err(said)?;
                }
                None => {}
            }
            merge.lockfiles.push((path.clone(), kind.tool()));
        }
        let text = read(env, dir, &path).unwrap_or_default();
        let file = File::parse(&path, &text);
        if file.conflicts.is_empty() {
            merge.elsewhere.push(path);
        } else {
            merge.files.push(file);
        }
    }
    explain(env, target, &mut merge, forge);
    Ok(merge)
}

/// A conflicted lockfile, as the base has it and written again by its tool: None when the tool
/// isn't on the path.
fn regenerate(env: &Env, dir: &Path, path: &str, kind: Lockfile) -> Option<Result<String, String>> {
    let tool = env.which(kind.tool())?;
    let run = || {
        git::run(env, dir, &["checkout", "--theirs", "--", path]).map_err(said)?;
        let folder = dir.join(path).parent().map(Path::to_path_buf).unwrap_or_else(|| dir.to_path_buf());
        let mut cmd = std::process::Command::new(&tool);
        cmd.args(kind.regenerate()).current_dir(&folder);
        crate::harness::probe::child_env(env, &mut cmd);
        let out = cmd.output().map_err(said)?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
        }
        git::run(env, dir, &["add", "--", path]).map_err(said)?;
        Ok(kind.command())
    };
    Some(run())
}

fn read(env: &Env, dir: &Path, path: &str) -> Result<String, String> {
    let at = dir.join(path);
    env.off_ui_thread(&format_args!("reading {}", at.display()));
    std::fs::read_to_string(&at).map_err(|e| format!("{path}: {e}"))
}

/// Says why the base changed each conflict's lines: the commit, and its pull request.
fn explain(env: &Env, target: &Target, merge: &mut Merge, forge: Option<&dyn Forge>) {
    let dir = &target.worktree;
    let Ok(fork) = git::run(env, dir, &["merge-base", "HEAD", &merge.base_commit]) else { return };
    let fork = fork.trim().to_owned();
    let merged: Vec<PullSummary> = forge
        .and_then(|f| f.pulls().ok())
        .map(|all| all.into_iter().filter(|p| p.merge_commit.is_some()).collect())
        .unwrap_or_default();
    let by_commit = local::commit_pulls(env, dir, &merged);
    let mut known: HashMap<u64, Option<WhyPull>> = HashMap::new();
    let mut describe = |n: u64| {
        known
            .entry(n)
            .or_insert_with(|| {
                let summary = merged.iter().find(|p| p.number == n)?;
                let body = forge.and_then(|f| f.pull(n).ok()).map(|p| p.body).unwrap_or_default();
                let reason = body.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_owned);
                let merged_at = summary.merge_commit.as_deref().and_then(|c| {
                    let t = git::run(env, dir, &["log", "-1", "--format=%ct", c]).ok()?;
                    jiff::Timestamp::from_second(t.trim().parse().ok()?).ok()
                });
                Some(WhyPull {
                    number: n,
                    title: summary.title.clone(),
                    author: summary.author.name.clone(),
                    reason,
                    merged: merged_at,
                })
            })
            .clone()
    };
    let base = merge.base_commit.clone();
    for file in &mut merge.files {
        for c in &mut file.conflicts {
            c.why = why::find(env, dir, (&fork, &base), &file.path, &c.hunk, &by_commit, &mut describe);
        }
    }
}

/// Writes each file as it now stands, where it differs from what's on disk.
pub fn write(env: &Env, target: &Target, files: &[File]) -> Result<(), String> {
    for f in files {
        let text = f.render();
        if read(env, &target.worktree, &f.path).ok().as_deref() == Some(text.as_str()) {
            continue;
        }
        let at = target.worktree.join(&f.path);
        env.off_ui_thread(&format_args!("writing {}", at.display()));
        std::fs::write(&at, text).map_err(|e| format!("{}: {e}", f.path))?;
    }
    Ok(())
}

/// The files as they are on disk now, after `by` changed them.
pub fn reread(env: &Env, target: &Target, files: &[File], by: By) -> Result<Vec<File>, String> {
    files.iter().map(|f| Ok(f.reconcile(&read(env, &target.worktree, &f.path)?, by))).collect()
}

/// Commits the merge with git's own message, then pushes the branch. Every file must be free of
/// markers, and nothing else may still conflict.
pub fn commit_and_push(env: &Env, target: &Target, files: &[File]) -> Result<String, NotPushed> {
    let dir = &target.worktree;
    let refuse = NotPushed::Commit;
    for f in files {
        let text = read(env, dir, &f.path).map_err(refuse)?;
        if super::markers::has_markers(&text) {
            return Err(refuse(format!("{} still has conflict markers.", f.path)));
        }
    }
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    if !paths.is_empty() {
        git::run(env, dir, &[&["add", "--"], paths.as_slice()].concat()).map_err(|e| refuse(said(e)))?;
    }
    let left = unmerged(env, dir).map_err(refuse)?;
    if let Some(f) = left.first() {
        return Err(refuse(format!("{f} still conflicts: resolve it in a terminal, then commit again.")));
    }
    git::run(env, dir, &["commit", "--no-edit", "--quiet"]).map_err(|e| refuse(said(e)))?;
    let commit = git::run(env, dir, &["rev-parse", "HEAD"]).map_err(|e| refuse(said(e)))?.trim().to_owned();
    push(env, target)?;
    Ok(commit)
}

/// Pushes the branch, as the toolbar's Push does.
pub fn push(env: &Env, target: &Target) -> Result<(), NotPushed> {
    ops::run(env, &target.worktree, &Op::Push).map_err(|e| NotPushed::Push(said(e)))
}

/// Stops the merge: the worktree goes back to `head`, exactly as it was before.
pub fn abort(env: &Env, target: &Target, head: &str) -> Result<(), String> {
    let dir = &target.worktree;
    if merging(env, dir).is_some() {
        git::run(env, dir, &["merge", "--abort"]).map_err(said)?;
    }
    let now = git::run(env, dir, &["rev-parse", "HEAD"]).map_err(said)?;
    if now.trim() != head {
        return Err(format!("{} is at {} after the abort, not {head}", target.worktree_name, now.trim()));
    }
    Ok(())
}
