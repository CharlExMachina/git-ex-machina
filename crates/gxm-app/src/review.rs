//! Looking at one pull request: on the graph (its lane lit, the inspector as the pull request)
//! and in the review cockpit (its files, its diff with threads, the verdict). The review in
//! progress is a draft kept on this machine, saved as it changes and sent as one review.

use std::path::PathBuf;

use gxm_core::forge::drafts::{self, ReviewDraft};
use gxm_core::forge::local::Changes;
use gxm_core::forge::model::{NewComment, Reply, Side, Verdict};
use gxm_core::forge::{RepoKey, model::Pull};
use rune::TextEdit;

use crate::pulls::{Details, Link, Sending};
use crate::state::{App, Selection};

/// What the window shows.
pub enum Mode {
    History,
    Pull(Box<PullView>),
}

pub struct PullView {
    pub number: u64,
    pub tab: PullTab,
    pub cockpit: Option<Cockpit>,
    pub draft: ReviewDraft,
    draft_path: Option<PathBuf>,
    /// The review's summary, saved into the draft as it's left.
    pub summary: TextEdit,
    /// The history's selection before the pull request opened, given back when it closes.
    returning: Selection,
    /// While confirming Squash and merge: the squash commit's message.
    pub merging: Option<TextEdit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullTab {
    Overview,
    Checks,
    Conversation,
}

pub struct Cockpit {
    /// The file shown; the first one until the person picks another.
    pub file: Option<String>,
    pub split: bool,
    pub composer: Option<Composer>,
    /// The hunk `j` and `k` last moved to.
    pub change: usize,
    /// One commit shown alone, with its changes; None shows the whole pull request.
    /// Its id at once; its changes once they're read.
    pub commit: Option<(String, Option<Changes>)>,
}

/// A comment being written.
pub struct Composer {
    pub target: Target,
    pub edit: TextEdit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Reply(String),
    Line { path: String, side: Side, line: u32 },
}

impl App {
    pub fn pull_view(&self) -> Option<&PullView> {
        match &self.mode {
            Mode::Pull(v) => Some(v),
            Mode::History => None,
        }
    }

    fn pull_view_mut(&mut self) -> Option<&mut PullView> {
        match &mut self.mode {
            Mode::Pull(v) => Some(v),
            Mode::History => None,
        }
    }

    pub fn cockpit(&self) -> Option<&Cockpit> {
        self.pull_view()?.cockpit.as_ref()
    }

    fn cockpit_mut(&mut self) -> Option<&mut Cockpit> {
        self.pull_view_mut()?.cockpit.as_mut()
    }

    /// The open pull request's details, when they've been read.
    pub fn pull_details(&self) -> Option<&Details> {
        self.pulls.details.get(&self.pull_view()?.number)
    }

    pub fn open_pull_full(&self) -> Option<&Pull> {
        self.pull_details().map(|d| &d.pull)
    }

    /// What the open pull request changes, once git has answered.
    pub fn pull_changes(&self) -> Option<&Changes> {
        self.pull_details()?.changes.as_ref()?.as_ref().ok()
    }

    /// Opens pull request `n` on the graph, with the review in progress, if any.
    pub fn open_pull(&mut self, n: u64) {
        self.pulls.want(&self.env, n, true);
        let draft_path = match &self.pulls.link {
            Link::Connected { key: RepoKey { host, owner, name }, .. } => {
                Some(drafts::path(&self.env, host, owner, name, n))
            }
            _ => None,
        };
        // The saved draft is read off the UI thread and taken when it lands (see `land_pulls`).
        if let Some(path) = &draft_path {
            self.pulls.disk.read_draft(path.clone());
        }
        let draft = ReviewDraft::default();
        let summary = TextEdit::new("");
        let returning = std::mem::take(&mut self.selection);
        self.mode = Mode::Pull(Box::new(PullView {
            number: n,
            tab: PullTab::Overview,
            cockpit: None,
            draft,
            draft_path,
            summary,
            returning,
            merging: None,
        }));
        self.relight();
        self.refresh_details();
    }

    /// Back to the plain history, with the selection it had.
    pub fn close_pull(&mut self) {
        self.save_draft();
        if let Mode::Pull(v) = std::mem::replace(&mut self.mode, Mode::History) {
            self.selection = v.returning;
        }
        self.relight();
        self.refresh_details();
    }

    pub fn open_cockpit(&mut self) {
        if let Some(v) = self.pull_view_mut() {
            v.cockpit = Some(Cockpit { file: None, split: false, composer: None, change: 0, commit: None });
        }
    }

    pub fn close_cockpit(&mut self) {
        self.save_draft();
        if let Some(v) = self.pull_view_mut() {
            v.cockpit = None;
        }
    }

    pub fn set_pull_tab(&mut self, tab: PullTab) {
        if let Some(v) = self.pull_view_mut() {
            v.tab = tab;
        }
    }

    /// The file the cockpit shows: the one picked, else the first.
    pub fn cockpit_file(&self) -> Option<&str> {
        let picked = self.cockpit()?.file.as_deref();
        picked.or_else(|| self.shown_changes()?.files.first().map(|f| f.path.as_str()))
    }

    pub fn select_pull_file(&mut self, path: &str) {
        if let Some(c) = self.cockpit_mut() {
            c.file = Some(path.to_owned());
            c.change = 0;
            c.composer = None;
        }
    }

    pub fn set_split(&mut self, split: bool) {
        if let Some(c) = self.cockpit_mut() {
            c.split = split;
        }
    }

    pub fn toggle_viewed(&mut self) {
        let Some(path) = self.cockpit_file().map(str::to_owned) else { return };
        if let Some(v) = self.pull_view_mut() {
            v.draft.toggle_viewed(&path);
        }
        self.save_draft();
    }

    /// Starts writing a reply or a comment on a line.
    pub fn compose(&mut self, target: Target) {
        if let Some(c) = self.cockpit_mut() {
            c.composer = Some(Composer { target, edit: TextEdit::new("") });
        }
    }

    /// Keeps what the composer holds as a draft, to send with the review.
    pub fn keep_comment(&mut self) {
        let Some(v) = self.pull_view_mut() else { return };
        let Some(composer) = v.cockpit.as_mut().and_then(|c| c.composer.take()) else { return };
        let body = composer.edit.text.trim().to_owned();
        if body.is_empty() {
            return;
        }
        match composer.target {
            Target::Reply(thread) => v.draft.replies.push(Reply { thread, body }),
            Target::Line { path, side, line } => v.draft.comments.push(NewComment { path, line, side, body }),
        }
        self.save_draft();
    }

    pub fn set_verdict(&mut self, verdict: Verdict) {
        if let Some(v) = self.pull_view_mut() {
            v.draft.verdict = verdict;
        }
        self.save_draft();
    }

    /// Moves to the next (`1`) or previous (`-1`) hunk of the file; returns its key.
    pub fn step_change(&mut self, by: isize) -> Option<String> {
        let path = self.cockpit_file()?.to_owned();
        let count = self.shown_changes()?.diffs.get(&path).map_or(0, Vec::len);
        let c = self.cockpit_mut()?;
        c.change = c.change.saturating_add_signed(by).min(count.saturating_sub(1));
        Some(crate::keys::hunk(c.change))
    }

    /// Sends the review: the verdict, the summary, and every draft comment and reply.
    pub fn submit_review(&mut self) {
        self.save_draft();
        let Some(v) = self.pull_view() else { return };
        let (n, submission) = (v.number, v.draft.submission());
        self.pulls.submit(&self.env, n, submission);
    }

    /// Writes the draft to disk (the summary included).
    pub fn save_draft(&mut self) {
        let Some(v) = self.pull_view_mut() else { return };
        v.draft.summary = v.summary.text.clone();
        if let Some(path) = v.draft_path.clone() {
            let draft = v.draft.clone();
            self.pulls.disk.save_draft(path, draft);
        }
    }

    /// Takes the provider's answers. Once a review is with the provider its drafts go (the
    /// files marked viewed stay), and once the pull request has been read again the cockpit
    /// closes on the graph showing the review.
    pub fn land_pulls(&mut self) -> bool {
        let before = self.pulls.machine.sending.clone();
        let landed = self.pulls.land(&self.env);
        let changed = landed.changed;
        // A saved draft lands unless the person started a new one in the meantime.
        if let Some((path, draft)) = landed.draft
            && let Some(v) = self.pull_view_mut()
            && v.draft_path.as_ref() == Some(&path)
            && v.draft == ReviewDraft::default()
            && v.summary.text.is_empty()
        {
            v.summary = TextEdit::new(draft.summary.clone());
            v.draft = draft;
        }
        if let Some((id, changes)) = landed.commit
            && let Some(c) = self.cockpit_mut()
            && c.commit.as_ref().is_some_and(|(want, _)| *want == id)
        {
            // A commit git can't show leaves the cockpit on the whole pull request.
            c.commit = changes.ok().map(|changes| (id, Some(changes)));
        }
        if changed && self.pull_view().is_some() {
            self.relight();
        }
        for n in landed.merged {
            if let Some(v) = self.pull_view_mut().filter(|v| v.number == n) {
                v.merging = None;
            }
            // The merge is on the provider; fetching brings it into the history.
            self.start(gxm_core::ops::Op::Fetch);
        }
        for n in landed.sent {
            if let Some(v) = self.pull_view_mut().filter(|v| v.number == n) {
                v.draft = ReviewDraft { viewed: std::mem::take(&mut v.draft.viewed), ..ReviewDraft::default() };
                v.summary = TextEdit::new("");
            }
            self.save_draft();
        }
        if let Sending::Sent { number, .. } = self.pulls.machine.sending
            && before != self.pulls.machine.sending
            && let Some(v) = self.pull_view_mut().filter(|v| v.number == number)
        {
            v.cockpit = None;
            v.tab = PullTab::Overview;
        }
        changed
    }
}

impl App {
    /// Starts reading the inspected commit's pull request, if it has one.
    pub fn prefetch_pull(&mut self) {
        if let Some(crate::state::Subject::Head(id) | crate::state::Subject::Commit(id)) = self.subject()
            && let Some(n) = self.pulls.of_commit(&id).map(|p| p.number)
        {
            self.pulls.want(&self.env, n, false);
        }
    }
}

impl App {
    /// The comment being written, for its field.
    pub fn composer_edit(&mut self) -> &mut TextEdit {
        match &mut self.mode {
            Mode::Pull(v) => match v.cockpit.as_mut().and_then(|c| c.composer.as_mut()) {
                Some(c) => &mut c.edit,
                None => &mut self.scratch,
            },
            Mode::History => &mut self.scratch,
        }
    }

    /// The review's summary, for its field.
    pub fn summary_edit(&mut self) -> &mut TextEdit {
        match &mut self.mode {
            Mode::Pull(v) => &mut v.summary,
            Mode::History => &mut self.scratch,
        }
    }
}

impl App {
    /// Opens the merge box's confirmation, with the squash commit's message to edit.
    pub fn start_merge(&mut self) {
        let title = self.open_pull_full().map(|p| format!("{} (#{})", p.summary.title, p.summary.number));
        if let (Some(title), Some(v)) = (title, self.pull_view_mut()) {
            v.tab = PullTab::Overview;
            v.merging = Some(TextEdit::new(title));
        }
    }

    pub fn cancel_merge(&mut self) {
        if let Some(v) = self.pull_view_mut() {
            v.merging = None;
        }
    }

    /// Squashes and merges the open pull request on the provider.
    pub fn confirm_merge(&mut self) {
        let Some(v) = self.pull_view() else { return };
        let Some(headline) = v.merging.as_ref().map(|e| e.text.trim().to_owned()).filter(|h| !h.is_empty()) else {
            return;
        };
        let n = v.number;
        self.pulls.merge(&self.env, n, headline);
    }

    /// The squash commit's message, for its field.
    pub fn merge_headline_edit(&mut self) -> &mut TextEdit {
        match &mut self.mode {
            Mode::Pull(v) => match &mut v.merging {
                Some(e) => e,
                None => &mut self.scratch,
            },
            Mode::History => &mut self.scratch,
        }
    }

    /// Shows one of the pull request's commits alone in the cockpit (or all again, with None).
    pub fn show_pull_commit(&mut self, id: Option<String>) {
        let repo = match &self.pulls.link {
            Link::Connected { repo, .. } => repo.clone(),
            _ => return,
        };
        if let Some(id) = &id {
            self.pulls.disk.read_commit(&self.env, repo, id.clone());
        }
        if let Some(c) = self.cockpit_mut() {
            c.commit = id.map(|id| (id, None));
            c.file = None;
            c.change = 0;
            c.composer = None;
        }
    }

    /// What the cockpit shows: one commit's changes, or the whole pull request's.
    pub fn shown_changes(&self) -> Option<&Changes> {
        match self.cockpit().and_then(|c| c.commit.as_ref()) {
            Some((_, changes)) => changes.as_ref(),
            None => self.pull_changes(),
        }
    }
}
