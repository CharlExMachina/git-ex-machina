//! A repository at a glance: its branch, how far it is from upstream, and what's uncommitted.

use std::path::{Path, PathBuf};

use jiff::Timestamp;

use crate::env::Env;
use crate::git::{self, GitError};
use crate::remote::Remote;

/// What the sidebar shows for a repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoSummary {
    pub path: PathBuf,
    /// The folder's name.
    pub name: String,
    pub status: Status,
    /// `origin`, parsed. None without one or when its URL isn't one we understand.
    pub origin: Option<Remote>,
    /// When the repository last fetched (`FETCH_HEAD`'s modification time); None if never.
    pub fetched: Option<Timestamp>,
}

impl RepoSummary {
    pub fn read(env: &Env, path: &Path) -> Result<RepoSummary, GitError> {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        let status = status(env, path)?;
        // The URL as configured, before `insteadOf` rewrites, so the owner reads as people wrote it.
        let origin =
            git::run(env, path, &["config", "--get", "remote.origin.url"]).ok().and_then(|u| Remote::parse(&u));
        let fetched = git::run(env, path, &["rev-parse", "--git-path", "FETCH_HEAD"])
            .ok()
            .and_then(|p| std::fs::metadata(path.join(p.trim())).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| Timestamp::try_from(t).ok());
        Ok(RepoSummary { path: path.to_path_buf(), name, status, origin, fetched })
    }

    /// Who owns the repository on its host (`studio`), or `local` without a remote.
    pub fn owner(&self) -> &str {
        self.origin.as_ref().map_or("local", |r| r.owner.as_str())
    }
}

/// `git status`, summarised.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// None when HEAD is detached.
    pub branch: Option<String>,
    /// HEAD's commit; None before the first commit.
    pub head: Option<String>,
    /// `origin/main`; None without an upstream.
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Files with staged, unstaged or untracked changes, or conflicts.
    pub changed: u32,
    /// Of those, the files with every change staged.
    pub staged: u32,
    /// Of those, the files a merge, rebase or cherry-pick left conflicted.
    pub conflicted: u32,
}

pub fn status(env: &Env, repo: &Path) -> Result<Status, GitError> {
    let out = git::run(env, repo, &["status", "--porcelain=v2", "--branch", "--untracked-files=all", "-z"])?;
    parse_status(&out)
}

fn parse_status(out: &str) -> Result<Status, GitError> {
    let mut s = Status::default();
    let mut entries = out.split('\0').filter(|e| !e.is_empty());
    while let Some(e) = entries.next() {
        if let Some(h) = e.strip_prefix("# branch.") {
            let (key, value) = h.split_once(' ').unwrap_or((h, ""));
            match key {
                "oid" if value != "(initial)" => s.head = Some(value.to_owned()),
                "head" if value != "(detached)" => s.branch = Some(value.to_owned()),
                "upstream" => s.upstream = Some(value.to_owned()),
                "ab" => {
                    let bad = || GitError::Parse { what: "ahead and behind counts", line: e.to_owned() };
                    let (a, b) = value.split_once(' ').ok_or_else(bad)?;
                    s.ahead = a.trim_start_matches('+').parse().map_err(|_| bad())?;
                    s.behind = b.trim_start_matches('-').parse().map_err(|_| bad())?;
                }
                _ => {}
            }
            continue;
        }
        // `1 XY …` and `2 XY …`: X is the index, Y the worktree; `.` means unchanged.
        let fully_staged =
            || e.split(' ').nth(1).is_some_and(|xy| xy.len() == 2 && !xy.starts_with('.') && xy.ends_with('.'));
        match e.as_bytes().first() {
            Some(b'1') => {
                s.changed += 1;
                s.staged += u32::from(fully_staged());
            }
            Some(b'u') => {
                s.changed += 1;
                s.conflicted += 1;
            }
            Some(b'?') => s.changed += 1,
            Some(b'2') => {
                s.changed += 1;
                s.staged += u32::from(fully_staged());
                // A rename carries its original path as the next entry.
                entries.next();
            }
            _ => {}
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_counts_changes_and_the_fully_staged_ones() {
        let out = [
            "# branch.oid 5832000b646d40a027a258ec2ab417e039a1729b",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +4 -1",
            "1 M. N... 100644 100644 100644 aaaa bbbb src/staged.rs",
            "1 MM N... 100644 100644 100644 aaaa bbbb src/half.rs",
            "1 .M N... 100644 100644 100644 aaaa bbbb src/unstaged.rs",
            "2 R. N... 100644 100644 100644 aaaa bbbb R100 src/new.rs",
            "src/old.rs",
            "? notes.md",
            "u UU N... 100644 100644 100644 100644 aaaa bbbb cccc src/both.rs",
            "",
        ]
        .join("\0");
        let s = parse_status(&out).unwrap();
        assert_eq!((s.branch.as_deref(), s.upstream.as_deref()), (Some("main"), Some("origin/main")));
        assert_eq!((s.ahead, s.behind), (4, 1));
        assert_eq!((s.changed, s.staged), (6, 2), "staged.rs and the rename are fully staged");
        assert_eq!(s.conflicted, 1, "both.rs is conflicted");
    }
}
