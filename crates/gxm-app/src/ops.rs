//! The toolbar's git operations, run off the UI thread as a small machine: `Idle`, `Running`
//! while git works, then back to `Idle` with the repository re-read (`load.rs`), or `Failed` with git's
//! message until the next attempt or another repository.

use gxm_core::git::GitError;
use gxm_core::ops::{self, Op};
use rune::Latest;

use crate::state::App;

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

    /// What git said when it failed: its output, else what kept it from running.
    pub fn said(e: GitError) -> String {
        match e {
            GitError::Failed { stderr, .. } if !stderr.trim().is_empty() => stderr,
            other => other.to_string(),
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
        self.ops.job.request((env, path, op), |(env, path, op)| ops::run(&env, &path, &op).map_err(Ops::said));
    }

    /// Takes a finished operation's result. True when the state changed; after a success the
    /// repository is read again, and the history scrolls to HEAD, where commits it made appear.
    pub fn land(&mut self) -> bool {
        let Some(result) = self.ops.job.take() else { return false };
        let Some(op) = self.ops.running().cloned() else { return false };
        self.ops.state = match result {
            Ok(()) => {
                if matches!(op, Op::Commit(_)) {
                    // Commit opened the message itself; with the work committed, back to Files.
                    self.message = rune::TextEdit::new("");
                    self.tab = crate::state::InspectorTab::Files;
                }
                // The history scrolls to HEAD when the re-read lands.
                self.reload_repo();
                if matches!(op, Op::Fetch | Op::Pull | Op::Push) {
                    // New commits may belong to pull requests (a merge just made, say).
                    self.pulls.refresh(&self.env);
                    if let Some(n) = self.pull_view().map(|v| v.number) {
                        self.pulls.want(&self.env, n, true);
                    }
                }
                OpState::Idle
            }
            Err(message) => OpState::Failed { op, message },
        };
        true
    }
}
