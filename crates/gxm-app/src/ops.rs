//! The toolbar's git operations, run off the UI thread as a small machine: `Idle`, `Running`
//! while git works, then back to `Idle` with the repository re-read, or `Failed` with git's
//! message until the next attempt or another repository.

use gxm_core::git::GitError;
use gxm_core::ops::{self, Op};
use gxm_core::repo::RepoSummary;
use rune::Latest;

use crate::keys;
use crate::state::{App, Row, Selection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpState {
    Idle,
    Running(Op),
    Failed { op: Op, message: String },
}

/// The machine's state and the background job carrying out its one operation.
pub struct Ops {
    pub state: OpState,
    job: Latest<Result<(), String>>,
}

impl Default for Ops {
    fn default() -> Self {
        Ops { state: OpState::Idle, job: Latest::default() }
    }
}

impl Ops {
    /// The window's waker, so an operation finishing wakes `poll`.
    pub fn set_waker(&self, waker: rune::Waker) {
        self.job.set_waker(waker);
    }

    pub fn running(&self) -> Option<&Op> {
        match &self.state {
            OpState::Running(op) => Some(op),
            _ => None,
        }
    }

    /// Git's reason, from its output: the `fatal:` or `error:` line, else the first line.
    pub fn reason(message: &str) -> &str {
        let lines = || message.lines().map(str::trim).filter(|l| !l.is_empty());
        lines()
            .find_map(|l| l.strip_prefix("fatal: ").or_else(|| l.strip_prefix("error: ")))
            .or_else(|| lines().next())
            .unwrap_or("git failed")
    }
}

impl App {
    /// Starts `op` on the selected repository, unless one is already running.
    pub fn start(&mut self, op: Op) {
        if self.ops.running().is_some() {
            return;
        }
        let Some(path) = self.repo().map(|r| r.summary.path.clone()) else { return };
        self.ops.state = OpState::Running(op.clone());
        let env = self.env.clone();
        self.ops.job.request((env, path, op), |(env, path, op)| {
            ops::run(&env, &path, &op).map_err(|e| match e {
                GitError::Failed { stderr, .. } if !stderr.trim().is_empty() => stderr,
                other => other.to_string(),
            })
        });
    }

    /// Takes a finished operation's result. True when the state changed; after a success the
    /// history scrolls to HEAD, where commits it made appear.
    pub fn land(&mut self, tx: &mut rune::Tx) -> bool {
        let Some(result) = self.ops.job.take() else { return false };
        let Some(op) = self.ops.running().cloned() else { return false };
        let succeeded = result.is_ok();
        self.ops.state = match result {
            Ok(()) => {
                self.reload_repo();
                if matches!(op, Op::Commit(_)) {
                    // Commit opened the message itself; with the work committed, back to Files.
                    self.message = rune::TextEdit::new("");
                    self.tab = crate::state::InspectorTab::Files;
                }
                OpState::Idle
            }
            Err(message) => OpState::Failed { op, message },
        };
        if let Some(head) = self.head().filter(|_| succeeded) {
            tx.scroll_into_view_in(keys::HISTORY, head.to_owned());
        }
        true
    }

    /// Reads the selected repository again (status, history, inspector), keeping the selection
    /// where its rows are still there.
    fn reload_repo(&mut self) {
        let Some(i) = self.selected_repo else { return };
        let Some(entry) = self.repos.get(i) else { return };
        if let Ok(summary) = RepoSummary::read(&self.env, &entry.summary.path) {
            self.repos[i].summary = summary;
        }
        let (keep, tab) = (self.selection.clone(), self.tab);
        self.select_repo(i);
        let Some(h) = &self.history else { return };
        let has = |id: &str| h.rows.iter().any(|r| r.id() == id);
        let kept = if keep.worktree {
            let still = h.rows.iter().any(|r| matches!(r, Row::Worktree { .. }));
            if still { keep } else { Selection::default() }
        } else {
            let commits: Vec<String> = keep.commits.into_iter().filter(|c| has(c)).collect();
            let anchor = keep.anchor.filter(|a| has(a) && a != keys::WORKTREE_ROW);
            Selection { commits, anchor, worktree: false }
        };
        self.selection = kept;
        self.tab = tab;
        self.refresh_details();
    }
}
