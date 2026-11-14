//! The app's state and what changes it.

use gxm_core::diff::{self, Hunk};
use gxm_core::env::Env;
use gxm_core::graph::{self, GraphRow};
use gxm_core::history::{self, ChangeStatus, Commit, FileChange};
use gxm_core::repo::RepoSummary;
use gxm_core::workspace::Workspace;
use rune::TextEdit;

use crate::keys;
use crate::ops::{OpState, Ops};

/// How many commits the history loads. Paging past this is future work (see the perf budget).
pub const HISTORY_LIMIT: usize = 2000;

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
    /// The commit message, written in the inspector's Message tab while the working tree is
    /// shown.
    pub message: TextEdit,
}

/// Loading is an explicit machine: nothing is read before the first frame is up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Load {
    NotStarted,
    Loaded,
    Failed(String),
}

pub struct RepoEntry {
    pub summary: RepoSummary,
    pub pinned: bool,
}

/// The selected repository's history.
pub struct History {
    pub rows: Vec<Row>,
    pub graph: Vec<GraphRow>,
    /// The widest row's lane count.
    pub cols: usize,
    /// Commits reachable from any ref (may exceed `rows`).
    pub total: usize,
    pub details: Option<Details>,
    pub error: Option<String>,
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

/// What the history has selected, as the design's `pick` keeps it: a click selects one row
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
    Message,
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
    pub body: String,
    pub files: Vec<FileChange>,
    /// The file whose first hunk the inspector previews.
    pub file: Option<String>,
    pub hunk: Option<Hunk>,
    pub error: Option<String>,
}

impl App {
    /// A window's state before anything is read. Cheap: no files, no git.
    pub fn new(env: Env) -> App {
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
            message: TextEdit::new(""),
        }
    }

    /// Reads the workspace and opens the first pinned repository (or the first one).
    pub fn load_workspace(&mut self) {
        let ws = match Workspace::load(&self.env) {
            Ok(ws) => ws,
            Err(e) => {
                self.load = Load::Failed(e.to_string());
                return;
            }
        };
        self.repos = ws
            .repos
            .iter()
            .filter_map(|r| {
                RepoSummary::read(&self.env, &r.path).ok().map(|summary| RepoEntry { summary, pinned: r.pinned })
            })
            .collect();
        self.load = Load::Loaded;
        let first = self.repos.iter().position(|r| r.pinned).or(if self.repos.is_empty() { None } else { Some(0) });
        if let Some(i) = first {
            self.select_repo(i);
        }
    }

    pub fn repo(&self) -> Option<&RepoEntry> {
        self.selected_repo.and_then(|i| self.repos.get(i))
    }

    /// Opens a repository's history with nothing selected, so the inspector shows HEAD.
    pub fn select_repo(&mut self, i: usize) {
        let Some(entry) = self.repos.get(i) else { return };
        let (env, path) = (&self.env, entry.summary.path.clone());
        self.selected_repo = Some(i);
        let mut rows = Vec::new();
        let mut error = None;
        let changed = entry.summary.status.changed;
        let head = entry.summary.status.head.clone();
        match history::log(env, &path, HISTORY_LIMIT) {
            Ok(commits) => {
                if changed > 0 && head.is_some() {
                    rows.push(Row::Worktree { changed });
                }
                rows.extend(commits.into_iter().map(Row::Commit));
            }
            Err(e) => error = Some(e.to_string()),
        }
        let graph = graph::layout(rows.iter().map(|r| match r {
            Row::Worktree { .. } => {
                (keys::WORKTREE_ROW, head.as_slice().iter().map(String::as_str).collect::<Vec<_>>())
            }
            Row::Commit(c) => (c.id.as_str(), c.parents.iter().map(String::as_str).collect()),
        }));
        let cols = graph.iter().map(|g| g.width).max().unwrap_or(1);
        let total = history::count(env, &path).unwrap_or(rows.len());
        self.history = Some(History { rows, graph, cols, total, details: None, error });
        self.selection = Selection::default();
        if matches!(self.ops.state, OpState::Failed { .. }) {
            self.ops.state = OpState::Idle;
        }
        self.refresh_details();
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

    /// What a multi-selection is, as the design reads it: `linear` when every commit is on one
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

    /// Loads the inspector's details when its subject changed. True when it changed anything.
    pub fn refresh_details(&mut self) -> bool {
        let want = self.subject();
        let Some(h) = &self.history else { return false };
        if h.details.as_ref().map(|d| &d.subject) == want.as_ref() {
            return false;
        }
        let Some(repo) = self.repo().map(|r| r.summary.path.clone()) else { return false };
        let env = &self.env;
        let details = want.map(|subject| {
            let (body, files) = match &subject {
                Subject::Worktree => (Ok(String::new()), history::uncommitted(env, &repo)),
                Subject::Head(id) | Subject::Commit(id) => {
                    (history::body(env, &repo, id), history::changes(env, &repo, id))
                }
                Subject::Commits(ids) => {
                    let mut all: Vec<FileChange> = Vec::new();
                    let mut error = None;
                    for id in ids {
                        match history::changes(env, &repo, id) {
                            Ok(files) => {
                                for f in files {
                                    if !all.iter().any(|a| a.path == f.path) {
                                        all.push(f);
                                    }
                                }
                            }
                            Err(e) => error = Some(e),
                        }
                    }
                    (Ok(String::new()), error.map_or(Ok(all), Err))
                }
            };
            let error = body.as_ref().err().or(files.as_ref().err()).map(ToString::to_string);
            let mut files = files.unwrap_or_default();
            review_order(&mut files);
            let mut d = Details { subject, body: body.unwrap_or_default(), files, file: None, hunk: None, error };
            let first = d.files.first().map(|f| f.path.clone());
            preview(env, &repo, &mut d, first);
            d
        });
        if let Some(h) = &mut self.history {
            h.details = details;
        }
        true
    }

    /// Previews `path`'s first hunk in the inspector.
    pub fn select_file(&mut self, path: &str) {
        let Some(repo) = self.repo().map(|r| r.summary.path.clone()) else { return };
        let env = self.env.clone();
        if let Some(d) = self.history.as_mut().and_then(|h| h.details.as_mut()) {
            preview(&env, &repo, d, Some(path.to_owned()));
        }
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

/// Files a reviewer reads first come first: changed and added files by path, then renames,
/// then deletions, whose content is gone.
fn review_order(files: &mut [FileChange]) {
    files.sort_by(|a, b| {
        let rank = |f: &FileChange| match f.status {
            ChangeStatus::Renamed | ChangeStatus::Copied => 1,
            ChangeStatus::Deleted => 2,
            _ => 0,
        };
        rank(a).cmp(&rank(b)).then_with(|| a.path.cmp(&b.path))
    });
}

fn preview(env: &Env, repo: &std::path::Path, d: &mut Details, path: Option<String>) {
    d.hunk = path.as_deref().and_then(|p| match &d.subject {
        Subject::Worktree => diff::worktree_hunk(env, repo, p).ok().flatten(),
        Subject::Head(id) | Subject::Commit(id) => diff::commit_hunk(env, repo, id, p).ok().flatten(),
        Subject::Commits(ids) => ids.iter().find_map(|id| diff::commit_hunk(env, repo, id, p).ok().flatten()),
    });
    d.file = path;
}
