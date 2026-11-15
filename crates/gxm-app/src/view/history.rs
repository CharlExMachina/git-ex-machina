//! The history pane: the action bar, the branch summary, the column header and the commit rows
//! (`CommitGraphRow`), with the uncommitted-changes row on top.

use gxm_core::history::{Commit, RefKind};
use gxm_core::time;
use rune::{Color, El, GrowthSpec, ScrollSpec, col, icon, row, text};

use super::graph::{self, Node, RowLook};
use crate::icons;
use crate::keys;
use crate::ops::{OpState, Ops};
use crate::state::{App, History, Pick, Row};
use crate::theme::{Theme, size, ty};
use crate::ui;

pub fn pane(s: &App, t: &Theme) -> El {
    let body = match &s.history {
        // Still reading: a blank canvas, not a claim that there's nothing to show.
        None => col().w_full().grow(1.0),
        Some(History { error: Some(e), .. }) => empty(t, e),
        Some(h) => {
            let top = match s.pull_view() {
                Some(v) => super::on_graph::summary(s, v, t),
                None => summary(s, h, t),
            };
            col().w_full().grow(1.0).min_h(0.0).child(top).child(header(t)).child(rows(h, t))
        }
    };
    col().fill(t.canvas).child(super::toolbar::toolbar(s, t)).child(body)
}

fn empty(t: &Theme, message: &str) -> El {
    col()
        .key(keys::EMPTY_STATE)
        .w_full()
        .grow(1.0)
        .center()
        .child(text(message.to_owned()).font(ty::BODY).color(t.ink_muted))
}

/// `⑂ main ↑2 · 1,284 commits · fetched 2 minutes ago`, the multi-select hint and commit search.
fn summary(s: &App, h: &History, t: &Theme) -> El {
    let status = s.repo().map(|r| &r.summary.status);
    let branch = status.and_then(|st| st.branch.clone()).unwrap_or_else(|| "detached HEAD".to_owned());
    let mut line = row()
        .key(keys::BRANCH_SUMMARY)
        .gap(6.0)
        .center_y()
        .child(icon(&icons::BRANCH, 12.0).color(t.ink_muted))
        .child(text(branch).font(ty::CAPTION_STRONG).color(t.ink));
    if let Some(st) = status {
        if st.ahead > 0 {
            line = line.child(text(format!("↑{}", st.ahead)).font(ty::MONO_SM).color(t.ink_muted));
        }
        if st.behind > 0 {
            line = line.child(text(format!("↓{}", st.behind)).font(ty::MONO_SM).color(t.ink_muted));
        }
        // A branch only this computer has: Push publishes it.
        let has_remote = s.repo().is_some_and(|r| r.summary.origin.is_some());
        if st.branch.is_some() && st.upstream.is_none() && has_remote {
            line = line.child(text("not pushed yet").font(ty::CAPTION).color(t.accent));
        }
    }
    // The count lands after the history on a big repository; until then the line goes without.
    let count = h.total.map(|n| if n == 1 { "1 commit".to_owned() } else { format!("{} commits", thousands(n)) });
    let fetched = s
        .repo()
        .and_then(|r| r.summary.fetched)
        .map(|at| format!("fetched {}", time::long(at, s.env.now(), &s.env.tz)));
    let rest: String = count.into_iter().chain(fetched).map(|part| format!("· {part} ")).collect();
    line = line.child(text(rest.trim_end().to_owned()).font(ty::CAPTION).color(t.ink_muted));
    if let OpState::Failed { op, message } = &s.ops.state {
        line = line.child(
            text(format!("· {} failed: {}", op.verb(), Ops::reason(message)))
                .key(keys::OP_ERROR)
                .font(ty::CAPTION)
                .color(t.removed)
                .truncate()
                .help(message.clone()),
        );
    }
    let search = row().w(220.0).child(
        ui::search_field(
            t,
            &s.search,
            "Search commits",
            "⌘F",
            keys::COMMIT_SEARCH,
            |s: &mut App| &mut s.search,
            |f| {
                f.on_input_change(|s: &mut App, tx| reveal(s.find(false), tx))
                    .on_submit(|s: &mut App, tx| reveal(s.find(true), tx))
            },
        )
        .grow(1.0),
    );
    row()
        .w_full()
        .pt(10.0)
        .pb(10.0)
        .pl(24.0)
        .pr(16.0)
        .gap(12.0)
        .center_y()
        .justify_between()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout()
        .child(line)
        .child(
            row()
                .gap(12.0)
                .center_y()
                .child(text("⌘-click or ⇧-click to select several").font(ty::HINT).color(t.ink_faint))
                .child(search),
        )
}

/// Scrolls the history to a row the search found.
fn reveal(found: Option<String>, tx: &mut rune::Tx) {
    if let Some(id) = found {
        tx.scroll_into_view_in(keys::HISTORY, id);
    }
}

const AUTHOR_W: f32 = 128.0;
const DATE_W: f32 = 64.0;
const SHA_W: f32 = 60.0;

/// The column header: 24 tall, inset 8 a side, text from 58 in (where the subjects start on
/// the uncommitted row), 12 from the right.
fn header(t: &Theme) -> El {
    let cell = |s: &str, w: Option<f32>, right: bool| {
        let mut c = row().child(text(s.to_owned()).font(ty::HEADER).color(t.ink_faint));
        c = match w {
            Some(w) => c.w(w),
            None => c.grow(1.0),
        };
        if right { c.justify_end() } else { c }
    };
    row()
        .h(25.0)
        .mx(8.0)
        .pl(58.0)
        .pr(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout()
        .child(cell("Description", None, false))
        .child(cell("Author", Some(AUTHOR_W), false))
        .child(cell("Date", Some(DATE_W), true))
        .child(cell("Commit", Some(SHA_W), true))
}

fn rows(h: &History, t: &Theme) -> El {
    let theme = *t;
    let list = col().key(keys::HISTORY).w_full().growth(GrowthSpec::default()).virtual_rows(
        h.rows.len(),
        size::ROW_COMMIT,
        |s: &App, i| s.history.as_ref().and_then(|h| h.rows.get(i)).map_or("".into(), |r| r.id().into()),
        move |s: &App, i| history_row(s, i, &theme),
    );
    // The padding sits inside the scroll viewport, which clips: a selected row's outline and
    // shadow reach past the row, and need that room to draw.
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(col().w_full().pt(6.0).px(8.0).child(list))
}

fn history_row(s: &App, i: usize, t: &Theme) -> El {
    let Some(h) = &s.history else { return rune::el() };
    let (Some(r), Some(g)) = (h.rows.get(i), h.graph.get(i)) else { return rune::el() };
    let id = r.id().to_owned();
    let selected = s.is_selected(&id);
    let lane = graph::color(t, g.color);
    // With a pull request open, what isn't its dims: the content as a whole, the graph part by
    // part, so the pull request's lane runs on bright through a dimmed row.
    let light = h.lit.as_ref().and_then(|l| l.get(i));
    let dim = light.is_some_and(|l| !l.node);
    // The graph sits beside the row's content, never under its highlight, so no rail is ever
    // covered or cut by a selection.
    let (graph_el, content) = match r {
        Row::Worktree { changed } => {
            // There's room here to say it in words; everything staged also fills the mark.
            let staged = s.all_staged();
            let files = if *changed == 1 { "1 file".to_owned() } else { format!("{changed} files") };
            let note = format!("{files} {}", if staged { "staged" } else { "changed" });
            let c = row()
                .gap(6.0)
                .child(row().grow(1.0).child(text("Uncommitted changes").font(ty::BODY_STRONG).color(t.ink)))
                .child(text(note).font(ty::CAPTION).color(t.ink_muted));
            let mark = graph::worktree(t, staged, lane, h.cols);
            (if dim { mark.opacity(graph::DIM) } else { mark }, c)
        }
        Row::Commit(c) => {
            let node = if c.refs.iter().any(|r| r.kind == RefKind::Head) {
                Node::Head
            } else if c.is_merge() {
                Node::Merge
            } else {
                Node::Commit
            };
            let under_worktree = i > 0 && matches!(h.rows[i - 1], Row::Worktree { .. });
            let look = RowLook { node, under_worktree, selected, light };
            (graph::row(t, g, h.cols, &look), commit_content(s, c, lane, selected, t))
        }
    };
    let content = if dim { content.opacity(graph::DIM) } else { content };
    let mut card = content.grow(1.0).min_w(0.0).h(size::ROW_COMMIT).pl(6.0).pr(12.0).center_y().radius(size::RADIUS_SM);
    // The pull request's row sits above its newest commit, in the same virtual row.
    let banner = (s.pull_tip() == Some(i)).then(|| super::on_graph::banner(s, g, h.cols, light, t));
    let mut el = row().h(size::ROW_COMMIT).center_y();
    if banner.is_none() {
        el = el.key(id.clone());
    }
    let key = id.clone();
    if selected {
        // Neighbouring selected rows merge into one block: the first keeps its top, a middle
        // row only its sides, the last its bottom and drop. A row with a selected row below
        // paints over it, so the drop beneath can't cut across the block.
        let picked = |k: Option<usize>| k.and_then(|k| h.rows.get(k)).is_some_and(|r| s.is_selected(r.id()));
        let (above, below) = (picked(i.checked_sub(1)), picked(Some(i + 1)));
        let r = size::RADIUS_SM;
        let top = if above { 0.0 } else { r };
        let bottom = if below { 0.0 } else { r };
        card = card.fill(t.raised).corners([top, top, bottom, bottom]);
        let shadows: Vec<rune::Shadow> = match (above, below) {
            (false, false) => t.shadow_lifted().to_vec(),
            (false, true) => t.shadow_lifted_first().to_vec(),
            (true, true) => t.shadow_lifted_middle().to_vec(),
            (true, false) => t.shadow_lifted_last().to_vec(),
        };
        for sh in shadows {
            card = card.box_shadow(sh);
        }
        el = el.paint_order(if below { 2 } else { 1 });
    } else {
        card = card.hover(t.control_hover);
    }
    let el = el.child(graph_el).child(card).on_press_tx(move |s: &mut App, tx| {
        let m = tx.mods();
        let how = if m.shift {
            Pick::Range
        } else if m.cmd {
            Pick::Toggle
        } else {
            Pick::Plain
        };
        s.pick(&id, how);
    });
    match banner {
        Some(b) => col().key(key).w_full().child(b).child(el),
        None => el,
    }
}

/// A commit's row content: refs and subject, author, date, short id.
fn commit_content(s: &App, c: &Commit, lane: Color, selected: bool, t: &Theme) -> El {
    let mut body = row().grow(1.0).min_w(0.0).gap(6.0).center_y();
    for label in &c.refs {
        body = body.child(ref_badge(t, label.kind, &label.name, lane));
    }
    let (subject_ty, subject_ink) = if c.is_merge() { (ty::BODY, t.ink_muted) } else { (ty::BODY_STRONG, t.ink) };
    body = body.child(text(c.subject.clone()).font(subject_ty).color(subject_ink).truncate());
    let author = row()
        .w(AUTHOR_W)
        .gap(6.0)
        .center_y()
        .child(ui::avatar(t, &c.author, author_color(t, &c.email)))
        .child(text(c.author.clone()).font(ty::CAPTION).color(t.ink_muted).truncate());
    let date = row().w(DATE_W).justify_end().child(
        text(time::relative(c.time, s.env.now(), &s.env.tz))
            .font(ty::CAPTION)
            .color(t.ink_muted)
            .help(time::absolute(c.time, &s.env.tz)),
    );
    let sha_ink = if selected { t.ink_muted } else { t.ink_faint };
    let sha = row().w(SHA_W).justify_end().child(text(c.short_id().to_owned()).font(ty::MONO_11).color(sha_ink));
    row().child(body).child(author).child(date).child(sha)
}

/// A person's colour: a lane chosen by a hash of their email (FNV-1a), so a name keeps its
/// colour across repositories and runs.
pub fn author_color(t: &Theme, email: &str) -> Color {
    let h = email.to_lowercase().bytes().fold(0x811c_9dc5_u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    t.lane(h as usize % 6 + 1)
}

/// `RefBadge`: a 20-tall pill, 6 in on the left and 8 on the right, 11.5px medium.
pub fn ref_badge(t: &Theme, kind: RefKind, name: &str, lane: Color) -> El {
    let pill = row().h(20.0).gap(5.0).center_y().radius(size::PILL);
    match kind {
        RefKind::Tag => pill
            .pl(7.0)
            .pr(8.0)
            .box_shadow(Theme::ring(1.0, t.line))
            .child(icon(&icons::TAG, 12.0).color(t.ink_muted))
            .child(text(name.to_owned()).font(ty::REF).color(t.ink_muted)),
        RefKind::RemoteBranch => {
            let (prefix, rest) = name.split_once('/').map_or(("", name), |(a, b)| (a, b));
            let mut label = row();
            if !prefix.is_empty() {
                label = label.child(text(format!("{prefix}/")).font(ty::REF_PREFIX).color(t.ink_muted));
            }
            pill.pl(6.0)
                .pr(8.0)
                .box_shadow(Theme::ring(1.0, t.line))
                .child(rune::el().size(7.0, 7.0).radius(3.5).box_shadow(Theme::ring(1.5, lane)))
                .child(label.child(text(rest.to_owned()).font(ty::REF).color(t.ink_muted)))
        }
        RefKind::Head => pill
            .pl(6.0)
            .pr(8.0)
            .fill(t.accent_soft)
            .child(
                // `.gu-ref-head .gu-ref-dot`: 5 across, margin 0 2 0 1, ringed by 2 of
                // accent-soft then 1 of accent (`0 0 0 2px`, `0 0 0 3px`).
                rune::el()
                    .size(5.0, 5.0)
                    .ml(1.0)
                    .mr(2.0)
                    .radius(2.5)
                    .fill(t.accent)
                    // Rune paints shadows in the order given, later ones on top: the reverse of
                    // CSS, so the 3-point accent ring goes first and the gap over it.
                    .box_shadow(rune::Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 3.0, color: t.accent, inset: false })
                    .box_shadow(rune::Shadow {
                        x: 0.0,
                        y: 0.0,
                        blur: 0.0,
                        spread: 2.0,
                        color: t.accent_soft,
                        inset: false,
                    }),
            )
            .child(text(name.to_owned()).font(ty::REF).color(t.ink)),
        RefKind::LocalBranch => pill
            .pl(6.0)
            .pr(8.0)
            .fill(t.raised)
            .box_shadow(Theme::ring(1.0, t.line))
            .child(rune::el().size(7.0, 7.0).radius(3.5).fill(lane))
            .child(text(name.to_owned()).font(ty::REF).color(t.ink)),
    }
}

/// `1284` → `1,284`.
fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
