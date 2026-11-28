//! The action bar over the main view: the checkout control at its left, then what it offers,
//! which follows the selection in History, and the Work | History switch at its right.
//!
//! - in Work, or nothing selected: Fetch, Pull, Push
//! - the uncommitted row: Stage all, Discard all; with everything staged, Commit, Unstage all,
//!   Discard all
//! - one commit: Checkout, Cherry-pick, Revert, Branch from here
//! - several: Bisect (a line of three or more), Squash (consecutive, no merge), Cherry-pick

use gxm_core::forge::model::ReviewStatus;
use gxm_core::history::RefKind;
use gxm_core::ops::Op;
use rune::{El, Icon};

use super::bar;
use crate::icons;
use crate::keys;
use crate::state::{App, InspectorTab, Subject, View};
use crate::theme::Theme;
use crate::ui::{self, Variant};

pub fn toolbar(s: &App, t: &Theme) -> El {
    let mut bar = bar(t).px(16.0).key(keys::TOOLBAR);
    if !s.sidebar_open {
        // With the sidebar hidden, its toggle moves here, so it can come back, clear of any
        // traffic lights the sidebar's bar made room for (12 in, their width, then its 8 gap).
        let lights = super::sidebar::LIGHTS_W;
        bar = bar.pl(if lights > 0.0 { 12.0 + lights + 8.0 } else { 16.0 }).child(
            ui::button(t, Variant::Ghost, Some(&icons::SIDEBAR), "", None)
                .key(keys::SIDEBAR_TOGGLE)
                .help("Show the sidebar (⌃⌘S)")
                .on_press(|s: &mut App| s.sidebar_open = true),
        );
    }
    bar = bar.child(super::checkout::control(s, t));
    let b = Buttons { s, t };
    if s.pull_view().is_some() {
        return bar.children(b.pull()).child(switch(s, t));
    }
    let subject = if s.view == View::Work { None } else { s.subject() };
    let actions: Vec<El> = match subject {
        Some(Subject::Worktree) if s.all_staged() => {
            vec![b.commit(), b.op(Variant::Secondary, None, keys::UNSTAGE_ALL, Op::UnstageAll, None), b.discard()]
        }
        Some(Subject::Worktree) => {
            vec![b.op(Variant::Primary, Some(&icons::PLUS), keys::STAGE_ALL, Op::StageAll, None), b.discard()]
        }
        Some(Subject::Commit(id)) => vec![
            b.op(Variant::Secondary, None, keys::CHECKOUT, Op::Checkout(checkout_target(s, &id)), None),
            b.op(Variant::Secondary, None, keys::CHERRY_PICK, Op::CherryPick(vec![id.clone()]), None),
            b.op(Variant::Secondary, None, keys::REVERT, Op::Revert(id.clone()), None),
            b.soon(
                Variant::Secondary,
                Some(&icons::BRANCH),
                keys::BRANCH_FROM_HERE,
                "Branch from here",
                "Coming next: name the new branch",
            ),
            super::tag::button(s, t, &id),
        ],
        Some(Subject::Commits(ids)) => {
            let (bisect, squash) = (s.can_bisect(&ids), s.can_squash(&ids));
            let mut v = Vec::new();
            if bisect {
                v.push(b.soon(
                    Variant::Primary,
                    Some(&icons::SEARCH),
                    keys::BISECT,
                    "Bisect",
                    "Coming next: mark commits good or bad as you go",
                ));
            }
            if squash {
                let variant = if bisect { Variant::Secondary } else { Variant::Primary };
                v.push(b.soon(
                    variant,
                    Some(&icons::MERGE),
                    keys::SQUASH,
                    "Squash",
                    "Coming next: squash with a message you write",
                ));
            }
            // Oldest first, so they land in the order they were made.
            let oldest_first: Vec<String> = ids.iter().rev().cloned().collect();
            v.push(b.op(Variant::Secondary, None, keys::CHERRY_PICK, Op::CherryPick(oldest_first), None));
            v
        }
        Some(Subject::Head(_)) | None => {
            let st = s.status();
            let ahead = st.map_or(0, |st| st.ahead);
            let mut push = b.op(
                Variant::Primary,
                Some(&icons::PUSH),
                keys::PUSH,
                Op::Push,
                (ahead > 0).then(|| ahead.to_string()),
            );
            if let Some(branch) = st.filter(|st| st.upstream.is_none()).and_then(|st| st.branch.as_deref()) {
                push = push.help(format!("Publishes {branch} and tracks it from then on"));
            }
            vec![
                b.op(Variant::Secondary, Some(&icons::FETCH), keys::FETCH, Op::Fetch, None),
                b.op(Variant::Secondary, Some(&icons::PULL), keys::PULL, Op::Pull, None),
                push,
            ]
        }
    };
    let mut bar = bar.children(actions).child(rune::el().grow(1.0));
    if s.view == View::Work {
        bar = bar.child(b.soon(
            Variant::Secondary,
            Some(&icons::PLUS),
            keys::NEW_WORKTREE,
            "New worktree",
            "Coming next: start a worktree for new work",
        ));
    }
    bar.child(switch(s, t))
}

/// Work | History.
fn switch(s: &App, t: &Theme) -> El {
    let option = |label: &str, key: &str, view: View| {
        let on = s.view == view;
        (ui::segment_label(t, label, on).key(key).on_press(move |s: &mut App| s.show_view(view)), on)
    };
    ui::segmented(
        t,
        vec![option("Work", keys::VIEW_WORK, View::Work), option("History", keys::VIEW_HISTORY, View::History)],
    )
}

/// The branch whose tip a commit is, so Checkout lands on a branch rather than a detached HEAD
/// when it can; otherwise the commit itself.
fn checkout_target(s: &App, id: &str) -> String {
    s.commit(id)
        .and_then(|c| c.refs.iter().find(|r| matches!(r.kind, RefKind::LocalBranch | RefKind::Head)))
        .map_or_else(|| id.to_owned(), |r| r.name.clone())
}

struct Buttons<'a> {
    s: &'a App,
    t: &'a Theme,
}

impl Buttons<'_> {
    /// A button that runs `op`. While an operation runs, its button stays pressed and the
    /// others wait, dimmed as `.gu-btn:disabled`.
    fn op(&self, variant: Variant, glyph: Option<&'static Icon>, key: &str, op: Op, badge: Option<String>) -> El {
        let label = op.verb();
        self.op_as(variant, glyph, key, op, label, badge)
    }

    /// [`Buttons::op`] under a label of its own.
    fn op_as(
        &self,
        variant: Variant,
        glyph: Option<&'static Icon>,
        key: &str,
        op: Op,
        label: &str,
        badge: Option<String>,
    ) -> El {
        if self.s.repo().is_none() {
            // Nothing to act on yet: `.gu-btn:disabled`.
            return ui::button_in(self.t, variant, glyph, label, badge, false)
                .key(key)
                .disabled(true)
                .opacity(0.6)
                .help("Add a repository first");
        }
        let running = self.s.ops.running();
        let pressed = running.is_some_and(|r| r.verb() == op.verb());
        let b = ui::button_in(self.t, variant, glyph, label, badge, pressed).key(key);
        match running {
            Some(_) if pressed => b.disabled(true),
            Some(_) => b.disabled(true).opacity(0.6),
            None => b.on_press(move |s: &mut App| s.start(op.clone())),
        }
    }

    /// Commit: with a message, commits what's staged; without one, opens the message to write.
    fn commit(&self) -> El {
        if self.s.ops.running().is_some() {
            return self.op(Variant::Primary, Some(&icons::COMMIT), keys::COMMIT, Op::Commit(String::new()), None);
        }
        let b = ui::button(self.t, Variant::Primary, Some(&icons::COMMIT), "Commit", None).key(keys::COMMIT);
        b.on_press_tx(|s: &mut App, tx| {
            let message = s.message.text.trim().to_owned();
            if message.is_empty() {
                s.tab = InspectorTab::Overview;
                tx.focus(keys::COMMIT_MESSAGE, true);
            } else {
                s.start(Op::Commit(message));
            }
        })
    }

    /// Discard all, which sets the changes aside in a stash rather than deleting them.
    fn discard(&self) -> El {
        self.op(Variant::Secondary, None, keys::DISCARD_ALL, Op::DiscardAll, None)
            .help("Sets every change aside in a stash, so nothing is lost")
    }

    /// With a pull request open: Review (or Reviewed, once the person has), Check out its
    /// branch, Squash and merge (waiting), and Close pull request, back to the plain history.
    fn pull(&self) -> Vec<El> {
        let (s, t) = (self.s, self.t);
        let me = s.pulls.viewer().map(|u| u.login.clone()).unwrap_or_default();
        let reviewed = s.open_pull_full().and_then(|p| p.review_of(&me)).is_some_and(|r| r != ReviewStatus::Requested);
        let review = if reviewed {
            ui::button(t, Variant::Secondary, Some(&icons::CHECK), "Reviewed", None).help("Review it again")
        } else {
            ui::button(t, Variant::Primary, Some(&icons::EYE), "Review", None)
        };
        let head = s.open_pull_full().map(|p| p.summary.head_ref.clone());
        let check_out = match head {
            Some(branch) => {
                let op = Op::Checkout(branch);
                self.op_as(Variant::Secondary, Some(&icons::BRANCH), keys::CHECK_OUT_PULL, op, "Check out", None)
            }
            None => ui::button_disabled(t, Some(&icons::BRANCH), "Check out").key(keys::CHECK_OUT_PULL),
        };
        let squash = match s.open_pull_full() {
            Some(p) if super::pull_parts::ready_to_merge(p) => {
                ui::button(t, Variant::Secondary, Some(&icons::MERGE), "Squash and merge", None)
                    .on_press(|s: &mut App| s.start_merge())
            }
            Some(p) => ui::button_disabled(t, Some(&icons::MERGE), "Squash and merge")
                .help(super::pull_parts::why_not_merge(p)),
            None => ui::button_disabled(t, Some(&icons::MERGE), "Squash and merge"),
        };
        vec![
            review.key(keys::REVIEW).on_press(|s: &mut App| s.open_cockpit()),
            check_out,
            squash.key(keys::SQUASH_AND_MERGE),
            rune::el().grow(1.0),
            ui::button(t, Variant::Ghost, Some(&icons::X), "Close pull request", None)
                .key(keys::CLOSE_PULL_REQUEST)
                .help("Back to the history (the pull request stays open)")
                .on_press(|s: &mut App| s.close_pull()),
        ]
    }

    /// A control whose workflow isn't built yet: disabled, saying what's next.
    fn soon(&self, variant: Variant, glyph: Option<&'static Icon>, key: &str, label: &str, next: &str) -> El {
        ui::button(self.t, variant, glyph, label, None).key(key).help(next.to_owned()).disabled(true)
    }
}
