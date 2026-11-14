//! The review cockpit's sides (`design/pr-flow/flow3`): on the left the pull request's files
//! (with how many are viewed) and commits, and the way back; on the right the review: what the
//! pull request is, its checks and reviewers, and the verdict to send.

use gxm_core::forge::model::{CheckState, Verdict};
use gxm_core::history::ChangeStatus;
use rune::{El, ScrollSpec, col, icon, row, text, text_input};

use super::{bar, pull_parts};
use crate::icons;
use crate::keys;
use crate::pulls::Sending;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// The sidebar's width inside its 16-point padding.
const INNER_W: f32 = 248.0 - 32.0;

pub fn sidebar(s: &App, t: &Theme) -> El {
    let top = bar(t).px(12.0).child(rune::el().w(super::sidebar::LIGHTS_W).h(12.0)).child(rune::el().grow(1.0)).child(
        ui::button(t, Variant::Ghost, Some(&icons::SIDEBAR), "", None)
            .key(keys::SIDEBAR_TOGGLE)
            .help("Hide the sidebar (⌃⌘S)")
            .on_press(|s: &mut App| s.sidebar_open = false),
    );
    let Some(v) = s.pull_view() else { return col() };
    let back = row().pt(14.0).px(16.0).pb(10.0).child(
        ui::button(t, Variant::Ghost, Some(&icons::BACK), &format!("Pull request #{}", v.number), None)
            .key(keys::COCKPIT_BACK)
            .on_press(|s: &mut App| s.close_cockpit()),
    );
    let mut list = col().w_full().grow(1.0).min_h(0.0).child(top).child(back);
    if let Some(p) = s.open_pull_full() {
        list = list.child(
            col()
                .px(16.0)
                .pb(14.0)
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(ui::state_pill(t, p.summary.state, t.chrome))
                        .child(text(format!("#{}", p.summary.number)).font(ty::MONO_SM).color(t.ink_faint)),
                )
                .child(text(p.summary.title.clone()).font(ty::BODY_SEMI).color(t.ink).wrap().mt(8.0))
                .child(
                    text(format!("{} → {}", p.summary.head_ref, p.summary.base_ref))
                        .font(ty::CAPTION)
                        .color(t.ink_muted)
                        .mt(4.0),
                ),
        );
    }
    let mut files = col().key(keys::COCKPIT_FILES).w_full();
    if let Some(changes) = s.pull_changes() {
        let total = changes.files.len();
        let viewed = changes.files.iter().filter(|f| v.draft.is_viewed(&f.path)).count();
        files = files
            .child(
                row()
                    .w_full()
                    .px(16.0)
                    .pb(8.0)
                    .justify_between()
                    .child(ui::label(t, "Files"))
                    .child(text(format!("{viewed} of {total} viewed")).font(ty::MONO_SM).color(t.ink_muted)),
            )
            .child(row().px(16.0).pb(12.0).child({
                let done = if total == 0 { 0.0 } else { INNER_W * viewed as f32 / total as f32 };
                row()
                    .w(INNER_W)
                    .h(4.0)
                    .radius(2.0)
                    .fill(t.well)
                    .child(rune::el().w(done).h(4.0).radius(2.0).fill(t.added))
            }));
        let shown = s.cockpit_file();
        let mut rows = col().px(8.0);
        for f in &changes.files {
            let talk: usize = s
                .open_pull_full()
                .map_or(0, |p| p.threads.iter().filter(|th| th.path == f.path).map(|th| th.comments.len()).sum());
            rows = rows.child(file_row(f, talk, v.draft.is_viewed(&f.path), shown == Some(f.path.as_str()), t));
        }
        files = files.child(rows).child(
            row()
                .w_full()
                .pt(16.0)
                .px(16.0)
                .pb(8.0)
                .justify_between()
                .child(ui::label(t, "Commits"))
                .child(text("all").font(ty::MONO_SM).color(t.ink_muted)),
        );
        let mut commits = col().px(16.0).gap(6.0);
        for (id, subject) in &changes.commits {
            commits = commits.child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(text(id.get(..7).unwrap_or(id).to_owned()).font(ty::MONO_SM).color(t.ink_faint))
                    .child(text(subject.clone()).font(ty::CAPTION).color(t.ink_muted).truncate().grow(1.0).min_w(0.0)),
            );
        }
        files = files.child(commits);
    }
    list = list.child(files);
    let footer = row()
        .w_full()
        .padding(12.0)
        .gap(8.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], t.line)
        .border_layout()
        .child(ui::kbd(t, "j"))
        .child(ui::kbd(t, "k"))
        .child(text("next / previous change").font(ty::HINT).color(t.ink_faint));
    col().fill(t.chrome).border_sides([0.0, 1.0, 0.0, 0.0], t.line).border_layout().child(list).child(footer)
}

/// `.gu-file` with the name alone, and on the right its comment count or a viewed check.
fn file_row(f: &gxm_core::history::FileChange, talk: usize, viewed: bool, selected: bool, t: &Theme) -> El {
    let letter = match f.status {
        ChangeStatus::Copied => 'A',
        ChangeStatus::TypeChanged => 'M',
        other => other.letter(),
    };
    let name = f.path.rsplit('/').next().unwrap_or(&f.path).to_owned();
    let mut side = row().gap(6.0).center_y();
    if talk > 0 {
        side = side.child(
            row()
                .gap(4.0)
                .center_y()
                .child(icon(&icons::COMMENT, 12.0).color(t.ink_muted))
                .child(text(talk.to_string()).font(ty::SMALL).color(t.ink_muted)),
        );
    }
    if viewed {
        side = side.child(icon(&icons::CHECK, 13.0).color(t.added));
    }
    let path = f.path.clone();
    let mut el = row()
        .key(keys::pull_file(&f.path))
        .w_full()
        .h(size::ROW_FILE)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(size::RADIUS_SM)
        .help(f.path.clone())
        .child(ui::status(t, letter))
        .child(text(name).font(ty::BODY_STRONG).color(t.ink).truncate().grow(1.0).min_w(0.0))
        .child(side)
        .on_press(move |s: &mut App| s.select_pull_file(&path));
    if selected {
        el = el.fill(t.raised);
        for sh in t.shadow_lifted() {
            el = el.box_shadow(sh);
        }
        el
    } else {
        el.hover(t.control_hover)
    }
}

pub fn review_panel(s: &App, t: &Theme) -> El {
    let url = s.open_pull_full().map(|p| p.summary.url.clone());
    let mut copy = ui::button(t, Variant::Ghost, Some(&icons::COPY), "", None).key(keys::COPY_ID);
    copy = match url {
        Some(url) => copy.help("Copy the pull request's link").on_press_tx(move |_: &mut App, tx| tx.copy(url.clone())),
        None => copy.disabled(true).opacity(0.6),
    };
    let top = bar(t).pl(16.0).pr(12.0).child(ui::label(t, "Review")).child(rune::el().grow(1.0)).child(copy);
    let mut body = col().key(keys::REVIEW_PANEL).w_full().padding(20.0).gap(18.0);
    if let Some(p) = s.open_pull_full() {
        let first = p.body.split("\n\n").next().unwrap_or_default().trim().to_owned();
        let (first, spans) = pull_parts::inline(t, &first, false);
        body = body.child(
            col()
                .w_full()
                .gap(6.0)
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(pull_parts::person(s, t, &p.summary.author, ty::BODY_STRONG))
                        .child(
                            text(format!("opened {}", gxm_core::time::ago(p.created, s.env.now())))
                                .font(ty::CAPTION)
                                .color(t.ink_muted),
                        ),
                )
                .child(rune::rich(first, spans).font(ty::DESCRIPTION).color(t.ink_muted).w_full()),
        );
        if !p.checks.is_empty() {
            let passed = p.checks.iter().filter(|c| c.state == CheckState::Passed).count();
            let (glyph, color, what) = match p.failing().next() {
                Some(c) => (&icons::X, t.removed, format!("{} failed", c.name)),
                None if passed == p.checks.len() => (&icons::CHECK, t.added, "All checks passed".to_owned()),
                None => (&icons::CLOCK, t.accent, "Checks are running".to_owned()),
            };
            body = body.child(
                row()
                    .radius(size::RADIUS_LG)
                    .fill(t.raised)
                    .box_shadow(Theme::ring(1.0, t.line))
                    .w_full()
                    .px(12.0)
                    .py(10.0)
                    .gap(10.0)
                    .center_y()
                    .child(icon(glyph, 14.0).color(color))
                    .child(text(what).font(ty::CAPTION).color(t.ink).truncate().grow(1.0).min_w(0.0))
                    .child(text(format!("{passed}/{}", p.checks.len())).font(ty::MONO_SM).color(t.ink_faint)),
            );
        }
        body = body.child(pull_parts::reviewers(s, p, t));
    }
    body = body.child(your_review(s, t));
    col()
        .fill(t.raised)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.line)
        .border_layout()
        .child(top)
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}

/// The summary, the verdict and Submit review.
fn your_review(s: &App, t: &Theme) -> El {
    let Some(v) = s.pull_view() else { return col() };
    let pending = v.draft.pending();
    let mut hint = "Summarise your review…".to_owned();
    if pending > 0 {
        let what = pull_parts::plural(pending, "pending comment");
        hint.push_str(&format!(" ({what} will be sent with it)"));
    }
    // The hint is a text of its own under a clear field, so it's read like any other text.
    let mut well = ui::well(t).key(keys::REVIEW_SUMMARY).w_full().min_h(88.0).px(12.0).py(10.0);
    if v.summary.text.is_empty() {
        well = well.child(text(hint).font(ty::BODY).color(t.ink_faint).wrap().absolute(12.0, 10.0).w(INNER_W + 56.0));
    }
    well = well.child(
        text_input(&v.summary, ty::BODY).on_edit(|s: &mut App| s.summary_edit()).color(t.ink).w_full().min_h(68.0),
    );
    let verdict = v.draft.verdict;
    let choice = |label: &str, key: &str, which: Verdict| {
        let on = verdict == which;
        let ink = match (on, which) {
            (true, Verdict::Approve) => t.added,
            (true, Verdict::RequestChanges) => t.removed,
            (true, Verdict::Comment) => t.ink,
            (false, _) => t.ink_muted,
        };
        let mut l = row().gap(6.0).center_y();
        if on && which != Verdict::Comment {
            l = l.child(icon(if which == Verdict::Approve { &icons::CHECK } else { &icons::X }, 13.0).color(ink));
        }
        let l = l.child(text(label.to_owned()).font(ty::CONTROL).color(ink));
        (l.key(key).on_press(move |s: &mut App| s.set_verdict(which)), on)
    };
    let sending = matches!(s.pulls.machine.sending, Sending::Sending { .. } | Sending::Refreshing { .. });
    let submit = ui::button_in(t, Variant::Primary, None, "Submit review", None, sending).key(keys::SUBMIT_REVIEW);
    let submit = if sending { submit.disabled(true) } else { submit.on_press(|s: &mut App| s.submit_review()) };
    let mut c = col()
        .w_full()
        .gap(10.0)
        .child(ui::label(t, "Your review"))
        .child(well)
        .child(row().child(ui::segmented(
            t,
            vec![
                choice("Comment", keys::VERDICT_COMMENT, Verdict::Comment),
                choice("Approve", keys::VERDICT_APPROVE, Verdict::Approve),
                choice("Request changes", keys::VERDICT_REQUEST_CHANGES, Verdict::RequestChanges),
            ],
        )))
        .child(row().gap(8.0).center_y().child(submit).child(text("⌘↩").font(ty::HINT).color(t.ink_faint)));
    if let Sending::Failed { number, message } = &s.pulls.machine.sending
        && *number == v.number
    {
        c = c.child(text(format!("Not sent: {message}")).font(ty::CAPTION).color(t.removed).wrap());
    }
    c
}
