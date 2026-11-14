//! The action bar over the history (`role="toolbar"` in the design): what it offers follows
//! the selection, as the design's toolbar states do.
//!
//! - nothing selected: Fetch, Pull, Push
//! - the uncommitted row: Stage all, Discard all; with everything staged, Commit, Unstage all,
//!   Discard all
//! - one commit: Checkout, Cherry-pick, Revert, Branch from here
//! - several: Bisect (a line of three or more), Squash (consecutive, no merge), Cherry-pick

use gxm_core::history::RefKind;
use gxm_core::ops::Op;
use rune::{El, Icon};

use super::bar;
use crate::icons;
use crate::keys;
use crate::state::{App, InspectorTab, Subject};
use crate::theme::Theme;
use crate::ui::{self, Variant};

pub fn toolbar(s: &App, t: &Theme) -> El {
    let mut bar = bar(t).px(16.0);
    if !s.sidebar_open {
        // With the sidebar hidden, its toggle moves here, so it can come back, clear of the
        // traffic lights the sidebar's bar made room for (12 in, 52 wide, then its 8 gap).
        bar = bar.pl(12.0 + super::sidebar::LIGHTS_W + 8.0).child(
            ui::button(t, Variant::Ghost, Some(&icons::SIDEBAR), "", None)
                .key(keys::SIDEBAR_TOGGLE)
                .help("Show the sidebar (⌃⌘S)")
                .on_press(|s: &mut App| s.sidebar_open = true),
        );
    }
    let b = Buttons { s, t };
    let actions: Vec<El> = match s.subject() {
        Some(Subject::Worktree) if s.all_staged() => {
            vec![b.commit(), b.op(Variant::Secondary, None, keys::UNSTAGE_ALL, Op::UnstageAll, None), b.discard()]
        }
        Some(Subject::Worktree) => {
            vec![b.op(Variant::Primary, Some(&icons::PLUS), keys::STAGE_ALL, Op::StageAll, None), b.discard()]
        }
        Some(Subject::Commit(id)) => vec![
            b.op(Variant::Secondary, None, keys::CHECKOUT, Op::Checkout(checkout_target(s, &id)), None),
            b.op(Variant::Secondary, None, keys::CHERRY_PICK, Op::CherryPick(vec![id.clone()]), None),
            b.op(Variant::Secondary, None, keys::REVERT, Op::Revert(id), None),
            b.soon(
                Variant::Secondary,
                Some(&icons::BRANCH),
                keys::BRANCH_FROM_HERE,
                "Branch from here",
                "Coming next: name the new branch",
            ),
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
            let ahead = s.repo().map_or(0, |r| r.summary.status.ahead);
            vec![
                b.op(Variant::Secondary, Some(&icons::FETCH), keys::FETCH, Op::Fetch, None),
                b.op(Variant::Secondary, Some(&icons::PULL), keys::PULL, Op::Pull, None),
                b.op(
                    Variant::Primary,
                    Some(&icons::PUSH),
                    keys::PUSH,
                    Op::Push,
                    (ahead > 0).then(|| ahead.to_string()),
                ),
            ]
        }
    };
    bar.children(actions)
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
        if self.s.repo().is_none() {
            // Nothing to act on yet: `.gu-btn:disabled`.
            return ui::button_in(self.t, variant, glyph, op.verb(), badge, false)
                .key(key)
                .disabled(true)
                .opacity(0.6)
                .help("Add a repository first");
        }
        let running = self.s.ops.running();
        let pressed = running.is_some_and(|r| r.verb() == op.verb());
        let b = ui::button_in(self.t, variant, glyph, op.verb(), badge, pressed).key(key);
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
                s.tab = InspectorTab::Message;
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

    /// A control the design shows whose workflow isn't built yet: disabled, saying what's next.
    fn soon(&self, variant: Variant, glyph: Option<&'static Icon>, key: &str, label: &str, next: &str) -> El {
        ui::button(self.t, variant, glyph, label, None).key(key).help(next.to_owned()).disabled(true)
    }
}
