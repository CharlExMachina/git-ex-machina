//! What a pull request conflicts in, and which merge into its base caused it, worked out with
//! git: the merge is tried in memory (`git merge-tree`), and the culprit is the newest pull
//! request merged into the base since the branch forked that changed a conflicting file.

use std::path::Path;

use crate::env::Env;
use crate::forge::model::PullSummary;
use crate::git;

/// A pull request's conflicts with its base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// The base branch's name: `main`.
    pub base: String,
    /// The files that conflict, as far as git here can tell (empty when it can't).
    pub files: Vec<String>,
    /// The pull request whose merge into the base brought the conflicting change, when one did.
    pub since: Option<u64>,
}

impl Conflict {
    /// `Conflicts with main in src/sidebar.rs, since #121 landed`.
    pub fn sentence(&self) -> String {
        let mut s = format!("Conflicts with {}", self.base);
        match self.files.as_slice() {
            [] => {}
            [one] => s.push_str(&format!(" in {one}")),
            [a, b] => s.push_str(&format!(" in {a} and {b}")),
            [a, rest @ ..] => s.push_str(&format!(" in {a} and {} more files", rest.len())),
        }
        if let Some(n) = self.since {
            s.push_str(&format!(", since #{n} landed"));
        }
        s
    }
}

/// The conflicts merging `head` into `base` (a ref here: `origin/main`) would hit, named after
/// `base_name`, or None when it merges cleanly. `merged` are pull requests already merged, any
/// of which may be the cause.
pub fn find(
    env: &Env,
    repo: &Path,
    base: &str,
    base_name: &str,
    head: &str,
    merged: &[PullSummary],
) -> Option<Conflict> {
    let files = match conflicting_files(env, repo, base, head) {
        Some(files) if files.is_empty() => return None,
        Some(files) => files,
        // The head commit isn't here (not fetched): the provider's word stands, without detail.
        None => return Some(Conflict { base: base_name.to_owned(), files: Vec::new(), since: None }),
    };
    let since = culprit(env, repo, base, head, &files, merged);
    Some(Conflict { base: base_name.to_owned(), files, since })
}

/// The files merging `head` into `base` conflicts in: empty when it merges cleanly, None when
/// git can't try (a commit is missing).
fn conflicting_files(env: &Env, repo: &Path, base: &str, head: &str) -> Option<Vec<String>> {
    // merge-tree says 1 for a commit it doesn't have, as for a conflict: ask first.
    for rev in [base, head] {
        git::run(env, repo, &["cat-file", "-e", &format!("{rev}^{{commit}}")]).ok()?;
    }
    let out = git::command(env, repo, &["merge-tree", "--write-tree", "--name-only", "--no-messages", base, head])
        .output()
        .ok()?;
    match out.status.code() {
        Some(0) => Some(Vec::new()),
        // The tree it wrote, then each conflicting file once per stage.
        Some(1) => {
            let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .skip(1)
                .filter(|l| !l.is_empty())
                .map(str::to_owned)
                .collect();
            files.dedup();
            Some(files)
        }
        _ => None,
    }
}

/// The newest merged pull request whose merge is in `base` but not in `head`, and that changed
/// one of `files`.
fn culprit(env: &Env, repo: &Path, base: &str, head: &str, files: &[String], merged: &[PullSummary]) -> Option<u64> {
    let mut found: Option<(i64, u64)> = None;
    for p in merged {
        let Some(commit) = p.merge_commit.as_deref() else { continue };
        let in_base = git::run(env, repo, &["merge-base", "--is-ancestor", commit, base]).is_ok();
        if !in_base || git::run(env, repo, &["merge-base", "--is-ancestor", commit, head]).is_ok() {
            continue;
        }
        // What the merge brought into the base: its change against its first parent.
        let Ok(changed) = git::run(env, repo, &["diff", "--name-only", &format!("{commit}^"), commit]) else {
            continue;
        };
        if !changed.lines().any(|f| files.iter().any(|c| c == f)) {
            continue;
        }
        let when = git::run(env, repo, &["log", "-1", "--format=%ct", commit])
            .ok()
            .and_then(|t| t.trim().parse::<i64>().ok())
            .unwrap_or(0);
        if found.is_none_or(|(t, _)| when > t) {
            found = Some((when, p.number));
        }
    }
    found.map(|(_, n)| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conflict_names_its_files_and_the_merge_that_caused_it() {
        let c = |files: &[&str], since| Conflict {
            base: "main".to_owned(),
            files: files.iter().map(|f| (*f).to_owned()).collect(),
            since,
        };
        assert_eq!(c(&[], None).sentence(), "Conflicts with main");
        assert_eq!(
            c(&["src/sidebar.rs"], Some(121)).sentence(),
            "Conflicts with main in src/sidebar.rs, since #121 landed"
        );
        assert_eq!(c(&["a.rs", "b.rs"], None).sentence(), "Conflicts with main in a.rs and b.rs");
        assert_eq!(c(&["a.rs", "b.rs", "c.rs"], None).sentence(), "Conflicts with main in a.rs and 2 more files");
    }
}
