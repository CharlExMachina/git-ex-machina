//! The app's state and what changes it.

use std::path::PathBuf;

use gxm_core::diff::Hunk;
use gxm_core::env::Env;
use gxm_core::graph::{self, GraphRow};
use gxm_core::history::{Commit, CommitInfo, FileChange};
use gxm_core::repo::RepoSummary;
use rune::TextEdit;

use crate::account::AccountJobs;
use crate::keys;
use crate::load::Loads;
use crate::ops::Ops;
use crate::pulls::Pulls;
use crate::review::Mode;
use crate::sheet::{Fields, Sheet, SheetJobs};

pub struct App {
    pub env: Env,
    pub load: Load,
    pub repos: Vec<RepoEntry>,
    /// Index into `repos`.
    pub selected_repo: Option<usize>,
    pub history: Option<History>,
    pub selection: Selection,
    pub filter: TextEdit,
    pub search: TextEdit,
    pub tab: InspectorTab,
    pub sidebar_open: bool,
    pub inspector_open: bool,
    /// The toolbar's git operations.
    pub ops: Ops,
    /// The reads running off the UI thread.
    pub loads: Loads,
    /// The commit message, written in the inspector's Message tab while the working tree is
    /// shown.
    pub message: TextEdit,
    /// The selected repository's pull requests.
    pub pulls: Pulls,
    /// The history, or a pull request on it.
    pub mode: Mode,
    /// Where a field's edits land once what it edited is gone (a comment kept a moment ago).
    pub scratch: TextEdit,
    /// Add or Clone, over the window.
    pub sheet: Sheet,
    pub fields: Fields,
    pub sheet_jobs: SheetJobs,
    /// Who's signed in to GitHub; None when no one is.
    pub account: Option<gxm_core::forge::model::User>,
    pub account_jobs: AccountJobs,
}

/// Loading is an explicit machine: nothing is read before the first frame is up, and the
/// workspace is read off the UI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Load {
    NotStarted,
    Loading,
    Loaded,
    Failed(String),
}

pub struct RepoEntry {
    pub summary: RepoSummary,
    pub pinned: bool,
}

/// The selected repository's history.
pub struct History {
    /// The repository it was read from.
    pub path: PathBuf,
    pub rows: Vec<Row>,
    pub graph: Vec<GraphRow>,
    /// The widest row's lane count.
    pub cols: usize,
    /// Commits reachable from any ref (may exceed `rows`), once counted: a big repository
    /// takes a while, so the history shows before it.
    pub total: Option<usize>,
    pub details: Option<Details>,
    pub error: Option<String>,
    /// With a pull request open: which parts of each row are its (see [`App::relight`]).
    pub lit: Option<Vec<graph::Lit>>,
}

pub enum Row {
    /// Uncommitted work, drawn above HEAD.
    Worktree {
        changed: u32,
    },
    Commit(Commit),
}

impl Row {
    pub fn id(&self) -> &str {
        match self {
            Row::Worktree { .. } => keys::WORKTREE_ROW,
            Row::Commit(c) => &c.id,
        }
    }
}

/// What the history has selected: a click selects one row
/// (or clears it when it was the only one), ⌘-click toggles a row, ⇧-click selects the range
/// from the anchor. Pressing the uncommitted row shows the working tree instead.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// Commit ids, in history order.
    pub commits: Vec<String>,
    pub anchor: Option<String>,
    /// The uncommitted row is pressed.
    pub worktree: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectorTab {
    /// "Overview" for a commit (its message and details), "Message" on the uncommitted row
    /// (the commit message being written).
    Overview,
    Files,
}

/// See [`App::shape`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub linear: bool,
    pub merge: bool,
}

/// Which modifier a click on a history row came with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Plain,
    Toggle,
    Range,
}

/// What the inspector is showing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
    /// Nothing selected: HEAD's commit.
    Head(String),
    Commit(String),
    Worktree,
    Commits(Vec<String>),
}

/// The inspector's data for its subject.
pub struct Details {
    pub subject: Subject,
    /// A single commit's message, committer, trailers and containing refs.
    pub info: Option<CommitInfo>,
    pub files: Vec<FileChange>,
    /// The file whose first hunk the inspector previews.
    pub file: Option<String>,
    pub hunk: Option<Hunk>,
    pub error: Option<String>,
}

impl App {
    /// A window's state before anything is read. Cheap: no files, no git. Call it on the thread
    /// that draws the window: debug builds then panic if git or a file read ever runs there.
    pub fn new(mut env: Env) -> App {
        env.claim_ui_thread();
        let env_token = env.forge.github_token.clone();
        App {
            env,
            load: Load::NotStarted,
            repos: Vec::new(),
            selected_repo: None,
            history: None,
            selection: Selection::default(),
            filter: TextEdit::new(""),
            search: TextEdit::new(""),
            tab: InspectorTab::Files,
            sidebar_open: true,
            inspector_open: true,
            ops: Ops::default(),
            loads: Loads::default(),
            message: TextEdit::new(""),
            pulls: Pulls::default(),
            mode: Mode::History,
            scratch: TextEdit::new(""),
            sheet: Sheet::Closed,
            fields: Fields::default(),
            sheet_jobs: SheetJobs::default(),
            account: None,
            account_jobs: AccountJobs::new(env_token),
        }
    }

    /// A read, a write or a provider call is in flight: what's on screen may be about to change.
    pub fn busy(&self) -> bool {
        self.loads.busy() || self.pulls.busy() || self.account_jobs.reading()
    }

    /// Lights the open pull request's commits in the graph, or clears the light.
    pub fn relight(&mut self) {
        let n = self.pull_view().map(|v| v.number);
        let Some(h) = &self.history else { return };
        let lit = n.map(|n| {
            let member: Vec<bool> = h
                .rows
                .iter()
                .map(|r| matches!(r, Row::Commit(c) if self.pulls.of_commit(&c.id).is_some_and(|p| p.number == n)))
                .collect();
            graph::light(&h.graph, &member)
        });
        if let Some(h) = &mut self.history {
            h.lit = lit;
        }
    }

    /// The row a pull request's banner sits above: its newest commit in the history.
    pub fn pull_tip(&self) -> Option<usize> {
        let lit = self.history.as_ref()?.lit.as_ref()?;
        lit.iter().position(|l| l.node)
    }

    pub fn repo(&self) -> Option<&RepoEntry> {
        self.selected_repo.and_then(|i| self.repos.get(i))
    }

    /// The commit at HEAD, if the history has one.
    pub fn head(&self) -> Option<&str> {
        self.repo().and_then(|r| r.summary.status.head.as_deref())
    }

    pub fn subject(&self) -> Option<Subject> {
        let s = &self.selection;
        if s.worktree {
            return Some(Subject::Worktree);
        }
        match s.commits.as_slice() {
            [] => self.head().map(|h| Subject::Head(h.to_owned())),
            [one] => Some(Subject::Commit(one.clone())),
            many => Some(Subject::Commits(many.to_vec())),
        }
    }

    /// A click on the history row `id`.
    pub fn pick(&mut self, id: &str, how: Pick) {
        let Some(h) = &self.history else { return };
        if id == keys::WORKTREE_ROW {
            self.selection = Selection { worktree: !self.selection.worktree, ..Selection::default() };
            self.refresh_details();
            return;
        }
        let order = |id: &str| h.rows.iter().position(|r| r.id() == id);
        let cur = &self.selection.commits;
        let mut next = match how {
            Pick::Range if self.selection.anchor.is_some() => {
                let (Some(a), Some(b)) = (self.selection.anchor.as_deref().and_then(order), order(id)) else { return };
                let (lo, hi) = (a.min(b), a.max(b));
                let ids: Vec<String> =
                    h.rows[lo..=hi].iter().filter(|r| matches!(r, Row::Commit(_))).map(|r| r.id().to_owned()).collect();
                self.selection = Selection { commits: ids, anchor: self.selection.anchor.clone(), worktree: false };
                self.refresh_details();
                return;
            }
            Pick::Toggle if cur.iter().any(|c| c == id) => cur.iter().filter(|c| *c != id).cloned().collect(),
            Pick::Toggle => cur.iter().cloned().chain([id.to_owned()]).collect(),
            _ if cur.len() == 1 && cur[0] == id => Vec::new(),
            _ => vec![id.to_owned()],
        };
        next.sort_by_key(|c| order(c));
        self.selection = Selection { commits: next, anchor: Some(id.to_owned()), worktree: false };
        self.refresh_details();
    }

    /// Moves a single selection one row up (`-1`) or down (`1`), starting from HEAD.
    /// Returns the newly selected row's id.
    pub fn step(&mut self, by: isize) -> Option<String> {
        let h = self.history.as_ref()?;
        let current = if self.selection.worktree {
            Some(keys::WORKTREE_ROW.to_owned())
        } else {
            self.selection.anchor.clone().or_else(|| self.head().map(str::to_owned))
        };
        let at = current.and_then(|id| h.rows.iter().position(|r| r.id() == id)).unwrap_or(0);
        let next = at.checked_add_signed(by).filter(|i| *i < h.rows.len())?;
        let id = h.rows[next].id().to_owned();
        self.selection = if id == keys::WORKTREE_ROW {
            Selection { worktree: true, ..Selection::default() }
        } else {
            Selection { commits: vec![id.clone()], anchor: Some(id.clone()), worktree: false }
        };
        self.refresh_details();
        Some(id)
    }

    /// Selects the first commit, from the top (or after the selection, with `next`), whose
    /// subject, author or id holds the search text. Returns its id.
    pub fn find(&mut self, next: bool) -> Option<String> {
        let q = self.search.text.trim().to_lowercase();
        if q.is_empty() {
            return None;
        }
        let h = self.history.as_ref()?;
        let after = if next { self.selection.anchor.as_deref() } else { None };
        let start = after.and_then(|id| h.rows.iter().position(|r| r.id() == id)).map_or(0, |i| i + 1);
        let hit = |r: &Row| match r {
            Row::Commit(c) => {
                c.subject.to_lowercase().contains(&q) || c.author.to_lowercase().contains(&q) || c.id.starts_with(&q)
            }
            Row::Worktree { .. } => false,
        };
        let found = h.rows[start..].iter().chain(&h.rows[..start]).find(|r| hit(r))?.id().to_owned();
        self.selection = Selection { commits: vec![found.clone()], anchor: Some(found.clone()), worktree: false };
        self.refresh_details();
        Some(found)
    }

    /// What a multi-selection is: `linear` when every commit is on one
    /// lane with no other commit of that lane between them, and whether any is a merge.
    pub fn shape(&self, ids: &[String]) -> Shape {
        let Some(h) = &self.history else { return Shape { linear: false, merge: false } };
        let picked: Vec<(usize, usize)> = ids
            .iter()
            .filter_map(|id| h.rows.iter().position(|r| r.id() == id).map(|i| (h.graph[i].lane, i)))
            .collect();
        let lane = picked.first().map(|p| p.0);
        let same = picked.iter().all(|p| Some(p.0) == lane);
        let linear = same
            && picked
                .windows(2)
                .all(|w| (w[0].1 + 1..w[1].1).all(|k| h.graph.get(k).is_none_or(|g| Some(g.lane) != lane)));
        let merge = ids.iter().any(|id| self.commit(id).is_some_and(Commit::is_merge));
        Shape { linear, merge }
    }

    /// Bisect needs three or more commits in a line; squash two or more, with no merge.
    pub fn can_bisect(&self, ids: &[String]) -> bool {
        ids.len() >= 3 && self.shape(ids).linear
    }

    pub fn can_squash(&self, ids: &[String]) -> bool {
        let shape = self.shape(ids);
        ids.len() >= 2 && shape.linear && !shape.merge
    }

    /// The working tree's state: everything staged, or not.
    pub fn all_staged(&self) -> bool {
        self.repo().is_some_and(|r| r.summary.status.staged > 0 && r.summary.status.staged == r.summary.status.changed)
    }

    pub fn is_selected(&self, id: &str) -> bool {
        if id == keys::WORKTREE_ROW {
            return self.selection.worktree;
        }
        self.selection.commits.iter().any(|c| c == id)
    }

    pub fn commit(&self, id: &str) -> Option<&Commit> {
        self.history.as_ref()?.rows.iter().find_map(|r| match r {
            Row::Commit(c) if c.id == id => Some(c),
            _ => None,
        })
    }

    /// Repositories matching the sidebar's filter, pinned first, as (index, entry).
    pub fn visible_repos(&self, pinned: bool) -> Vec<(usize, &RepoEntry)> {
        let q = self.filter.text.trim().to_lowercase();
        self.repos
            .iter()
            .enumerate()
            .filter(|(_, r)| r.pinned == pinned && (q.is_empty() || r.summary.name.to_lowercase().contains(&q)))
            .collect()
    }
}
