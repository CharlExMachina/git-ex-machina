//! Why the base changed a conflict's lines: its side of the conflict is found in the base's
//! version of the file, `git blame` over just the base's commits since the fork names who last
//! changed those lines, and the forge's index (which commits came in with which merged pull
//! request) names the pull request.

use std::collections::HashMap;
use std::path::Path;

use jiff::Timestamp;

use super::{Hunk, Why};
use crate::env::Env;
use crate::git;

/// A commit, as blame or log describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Commit {
    id: String,
    subject: String,
    author: String,
    at: i64,
}

/// Where `block`'s lines stand in `file`'s lines, 1-based and inclusive, when they stand
/// together. A file can repeat a block; the first is the guess.
pub fn locate(file: &str, block: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = file.lines().collect();
    let want: Vec<&str> = block.lines().collect();
    if want.is_empty() || want.len() > lines.len() {
        return None;
    }
    let at = (0..=lines.len() - want.len()).find(|&i| lines[i..i + want.len()] == want[..])?;
    Some((at + 1, at + want.len()))
}

/// The commits `git blame --porcelain` names, without the boundary (lines older than the range).
fn blamed(porcelain: &str) -> Vec<Commit> {
    let mut found: Vec<Commit> = Vec::new();
    let mut boundary: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in porcelain.lines() {
        let first = line.split(' ').next().unwrap_or_default();
        if first.len() == 40 && first.chars().all(|c| c.is_ascii_hexdigit()) {
            current = Some(first.to_owned());
            if !found.iter().any(|c| c.id == first) {
                found.push(Commit { id: first.to_owned(), subject: String::new(), author: String::new(), at: 0 });
            }
            continue;
        }
        let Some(c) = current.as_ref().and_then(|id| found.iter_mut().find(|c| c.id == *id)) else { continue };
        if let Some(v) = line.strip_prefix("summary ") {
            c.subject = v.to_owned();
        } else if let Some(v) = line.strip_prefix("author ") {
            c.author = v.to_owned();
        } else if let Some(v) = line.strip_prefix("committer-time ") {
            c.at = v.trim().parse().unwrap_or(0);
        } else if line == "boundary" {
            boundary.push(c.id.clone());
        }
    }
    found.retain(|c| !boundary.contains(&c.id));
    found
}

/// The newest commit on `base` since `fork` (the pair) that changed the base's side of `hunk` in `path`:
/// the lines it holds, or, for lines the base deleted, the file. Then its pull request, from
/// `by_commit`; `pull` describes one: its title, author and first line of description.
pub fn find(
    env: &Env,
    dir: &Path,
    (fork, base): (&str, &str),
    path: &str,
    hunk: &Hunk,
    by_commit: &HashMap<String, u64>,
    pull: &mut dyn FnMut(u64) -> Option<super::WhyPull>,
) -> Option<Why> {
    let range = format!("{fork}..{base}");
    let text = git::run(env, dir, &["show", &format!("{base}:{path}")]).ok()?;
    let commits = match locate(&text, &hunk.theirs) {
        Some((a, b)) => {
            let lines = format!("{a},{b}");
            let out = git::run(env, dir, &["blame", "--porcelain", "-L", &lines, &range, "--", path]).ok()?;
            blamed(&out)
        }
        None => {
            let out = git::run(env, dir, &["log", "-1", "--format=%H%x1f%s%x1f%an%x1f%ct", &range, "--", path]).ok()?;
            let mut f = out.trim().split('\x1f');
            let (id, subject, author, at) = (f.next()?, f.next()?, f.next()?, f.next()?);
            vec![Commit {
                id: id.to_owned(),
                subject: subject.to_owned(),
                author: author.to_owned(),
                at: at.parse().unwrap_or(0),
            }]
        }
    };
    let newest = commits.into_iter().max_by_key(|c| c.at)?;
    let pull = by_commit.get(&newest.id).and_then(|n| pull(*n));
    Some(Why {
        commit: newest.id,
        subject: newest.subject,
        author: newest.author,
        at: Timestamp::from_second(newest.at).unwrap_or_default(),
        pull,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_is_found_where_its_lines_stand_together() {
        let file = "a\nb\nc\nb\nc\nd\n";
        assert_eq!(locate(file, "b\nc\n"), Some((2, 3)));
        assert_eq!(locate(file, "c\nd"), Some((5, 6)));
        assert_eq!(locate(file, "b\nd\n"), None);
        assert_eq!(locate(file, ""), None, "a deletion leaves nothing to find");
    }

    #[test]
    fn blame_names_each_commit_once_and_drops_the_boundary() {
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let porcelain = format!(
            "{a} 50 50 2\nauthor Lea Brandt\ncommitter-time 1762675920\nsummary Fix avatar cache eviction\nfilename src/c.rs\n\tline\n\
             {a} 51 51\n\tline\n\
             {b} 52 52 1\nauthor Ana Ruiz\ncommitter-time 1760000000\nsummary Old\nboundary\nfilename src/c.rs\n\tline\n"
        );
        let found = blamed(&porcelain);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].subject, "Fix avatar cache eviction");
        assert_eq!((found[0].author.as_str(), found[0].at), ("Lea Brandt", 1_762_675_920));
    }
}
