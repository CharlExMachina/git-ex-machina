//! A repository's commits, newest first, with the refs that point at them.

use std::path::Path;

use jiff::Timestamp;

use crate::env::Env;
use crate::git::{self, GitError};

/// A commit as the history list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub time: Timestamp,
    pub subject: String,
    pub refs: Vec<RefLabel>,
}

impl Commit {
    /// The seven-character abbreviation the list shows.
    pub fn short_id(&self) -> &str {
        self.id.get(..7).unwrap_or(&self.id)
    }

    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

/// A ref decorating a commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefLabel {
    pub kind: RefKind,
    /// `main`, `origin/main`, `v0.4.0`.
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefKind {
    /// The checked-out branch (`HEAD -> main`).
    Head,
    LocalBranch,
    RemoteBranch,
    Tag,
}

const FIELD: char = '\u{1f}';
const RECORD: char = '\u{1e}';

/// Up to `limit` commits reachable from a branch, a remote branch, a tag or HEAD, newest first (date order, children before
/// parents).
pub fn log(env: &Env, repo: &Path, limit: usize) -> Result<Vec<Commit>, GitError> {
    let max = format!("--max-count={limit}");
    let out = git::run(
        env,
        repo,
        &[
            "log",
            // What a person thinks of as history: branches, remotes, tags and HEAD. Not
            // `--all`, which would draw a stash's internal commits as rows.
            "--branches",
            "--remotes",
            "--tags",
            "HEAD",
            "--date-order",
            &max,
            "--decorate=full",
            "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%D%x1e",
        ],
    );
    let out = match out {
        Ok(out) => out,
        // A repository with no commits yet has no history, which isn't an error.
        Err(GitError::Failed { stderr, .. })
            if stderr.contains("does not have any commits") || stderr.contains("unknown revision") =>
        {
            return Ok(Vec::new());
        }
        Err(e) => return Err(e),
    };
    out.split(RECORD).map(str::trim_start).filter(|r| !r.is_empty()).map(parse_record).collect()
}

fn parse_record(record: &str) -> Result<Commit, GitError> {
    let bad = || GitError::Parse { what: "a commit", line: record.to_owned() };
    let mut f = record.split(FIELD);
    let mut next = || f.next().ok_or_else(bad);
    let id = next()?.to_owned();
    let parents = next()?.split_whitespace().map(str::to_owned).collect();
    let author = next()?.to_owned();
    let email = next()?.to_owned();
    let secs: i64 = next()?.parse().map_err(|_| bad())?;
    let time = Timestamp::from_second(secs).map_err(|_| bad())?;
    let subject = next()?.to_owned();
    let refs = parse_decorations(next()?.trim_end());
    Ok(Commit { id, parents, author, email, time, subject, refs })
}

/// Reads `%D` with `--decorate=full`: `HEAD -> refs/heads/main, refs/remotes/origin/main,
/// tag: refs/tags/v1`. Remote `HEAD` symrefs are left out; they repeat a branch.
pub fn parse_decorations(d: &str) -> Vec<RefLabel> {
    let mut labels = Vec::new();
    for part in d.split(", ").filter(|p| !p.is_empty()) {
        let (head, full) = match part.strip_prefix("HEAD -> ") {
            Some(rest) => (true, rest),
            None => (false, part),
        };
        let full = full.strip_prefix("tag: ").unwrap_or(full);
        let label = if let Some(name) = full.strip_prefix("refs/heads/") {
            RefLabel { kind: if head { RefKind::Head } else { RefKind::LocalBranch }, name: name.to_owned() }
        } else if let Some(name) = full.strip_prefix("refs/remotes/") {
            if name.ends_with("/HEAD") {
                continue;
            }
            RefLabel { kind: RefKind::RemoteBranch, name: name.to_owned() }
        } else if let Some(name) = full.strip_prefix("refs/tags/") {
            RefLabel { kind: RefKind::Tag, name: name.to_owned() }
        } else {
            // A detached `HEAD` alone, or refs outside the usual namespaces.
            continue;
        };
        labels.push(label);
    }
    labels
}

/// One file a commit changed, against its first parent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub status: ChangeStatus,
    /// None for binary files.
    pub added: Option<u32>,
    pub removed: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
}

impl ChangeStatus {
    /// The single letter `git status` uses.
    pub fn letter(self) -> char {
        match self {
            ChangeStatus::Added => 'A',
            ChangeStatus::Modified => 'M',
            ChangeStatus::Deleted => 'D',
            ChangeStatus::Renamed => 'R',
            ChangeStatus::Copied => 'C',
            ChangeStatus::TypeChanged => 'T',
        }
    }
}

/// What a commit changed against its first parent (everything, for a root commit).
pub fn changes(env: &Env, repo: &Path, commit: &str) -> Result<Vec<FileChange>, GitError> {
    // `--root` makes a root commit diff against the empty tree; `-m --first-parent` makes a
    // merge diff against the branch it was merged into, as the list shows it.
    let base = ["show", "--format=", "--root", "-m", "--first-parent", "-M", "-z"];
    let raw = git::run(env, repo, &[&base[..], &["--raw", commit]].concat())?;
    let num = git::run(env, repo, &[&base[..], &["--numstat", commit]].concat())?;
    let mut files = parse_raw(&raw)?;
    for (path, added, removed) in parse_numstat(&num)? {
        if let Some(f) = files.iter_mut().find(|f| f.path == path) {
            f.added = added;
            f.removed = removed;
        }
    }
    Ok(files)
}

/// `--raw -z`: `:old new oldsha newsha S\0path\0` (renames and copies add a second path).
fn parse_raw(out: &str) -> Result<Vec<FileChange>, GitError> {
    let mut files = Vec::new();
    let mut parts = out.split('\0').filter(|p| !p.is_empty());
    while let Some(meta) = parts.next() {
        let bad = || GitError::Parse { what: "a changed file", line: meta.to_owned() };
        let code = meta.split_whitespace().nth(4).and_then(|s| s.chars().next()).ok_or_else(bad)?;
        let status = match code {
            'A' => ChangeStatus::Added,
            'D' => ChangeStatus::Deleted,
            'R' => ChangeStatus::Renamed,
            'C' => ChangeStatus::Copied,
            'T' => ChangeStatus::TypeChanged,
            _ => ChangeStatus::Modified,
        };
        let mut path = parts.next().ok_or_else(bad)?;
        if matches!(status, ChangeStatus::Renamed | ChangeStatus::Copied) {
            path = parts.next().ok_or_else(bad)?;
        }
        files.push(FileChange { path: path.to_owned(), status, added: None, removed: None });
    }
    Ok(files)
}

/// A path with its added and removed line counts (None for binary files).
type LineCounts = (String, Option<u32>, Option<u32>);

/// `--numstat -z`: `added\tremoved\tpath\0`, or `added\tremoved\t\0old\0new\0` for renames.
fn parse_numstat(out: &str) -> Result<Vec<LineCounts>, GitError> {
    let mut rows = Vec::new();
    let mut parts = out.split('\0').filter(|p| !p.is_empty());
    while let Some(line) = parts.next() {
        let bad = || GitError::Parse { what: "a line count", line: line.to_owned() };
        let mut cols = line.splitn(3, '\t');
        let added = cols.next().ok_or_else(bad)?.parse().ok();
        let removed = cols.next().ok_or_else(bad)?.parse().ok();
        let mut path = cols.next().ok_or_else(bad)?.to_owned();
        if path.is_empty() {
            let _old = parts.next().ok_or_else(bad)?;
            path = parts.next().ok_or_else(bad)?.to_owned();
        }
        rows.push((path, added, removed));
    }
    Ok(rows)
}

/// What the inspector shows about a commit beyond its row: the message's body without its
/// trailers, the trailers themselves, who committed it and when, and the refs that contain it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitInfo {
    pub body: String,
    /// `Co-authored-by: …`, `Refs: …`, in the order they're written.
    pub trailers: Vec<(String, String)>,
    pub committer: String,
    pub committer_email: String,
    pub committed: Timestamp,
    /// Branches, remote branches and tags whose history includes the commit.
    pub contained_in: Vec<RefLabel>,
}

pub fn info(env: &Env, repo: &Path, commit: &str) -> Result<CommitInfo, GitError> {
    let out = git::run(env, repo, &["show", "-s", "--format=%cn%x1f%ce%x1f%ct%x1f%b", commit])?;
    let bad = || GitError::Parse { what: "a commit's details", line: out.clone() };
    let mut fields = out.splitn(4, FIELD);
    let committer = fields.next().ok_or_else(bad)?.to_owned();
    let committer_email = fields.next().ok_or_else(bad)?.to_owned();
    let secs: i64 = fields.next().ok_or_else(bad)?.trim().parse().map_err(|_| bad())?;
    let committed = Timestamp::from_second(secs).map_err(|_| bad())?;
    let (body, trailers) = split_trailers(fields.next().unwrap_or_default());
    let refs = git::run(
        env,
        repo,
        &["for-each-ref", "--contains", commit, "--format=%(refname)", "refs/heads", "refs/remotes", "refs/tags"],
    )?;
    let contained_in = refs
        .lines()
        .filter_map(|r| {
            let (kind, name) = if let Some(n) = r.strip_prefix("refs/heads/") {
                (RefKind::LocalBranch, n)
            } else if let Some(n) = r.strip_prefix("refs/tags/") {
                (RefKind::Tag, n)
            } else {
                (RefKind::RemoteBranch, r.strip_prefix("refs/remotes/")?)
            };
            // `origin/HEAD` only points at another branch.
            (!name.ends_with("/HEAD")).then(|| RefLabel { kind, name: name.to_owned() })
        })
        .collect();
    Ok(CommitInfo { body, trailers, committer, committer_email, committed, contained_in })
}

/// A message body's trailers are its last paragraph when every line in it is `Key: value`
/// (git's own rule, simplified). Returns the body without them, and the trailers.
pub fn split_trailers(body: &str) -> (String, Vec<(String, String)>) {
    let body = body.trim();
    let (rest, last) = body.rsplit_once("\n\n").unwrap_or(("", body));
    let parse = |line: &str| {
        let (key, value) = line.split_once(": ")?;
        let token = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        token.then(|| (key.to_owned(), value.trim().to_owned()))
    };
    let trailers: Option<Vec<_>> = last.lines().map(parse).collect();
    match trailers {
        Some(t) if !t.is_empty() => (rest.trim().to_owned(), t),
        _ => (body.to_owned(), Vec::new()),
    }
}

/// How many commits are reachable from a branch, a remote branch, a tag or HEAD.
pub fn count(env: &Env, repo: &Path) -> Result<usize, GitError> {
    let out = match git::run(env, repo, &["rev-list", "--branches", "--remotes", "--tags", "HEAD", "--count"]) {
        Ok(out) => out,
        Err(GitError::Failed { .. }) => return Ok(0),
        Err(e) => return Err(e),
    };
    out.trim().parse().map_err(|_| GitError::Parse { what: "a commit count", line: out.clone() })
}

/// Uncommitted changes against HEAD: staged, unstaged and untracked files.
pub fn uncommitted(env: &Env, repo: &Path) -> Result<Vec<FileChange>, GitError> {
    let out = git::run(env, repo, &["status", "--porcelain=v2", "--untracked-files=all", "-z"])?;
    let mut files = Vec::new();
    let mut entries = out.split('\0').filter(|e| !e.is_empty());
    while let Some(e) = entries.next() {
        let bad = || GitError::Parse { what: "a status entry", line: e.to_owned() };
        let (status, path) = match e.as_bytes().first() {
            // `1 XY sub mH mI mW hH hI path`: X is the index, Y the worktree.
            Some(b'1') => {
                let xy = e.split(' ').nth(1).ok_or_else(bad)?;
                let code = xy.chars().find(|c| *c != '.').unwrap_or('M');
                (
                    if code == 'A' {
                        ChangeStatus::Added
                    } else if code == 'D' {
                        ChangeStatus::Deleted
                    } else {
                        ChangeStatus::Modified
                    },
                    e.splitn(9, ' ').nth(8).ok_or_else(bad)?,
                )
            }
            Some(b'2') => {
                entries.next();
                (ChangeStatus::Renamed, e.splitn(10, ' ').nth(9).ok_or_else(bad)?)
            }
            Some(b'u') => (ChangeStatus::Modified, e.splitn(11, ' ').nth(10).ok_or_else(bad)?),
            Some(b'?') => (ChangeStatus::Added, e.get(2..).ok_or_else(bad)?),
            _ => continue,
        };
        files.push(FileChange { path: path.to_owned(), status, added: None, removed: None });
    }
    // Line counts for tracked files; untracked ones count every line as added.
    if let Ok(num) = git::run(env, repo, &["diff", "HEAD", "--numstat", "-M", "-z"]) {
        for (path, added, removed) in parse_numstat(&num)? {
            if let Some(f) = files.iter_mut().find(|f| f.path == path) {
                f.added = added;
                f.removed = removed;
            }
        }
    }
    for f in files.iter_mut().filter(|f| f.added.is_none() && f.status == ChangeStatus::Added) {
        if let Ok(text) = std::fs::read_to_string(repo.join(&f.path)) {
            f.added = Some(u32::try_from(text.lines().count()).unwrap_or(u32::MAX));
            f.removed = Some(0);
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailers_are_the_last_paragraph_of_key_value_lines() {
        let body = "Collapses on the spring.\n\nCo-authored-by: Lea Brandt <lea@example.com>\nRefs: #121\n";
        let (rest, trailers) = split_trailers(body);
        assert_eq!(rest, "Collapses on the spring.");
        assert_eq!(
            trailers,
            [
                ("Co-authored-by".to_owned(), "Lea Brandt <lea@example.com>".to_owned()),
                ("Refs".to_owned(), "#121".to_owned())
            ]
        );
        // A trailer-only body, and a last paragraph that's prose, not trailers.
        assert_eq!(split_trailers("Refs: #1").1.len(), 1);
        let prose = "First.\n\nNote: this line reads like a trailer.\nThis one doesn't, so neither is.";
        assert_eq!(split_trailers(prose), (prose.to_owned(), Vec::new()));
        assert_eq!(split_trailers(""), (String::new(), Vec::new()));
    }
}
