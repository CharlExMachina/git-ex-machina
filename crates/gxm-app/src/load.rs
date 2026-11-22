//! Everything the window reads from disk or git, off the UI thread.
//!
//! Each read is a latest-wins background job (`rune::Latest`): asking again supersedes the
//! request in flight, and an answer for anything but the newest request is dropped, so arrowing
//! through a thousand commits runs one read per frame at most and never shows a stale one.
//! While a read is in flight the window keeps showing what it had: a frame is never empty and
//! never waits. `Env::off_ui_thread` panics in debug builds if git or a file read slips back
//! onto the UI thread, so every journey guards this.
//!
//! The reads chain: the workspace (every repository's summary, read in parallel), then the
//! selected repository's history (its summary and log, in parallel), then the inspector's
//! details and the commit count. A file picked in the inspector reads its hunk on its own.

use std::path::{Path, PathBuf};

use gxm_core::diff::{self, Hunk};
use gxm_core::env::Env;
use gxm_core::graph;
use gxm_core::history::{self, ChangeStatus, FileChange};
use gxm_core::repo::RepoSummary;
use gxm_core::workspace::Workspace;
use rune::{Latest, Tx, Waker};

use crate::keys;
use crate::ops::OpState;
use crate::state::{App, Details, History, InspectorTab, Load, RepoEntry, Row, Selection, Subject};

/// How many commits the history loads. Paging past this is future work (see the perf budget).
pub const HISTORY_LIMIT: usize = 2000;

/// The background jobs, and what the newest request of each was for.
#[derive(Default)]
pub struct Loads {
    workspace: Latest<Result<Vec<RepoEntry>, String>>,
    history: Latest<Read>,
    count: Latest<(PathBuf, Option<usize>)>,
    details: Latest<(PathBuf, Details)>,
    hunk: Latest<(PathBuf, Subject, String, Option<Hunk>)>,
    /// What the history read in flight restores when it lands: the selection and tab from before
    /// an operation changed the repository. `None` opens it with nothing selected.
    keep: Option<(Selection, InspectorTab)>,
    /// The repository and subject the newest details request is for, so asking again for the
    /// same one is free.
    details_for: Option<(PathBuf, Subject)>,
}

/// A repository's history as read off the UI thread, ready to show.
struct Read {
    path: PathBuf,
    summary: Option<RepoSummary>,
    history: History,
}

impl Loads {
    /// The window's waker, so a read landing wakes `poll`.
    pub fn set_waker(&self, waker: Waker) {
        self.workspace.set_waker(waker.clone());
        self.history.set_waker(waker.clone());
        self.count.set_waker(waker.clone());
        self.details.set_waker(waker.clone());
        self.hunk.set_waker(waker);
    }

    /// A read is in flight: what's on screen may be about to change. An answer that has landed
    /// but not been taken yet doesn't count; the next `poll` takes it.
    pub fn busy(&self) -> bool {
        self.workspace.is_pending()
            || self.history.is_pending()
            || self.count.is_pending()
            || self.details.is_pending()
            || self.hunk.is_pending()
    }
}

impl App {
    /// Starts reading the workspace: the saved list, then every repository's summary.
    pub fn load_workspace(&mut self) {
        self.load = Load::Loading;
        // Who's signed in to GitHub, and the token that lets the pull requests in.
        self.account_jobs.remembered.request(self.env.clone(), |env| gxm_core::auth::stored(&env));
        self.loads.workspace.request(self.env.clone(), |env| {
            let ws = Workspace::load(&env).map_err(|e| e.to_string())?;
            let summaries: Vec<Option<RepoSummary>> = std::thread::scope(|scope| {
                let reads: Vec<_> =
                    ws.repos.iter().map(|r| scope.spawn(|| RepoSummary::read(&env, &r.path).ok())).collect();
                reads.into_iter().map(|h| h.join().ok().flatten()).collect()
            });
            Ok(ws
                .repos
                .iter()
                .zip(summaries)
                .filter_map(|(r, summary)| summary.map(|summary| RepoEntry { summary, pinned: r.pinned }))
                .collect())
        });
    }

    /// Opens a repository's history with nothing selected, so the inspector shows HEAD. The
    /// sidebar marks it at once; its history replaces the one on screen when the read lands.
    pub fn select_repo(&mut self, i: usize) {
        // Another repository: back to its plain history, and ask its provider for its pull
        // requests.
        if let Some(entry) = self.repos.get(i) {
            self.mode = crate::review::Mode::History;
            self.pulls.open(&self.env, entry.summary.path.clone(), entry.summary.origin.as_ref());
        }
        self.loads.keep = None;
        self.read_history(i);
        self.selection = Selection::default();
        if matches!(self.ops.state, OpState::Failed { .. }) {
            self.ops.state = OpState::Idle;
        }
    }

    /// Reads the selected repository again (status, history, inspector), keeping the selection
    /// where its rows are still there. After an operation changed it.
    pub fn reload_repo(&mut self) {
        let Some(i) = self.selected_repo else { return };
        self.loads.keep = Some((self.selection.clone(), self.tab));
        self.read_history(i);
    }

    fn read_history(&mut self, i: usize) {
        let Some(entry) = self.repos.get(i) else { return };
        self.selected_repo = Some(i);
        let path = entry.summary.path.clone();
        self.loads.history.request((self.env.clone(), path), |(env, path)| read_history(&env, path));
    }

    /// Asks for the inspector's details when its subject changed. Until they land, the inspector
    /// keeps showing the last ones.
    pub fn refresh_details(&mut self) {
        // A commit in a pull request starts reading that pull request, so it opens at once.
        self.prefetch_pull();
        let Some(path) = self.history.as_ref().map(|h| h.path.clone()) else { return };
        let Some(subject) = self.subject() else {
            self.loads.details.cancel();
            self.loads.details_for = None;
            if let Some(h) = &mut self.history {
                h.details = None;
            }
            return;
        };
        let want = (path, subject);
        if self.loads.details_for.as_ref() == Some(&want) {
            return;
        }
        self.loads.details_for = Some(want.clone());
        self.loads.hunk.cancel();
        self.loads.details.request((self.env.clone(), want), |(env, (path, subject))| {
            let d = read_details(&env, &path, subject);
            (path, d)
        });
    }

    /// Previews `path`'s first hunk in the inspector, once it's read.
    pub fn select_file(&mut self, file: &str) {
        let Some(h) = &self.history else { return };
        let Some(d) = &h.details else { return };
        let input = (self.env.clone(), h.path.clone(), d.subject.clone(), file.to_owned());
        self.loads.hunk.request(input, |(env, repo, subject, file)| {
            let hunk = first_hunk(&env, &repo, &subject, &file);
            (repo, subject, file, hunk)
        });
    }

    /// Takes whatever reads have landed. True when the state changed.
    pub fn land_loads(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(read) = self.loads.workspace.take() {
            changed = true;
            match read {
                Ok(repos) => {
                    self.repos = repos;
                    self.load = Load::Loaded;
                    let first = self.repos.iter().position(|r| r.pinned).or((!self.repos.is_empty()).then_some(0));
                    if let Some(i) = first {
                        self.select_repo(i);
                    }
                }
                Err(e) => self.load = Load::Failed(e),
            }
        }
        if let Some(read) = self.loads.history.take() {
            changed = true;
            self.land_history(read, tx);
        }
        if let Some((path, total)) = self.loads.count.take()
            && let Some(h) = self.history.as_mut().filter(|h| h.path == path)
        {
            changed = true;
            h.total = total;
        }
        if let Some((path, details)) = self.loads.details.take()
            && self.loads.details_for.as_ref().is_some_and(|(p, s)| *p == path && *s == details.subject)
            && let Some(h) = self.history.as_mut().filter(|h| h.path == path)
        {
            changed = true;
            h.details = Some(details);
        }
        if let Some((path, subject, file, hunk)) = self.loads.hunk.take()
            && let Some(d) = self.history.as_mut().filter(|h| h.path == path).and_then(|h| h.details.as_mut())
            && d.subject == subject
        {
            changed = true;
            d.file = Some(file);
            d.hunk = hunk;
        }
        changed
    }

    fn land_history(&mut self, read: Read, tx: &mut Tx) {
        let Some(i) = self.repos.iter().position(|r| r.summary.path == read.path) else { return };
        if self.selected_repo != Some(i) {
            return;
        }
        if let Some(summary) = read.summary {
            self.repos[i].summary = summary;
        }
        let mut history = read.history;
        // The same repository read again keeps what's still true until its own reads land.
        if let Some(old) = self.history.take().filter(|h| h.path == history.path) {
            history.total = old.total;
            history.details = old.details;
        }
        self.history = Some(history);
        self.relight();
        self.loads.details_for = None;
        let path = read.path;
        self.loads.count.request((self.env.clone(), path), |(env, path)| {
            let total = history::count(&env, &path).ok();
            (path, total)
        });
        match self.loads.keep.take() {
            Some((keep, tab)) => {
                self.selection = self.still_there(keep);
                self.tab = tab;
                if let Some(head) = self.head() {
                    tx.scroll_into_view_in(keys::HISTORY, head.to_owned());
                }
            }
            None => {
                // A repository with no commits yet opens on its files, waiting to be committed.
                let only_worktree =
                    self.history.as_ref().is_some_and(|h| matches!(h.rows.as_slice(), [Row::Worktree { .. }]));
                self.selection = Selection { worktree: only_worktree, ..Selection::default() };
            }
        }
        self.refresh_details();
    }

    /// The part of `keep` whose rows the history still has.
    fn still_there(&self, keep: Selection) -> Selection {
        let Some(h) = &self.history else { return Selection::default() };
        let has = |id: &str| h.rows.iter().any(|r| r.id() == id);
        if keep.worktree {
            let still = h.rows.iter().any(|r| matches!(r, Row::Worktree { .. }));
            return if still { keep } else { Selection::default() };
        }
        let commits: Vec<String> = keep.commits.into_iter().filter(|c| has(c)).collect();
        let anchor = keep.anchor.filter(|a| has(a) && a != keys::WORKTREE_ROW);
        Selection { commits, anchor, worktree: false }
    }
}

/// The repository's summary, its tags and its newest commits laid out as a graph, read side by
/// side.
fn read_history(env: &Env, path: PathBuf) -> Read {
    let (summary, tagging, log) = std::thread::scope(|scope| {
        let summary = scope.spawn(|| RepoSummary::read(env, &path).ok());
        let tagging = scope.spawn(|| gxm_core::tag::survey(env, &path));
        let log = history::log(env, &path, HISTORY_LIMIT);
        (summary.join().ok().flatten(), tagging.join().unwrap_or_default(), log)
    });
    let status = summary.as_ref().map(|s| &s.status);
    let (changed, head) = status.map_or((0, None), |s| (s.changed, s.head.clone()));
    let mut rows = Vec::new();
    let mut error = None;
    match log {
        Ok(commits) => {
            // Before the first commit too: then it's all there is.
            if changed > 0 {
                rows.push(Row::Worktree { changed });
            }
            rows.extend(commits.into_iter().map(Row::Commit));
        }
        Err(e) => error = Some(e.to_string()),
    }
    let graph = graph::layout(rows.iter().map(|r| match r {
        Row::Worktree { .. } => (keys::WORKTREE_ROW, head.as_slice().iter().map(String::as_str).collect::<Vec<_>>()),
        Row::Commit(c) => (c.id.as_str(), c.parents.iter().map(String::as_str).collect()),
    }));
    let cols = graph.iter().map(|g| g.width).max().unwrap_or(1);
    let history =
        History { path: path.clone(), rows, graph, cols, total: None, details: None, error, lit: None, tagging };
    Read { path, summary, history }
}

/// The inspector's details for `subject`: its files in review order, a single commit's message
/// and refs, and the first file's first hunk.
fn read_details(env: &Env, repo: &Path, subject: Subject) -> Details {
    let (info, files) = std::thread::scope(|scope| {
        let info = match &subject {
            Subject::Head(id) | Subject::Commit(id) => Some(scope.spawn(|| history::info(env, repo, id))),
            Subject::Worktree | Subject::Commits(_) => None,
        };
        let files = files_of(env, repo, &subject);
        let info = info.map(|h| h.join().unwrap_or_else(|_| Err(panicked())));
        (info.transpose(), files)
    });
    let error = info.as_ref().err().or(files.as_ref().err()).map(ToString::to_string);
    let mut files = files.unwrap_or_default();
    review_order(&mut files);
    let file = files.first().map(|f| f.path.clone());
    let hunk = file.as_deref().and_then(|f| first_hunk(env, repo, &subject, f));
    Details { subject, info: info.ok().flatten(), files, file, hunk, error }
}

fn panicked() -> gxm_core::git::GitError {
    gxm_core::git::GitError::Spawn { message: "the read panicked".to_owned() }
}

fn files_of(env: &Env, repo: &Path, subject: &Subject) -> Result<Vec<FileChange>, gxm_core::git::GitError> {
    match subject {
        Subject::Worktree => history::uncommitted(env, repo),
        Subject::Head(id) | Subject::Commit(id) => history::changes(env, repo, id),
        Subject::Commits(ids) => {
            let each: Vec<_> = std::thread::scope(|scope| {
                let reads: Vec<_> = ids.iter().map(|id| scope.spawn(|| history::changes(env, repo, id))).collect();
                reads.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(panicked()))).collect()
            });
            let mut all: Vec<FileChange> = Vec::new();
            for files in each {
                for f in files? {
                    if !all.iter().any(|a| a.path == f.path) {
                        all.push(f);
                    }
                }
            }
            Ok(all)
        }
    }
}

fn first_hunk(env: &Env, repo: &Path, subject: &Subject, file: &str) -> Option<Hunk> {
    match subject {
        Subject::Worktree => diff::worktree_hunk(env, repo, file).ok().flatten(),
        Subject::Head(id) | Subject::Commit(id) => diff::commit_hunk(env, repo, id, file).ok().flatten(),
        Subject::Commits(ids) => ids.iter().find_map(|id| diff::commit_hunk(env, repo, id, file).ok().flatten()),
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
