//! Resolving a pull request's conflicts with its base: its base, freshly fetched, is merged into
//! its branch inside its worktree, so every conflict shows at once; each is resolved (by taking
//! a side, by hand, or by the person's coding agent); then the merge is committed and pushed, or
//! aborted, which puts the worktree back exactly as it was.
//!
//! - [`markers`] reads git's conflict markers, [`scope`] names what a conflict sits in, and
//!   [`File`] keeps a file's conflicts and what each was resolved with.
//! - [`merge`] runs git: [`begin`](merge::begin), [`write`](merge::write),
//!   [`commit_and_push`](merge::commit_and_push), [`abort`](merge::abort).
//! - [`why`] finds the commits on the base that changed each conflict's lines, and their pull
//!   requests; [`lockfile`] handles generated lockfiles.
//! - [`flow`] is the whole of it as a machine, and [`prompt`] what an agent is asked.
//!
//! Everything that runs git or reads files blocks, so the app runs it off the UI thread.

pub mod file;
pub mod flow;
pub mod lockfile;
pub mod markers;
pub mod merge;
pub mod prompt;
pub mod scope;
pub mod why;

use std::path::PathBuf;

use jiff::Timestamp;

pub use file::File;
pub use flow::{Agent, Effect, Event, Flow, Phase};
pub use lockfile::Lockfile;
pub use markers::Hunk;

/// The pull request whose conflicts are resolved, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// The repository's main checkout.
    pub repo: PathBuf,
    /// The pull request's worktree, where the merge happens.
    pub worktree: PathBuf,
    /// The worktree's folder name: `avatar-loading`.
    pub worktree_name: String,
    /// The pull request's branch: `perf/avatar-loading`.
    pub branch: String,
    /// The branch it merges into: `main`.
    pub base: String,
    /// The remote both live on: `origin`.
    pub remote: String,
    pub number: u64,
    pub title: String,
}

impl Target {
    /// `origin/main`.
    pub fn base_ref(&self) -> String {
        format!("{}/{}", self.remote, self.base)
    }
}

/// One conflict in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub hunk: Hunk,
    /// What it sits in: `fn refresh`, when a declaration encloses it.
    pub scope: Option<String>,
    /// Why the base changed these lines, when its history says.
    pub why: Option<Why>,
    pub resolution: Option<Resolution>,
}

impl Conflict {
    /// What stands in the file for it: its resolution, or its markers.
    pub fn text(&self) -> &str {
        self.resolution.as_ref().map_or(self.hunk.raw.as_str(), |r| r.text.as_str())
    }

    /// The text taking `side` gives.
    pub fn side(&self, side: Side) -> String {
        match side {
            Side::Yours => self.hunk.ours.clone(),
            Side::Base => self.hunk.theirs.clone(),
            Side::Both => ended(&self.hunk.ours) + &self.hunk.theirs,
        }
    }
}

/// `text` ending in a newline, unless it's empty.
pub fn ended(text: &str) -> String {
    if text.is_empty() || text.ends_with('\n') { text.to_owned() } else { format!("{text}\n") }
}

/// Which side to take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The pull request's branch.
    Yours,
    /// The base: main.
    Base,
    /// Both, the branch's lines first.
    Both,
}

/// How a conflict was resolved, and by whom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    /// What replaces the conflict's markers.
    pub text: String,
    pub by: By,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    Side(Side),
    /// Written in the result pane.
    Hand,
    /// Written by the coding agent.
    Agent,
}

/// Why the base changed a conflict's lines: the newest commit on it that did, since the branch
/// forked, and the pull request that commit came in with, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Why {
    pub commit: String,
    pub subject: String,
    pub author: String,
    pub at: Timestamp,
    pub pull: Option<WhyPull>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhyPull {
    pub number: u64,
    pub title: String,
    pub author: String,
    /// The first line of its description, when it has one.
    pub reason: Option<String>,
    /// When it was merged: its merge commit's date.
    pub merged: Option<Timestamp>,
}

/// A lockfile that took the base's side and was written again by its tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regenerated {
    pub path: String,
    pub with: String,
}

/// A merge under way: the commit it started from, and every conflicted file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merge {
    /// HEAD before the merge, which aborting returns to.
    pub head: String,
    /// The base's commit merged in.
    pub base_commit: String,
    pub files: Vec<File>,
    pub regenerated: Vec<Regenerated>,
    /// Conflicted lockfiles left as conflicts, and the tool that would have written them.
    pub lockfiles: Vec<(String, &'static str)>,
    /// Conflicted files with no markers to show (deleted on one side, binary): they're
    /// resolved outside the window.
    pub elsewhere: Vec<String>,
}

impl Merge {
    /// Every conflict, across files, in order: (file, conflict in it).
    pub fn all(&self) -> Vec<(usize, usize)> {
        self.files.iter().enumerate().flat_map(|(f, file)| (0..file.conflicts.len()).map(move |c| (f, c))).collect()
    }

    pub fn total(&self) -> usize {
        self.files.iter().map(|f| f.conflicts.len()).sum()
    }

    pub fn open(&self) -> usize {
        self.files.iter().map(File::open).sum()
    }

    /// Conflict `n`, counting across files.
    pub fn conflict(&self, n: usize) -> Option<(&File, &Conflict)> {
        let (f, c) = *self.all().get(n)?;
        Some((&self.files[f], &self.files[f].conflicts[c]))
    }

    pub fn conflict_mut(&mut self, n: usize) -> Option<&mut Conflict> {
        let (f, c) = *self.all().get(n)?;
        Some(&mut self.files[f].conflicts[c])
    }

    /// The file conflict `n` is in.
    pub fn file_of(&self, n: usize) -> Option<usize> {
        self.all().get(n).map(|(f, _)| *f)
    }

    /// The first open conflict at or after `from`, wrapping around.
    pub fn next_open(&self, from: usize) -> Option<usize> {
        let all = self.all();
        let n = all.len();
        (0..n).map(|i| (from + i) % n).find(|&i| {
            let (f, c) = all[i];
            self.files[f].conflicts[c].resolution.is_none()
        })
    }
}
