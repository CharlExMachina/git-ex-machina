//! A repository's checkouts: the main one and every worktree (`git worktree list`), each with
//! its branch and state: what's uncommitted or conflicted, what's not pushed, how far behind it
//! is, and when it was last edited.

use std::path::{Path, PathBuf};

use jiff::Timestamp;

use super::rules::Tone;
use crate::env::Env;
use crate::git::{self, GitError};
use crate::repo;

/// One checkout of a repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    /// Its folder's name: `sidebar-resize`.
    pub name: String,
    /// The repository's own checkout, not one `git worktree add` made.
    pub main: bool,
    /// None when HEAD is detached.
    pub branch: Option<String>,
    pub head: Option<String>,
    /// HEAD's subject, when the branch has commits the base doesn't: what the work is.
    pub subject: Option<String>,
    pub upstream: Option<String>,
    /// Files with uncommitted changes, and of those, the conflicted ones.
    pub changed: u32,
    pub conflicted: u32,
    /// Commits in no remote branch: they exist only here.
    pub unpushed: u32,
    /// Commits its upstream has that it doesn't.
    pub behind: u32,
    /// Commits the base branch (`origin/main`) has that it doesn't.
    pub behind_base: u32,
    /// When it was last edited: its newest uncommitted file, else its newest commit.
    pub edited: Option<Timestamp>,
}

impl Worktree {
    /// Its colour: red when it's conflicted (here, or its pull request with the base) or behind
    /// its upstream, amber with uncommitted changes, green when clean.
    pub fn tone(&self, pull_conflicts: bool) -> Tone {
        if self.conflicted > 0 || pull_conflicts || self.behind > 0 {
            Tone::Bad
        } else if self.changed > 0 {
            Tone::Warn
        } else {
            Tone::Good
        }
    }

    /// Its state in a few words: `clean · 31 behind main`, `±3 files uncommitted`,
    /// `clean · 2 unpushed`, `2 conflicted files`.
    pub fn state(&self, base: &str) -> String {
        if self.conflicted > 0 {
            return plural(self.conflicted, "conflicted file", "conflicted files");
        }
        if self.changed > 0 {
            return format!("±{} uncommitted", plural(self.changed, "file", "files"));
        }
        let sync = if self.unpushed > 0 {
            format!("{} unpushed", self.unpushed)
        } else if self.behind > 0 {
            format!("{} to pull", self.behind)
        } else if self.behind_base > 0 {
            format!("{} behind {base}", self.behind_base)
        } else {
            "up to date".to_owned()
        };
        format!("clean · {sync}")
    }
}

fn plural(n: u32, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

/// One entry of `git worktree list --porcelain`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Listed {
    path: PathBuf,
    branch: Option<String>,
}

/// The checkouts git lists, the main one first; bare ones, and ones whose folder is gone
/// (prunable), left out.
fn parse_list(out: &str) -> Vec<Listed> {
    let mut list = Vec::new();
    for block in out.split("\n\n") {
        let mut path = None;
        let mut branch = None;
        let mut skip = false;
        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(p));
            } else if let Some(b) = line.strip_prefix("branch ") {
                branch = Some(b.strip_prefix("refs/heads/").unwrap_or(b).to_owned());
            } else if line == "bare" || line.starts_with("prunable") {
                skip = true;
            }
        }
        if let (Some(path), false) = (path, skip) {
            list.push(Listed { path, branch });
        }
    }
    list
}

/// The repository's checkouts, the main one first, read against `base` (`origin/main`).
pub fn list(env: &Env, repo: &Path, base: &str) -> Result<Vec<Worktree>, GitError> {
    let out = git::run(env, repo, &["worktree", "list", "--porcelain"])?;
    let has_base = git::run(env, repo, &["rev-parse", "--verify", "--quiet", base]).is_ok();
    parse_list(&out)
        .into_iter()
        .enumerate()
        .map(|(i, l)| read(env, &l.path, i == 0, l.branch, has_base.then_some(base)))
        .collect()
}

fn read(env: &Env, path: &Path, main: bool, branch: Option<String>, base: Option<&str>) -> Result<Worktree, GitError> {
    let st = repo::status(env, path)?;
    let count =
        |args: &[&str]| -> u32 { git::run(env, path, args).ok().and_then(|n| n.trim().parse().ok()).unwrap_or(0) };
    let has_head = st.head.is_some();
    let unpushed = if has_head { count(&["rev-list", "--count", "HEAD", "--not", "--remotes"]) } else { 0 };
    let (behind_base, own) = match base {
        Some(b) if has_head => (
            count(&["rev-list", "--count", &format!("HEAD..{b}")]),
            count(&["rev-list", "--count", &format!("{b}..HEAD")]),
        ),
        _ => (0, u32::from(has_head)),
    };
    let subject = if own > 0 {
        git::run(env, path, &["log", "-1", "--format=%s"]).ok().map(|s| s.trim().to_owned())
    } else {
        None
    };
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    Ok(Worktree {
        path: path.to_path_buf(),
        name,
        main,
        branch: branch.or(st.branch),
        head: st.head,
        subject,
        upstream: st.upstream,
        changed: st.changed,
        conflicted: st.conflicted,
        unpushed,
        behind: st.behind,
        behind_base,
        edited: edited(env, path, st.changed > 0, has_head),
    })
}

/// When the checkout was last edited: the newest modification among its uncommitted files, or
/// its newest commit's date when everything is committed.
fn edited(env: &Env, path: &Path, dirty: bool, has_head: bool) -> Option<Timestamp> {
    if dirty && let Ok(out) = git::run(env, path, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]) {
        // Entries are `XY path`; a rename's original path follows as an entry of its own.
        let newest = out
            .split('\0')
            .filter_map(|e| e.get(3..))
            .filter_map(|f| std::fs::metadata(path.join(f)).ok()?.modified().ok())
            .filter_map(|t| Timestamp::try_from(t).ok())
            .max();
        if newest.is_some() {
            return newest;
        }
    }
    if !has_head {
        return None;
    }
    git::run(env, path, &["log", "-1", "--format=%ct"])
        .ok()
        .and_then(|t| t.trim().parse::<i64>().ok())
        .and_then(|t| Timestamp::from_second(t).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_keeps_checkouts_with_folders_and_their_branches() {
        let out = "worktree /code/atlas-web\nHEAD aaaa\nbranch refs/heads/main\n\n\
                   worktree /code/atlas-web.worktrees/cubic\nHEAD bbbb\nbranch refs/heads/graph/cubic\n\n\
                   worktree /code/atlas-web.worktrees/look\nHEAD cccc\ndetached\n\n\
                   worktree /code/atlas-web.worktrees/gone\nHEAD dddd\nbranch refs/heads/gone\nprunable gitdir file points to non-existent location\n";
        let list = parse_list(out);
        let names: Vec<(&str, Option<&str>)> =
            list.iter().map(|l| (l.path.to_str().unwrap_or(""), l.branch.as_deref())).collect();
        assert_eq!(
            names,
            [
                ("/code/atlas-web", Some("main")),
                ("/code/atlas-web.worktrees/cubic", Some("graph/cubic")),
                ("/code/atlas-web.worktrees/look", None),
            ]
        );
    }

    fn clean() -> Worktree {
        Worktree {
            path: PathBuf::from("/w"),
            name: "w".to_owned(),
            main: false,
            branch: Some("b".to_owned()),
            head: Some("h".to_owned()),
            subject: None,
            upstream: Some("origin/b".to_owned()),
            changed: 0,
            conflicted: 0,
            unpushed: 0,
            behind: 0,
            behind_base: 0,
            edited: None,
        }
    }

    #[test]
    fn a_worktree_is_red_when_conflicted_or_behind_amber_when_dirty_and_green_when_clean() {
        let w = clean();
        assert_eq!((w.tone(false), w.state("main").as_str()), (Tone::Good, "clean · up to date"));
        assert_eq!(w.tone(true), Tone::Bad, "its pull request conflicts with the base");
        let w = Worktree { behind_base: 31, ..clean() };
        assert_eq!((w.tone(false), w.state("main").as_str()), (Tone::Good, "clean · 31 behind main"));
        let w = Worktree { behind: 2, ..clean() };
        assert_eq!((w.tone(false), w.state("main").as_str()), (Tone::Bad, "clean · 2 to pull"));
        let w = Worktree { unpushed: 2, behind_base: 9, ..clean() };
        assert_eq!((w.tone(false), w.state("main").as_str()), (Tone::Good, "clean · 2 unpushed"));
        let w = Worktree { changed: 3, ..clean() };
        assert_eq!((w.tone(false), w.state("main").as_str()), (Tone::Warn, "±3 files uncommitted"));
        let w = Worktree { changed: 3, conflicted: 2, ..clean() };
        assert_eq!((w.tone(false), w.state("main").as_str()), (Tone::Bad, "2 conflicted files"));
    }
}
