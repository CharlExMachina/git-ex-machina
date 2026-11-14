//! A file's change as the inspector previews it: its first hunk, with one line of context.

use std::path::Path;

use crate::env::Env;
use crate::git::{self, GitError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// The `@@ -41,3 +41,4 @@ fn layout_row(row: &Row)` line.
    pub header: String,
    pub lines: Vec<HunkLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HunkLine {
    pub kind: LineKind,
    /// The line's number before the change (None for an added line).
    pub old: Option<u32>,
    /// The line's number after the change (None for a removed line).
    pub new: Option<u32>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

/// Lines of context around each change, as the preview shows them.
const CONTEXT: &str = "-U1";

/// The first hunk of what `commit` changed in `path`, against its first parent.
pub fn commit_hunk(env: &Env, repo: &Path, commit: &str, path: &str) -> Result<Option<Hunk>, GitError> {
    let out = git::run(
        env,
        repo,
        &["show", "--format=", "--root", "-m", "--first-parent", "-M", CONTEXT, commit, "--", path],
    )?;
    Ok(first_hunk(&out))
}

/// The first hunk of `path`'s uncommitted change against HEAD. An untracked file is all one
/// added hunk.
pub fn worktree_hunk(env: &Env, repo: &Path, path: &str) -> Result<Option<Hunk>, GitError> {
    let tracked = git::run(env, repo, &["ls-files", "--", path])?;
    if tracked.trim().is_empty() {
        let Ok(text) = std::fs::read_to_string(repo.join(path)) else { return Ok(None) };
        let lines: Vec<HunkLine> = text
            .lines()
            .zip(1..)
            .map(|(l, n)| HunkLine { kind: LineKind::Added, old: None, new: Some(n), text: l.to_owned() })
            .collect();
        return Ok(Some(Hunk { header: format!("@@ -0,0 +1,{} @@ {path}", lines.len()), lines }));
    }
    let out = git::run(env, repo, &["diff", "HEAD", "-M", CONTEXT, "--", path])?;
    Ok(first_hunk(&out))
}

/// The first `@@` hunk of a unified diff.
pub fn first_hunk(diff: &str) -> Option<Hunk> {
    let mut lines = diff.lines().skip_while(|l| !l.starts_with("@@ "));
    let header = lines.next()?.to_owned();
    let (mut old, mut new) = starts(&header)?;
    let mut out = Vec::new();
    for l in lines {
        if l.starts_with("@@ ") || l.starts_with("diff --git ") {
            break;
        }
        let (kind, text) = match l.as_bytes().first() {
            Some(b'+') => (LineKind::Added, &l[1..]),
            Some(b'-') => (LineKind::Removed, &l[1..]),
            Some(b' ') => (LineKind::Context, &l[1..]),
            Some(b'\\') => continue,
            _ => (LineKind::Context, l),
        };
        let (o, n) = match kind {
            LineKind::Added => (None, Some(new)),
            LineKind::Removed => (Some(old), None),
            LineKind::Context => (Some(old), Some(new)),
        };
        if o.is_some() {
            old += 1;
        }
        if n.is_some() {
            new += 1;
        }
        out.push(HunkLine { kind, old: o, new: n, text: text.to_owned() });
    }
    Some(Hunk { header, lines: out })
}

/// The first old and new line numbers in `@@ -41,6 +41,8 @@`.
fn starts(header: &str) -> Option<(u32, u32)> {
    let mut parts = header.split(' ').skip(1);
    let num = |s: &str| s.split(',').next().and_then(|n| n.parse::<u32>().ok());
    let old = num(parts.next()?.strip_prefix('-')?)?;
    let new = num(parts.next()?.strip_prefix('+')?)?;
    Some((old, new))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_hunk_numbers_its_lines_on_both_sides() {
        let diff = "diff --git a/r.rs b/r.rs\n--- a/r.rs\n+++ b/r.rs\n@@ -41,3 +41,4 @@ fn layout_row(row: &Row)\n for lane in lanes.active() {\n-    draw_line(lane.x);\n+    let x = PAD;\n+    draw_rail(x);\n }\n@@ -90,1 +91,1 @@\n-a\n+b\n";
        let h = first_hunk(diff).unwrap();
        assert_eq!(h.header, "@@ -41,3 +41,4 @@ fn layout_row(row: &Row)");
        let rows: Vec<_> = h.lines.iter().map(|l| (l.kind, l.old, l.new, l.text.as_str())).collect();
        assert_eq!(
            rows,
            [
                (LineKind::Context, Some(41), Some(41), "for lane in lanes.active() {"),
                (LineKind::Removed, Some(42), None, "    draw_line(lane.x);"),
                (LineKind::Added, None, Some(42), "    let x = PAD;"),
                (LineKind::Added, None, Some(43), "    draw_rail(x);"),
                (LineKind::Context, Some(43), Some(44), "}"),
            ]
        );
        assert_eq!(first_hunk("no hunks here"), None);
    }
}
