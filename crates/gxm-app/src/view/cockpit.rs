//! The review cockpit's middle (`design/pr-flow/flow3`): Unified or Split, the file's header
//! with Viewed, and its diff with the threads, replies and draft comments inline. A line takes a
//! comment when pressed; Return keeps it as a draft.

use gxm_core::diff::{Hunk, HunkLine, LineKind};
use gxm_core::forge::model::{Side, Thread, User};
use gxm_core::history::ChangeStatus;
use gxm_core::ops::Op;
use gxm_core::time;
use rune::text::Span;
use rune::{El, ScrollSpec, col, rich, row, text, text_input};

use super::{bar, pull_parts};
use crate::icons;
use crate::keys;
use crate::review::{Composer, Target};
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

pub fn pane(s: &App, t: &Theme) -> El {
    col().fill(t.canvas).child(top(s, t)).child(file_header(s, t)).child(diff(s, t))
}

/// Unified or Split, how much is being discussed, Check out and Open on GitHub.
fn top(s: &App, t: &Theme) -> El {
    let split = s.cockpit().is_some_and(|c| c.split);
    let opt = |label: &str, key: &str, on: bool, value: bool| {
        (ui::segment_label(t, label, on).key(key).on_press(move |s: &mut App| s.set_split(value)), on)
    };
    let mut b = bar(t).px(16.0);
    if !s.sidebar_open {
        b = b.pl(12.0 + super::sidebar::LIGHTS_W + 8.0).child(
            ui::button(t, Variant::Ghost, Some(&icons::SIDEBAR), "", None)
                .key(keys::SIDEBAR_TOGGLE)
                .help("Show the sidebar (⌃⌘S)")
                .on_press(|s: &mut App| s.sidebar_open = true),
        );
    }
    b = b.child(ui::segmented(
        t,
        vec![opt("Unified", keys::DIFF_UNIFIED, !split, false), opt("Split", keys::DIFF_SPLIT, split, true)],
    ));
    b = b.child(rune::el().grow(1.0));
    if let Some(p) = s.open_pull_full() {
        let comments: usize = p.threads.iter().map(|th| th.comments.len()).sum();
        let talk = format!("{} · {} unresolved", pull_parts::plural(comments, "comment"), p.unresolved());
        b = b.child(text(talk).font(ty::HINT).color(t.ink_faint));
        let branch = p.summary.head_ref.clone();
        let busy = s.ops.running().is_some();
        let check_out =
            ui::button(t, Variant::Secondary, Some(&icons::BRANCH), "Check out", None).key(keys::CHECK_OUT_PULL);
        b = b.child(if busy {
            check_out.disabled(true).opacity(0.6)
        } else {
            check_out.on_press(move |s: &mut App| s.start(Op::Checkout(branch.clone())))
        });
        let url = p.summary.url.clone();
        b = b.child(
            ui::button(t, Variant::Ghost, Some(&icons::EXTERNAL), "Open on GitHub", None)
                .key(keys::OPEN_ON_PROVIDER)
                .on_press_tx(move |_: &mut App, tx| {
                    tx.open_url(&url).ok();
                }),
        );
    }
    b
}

/// The file shown: its status, path and line counts, and Viewed.
fn file_header(s: &App, t: &Theme) -> El {
    let changes = s.shown_changes();
    let file = s.cockpit_file().and_then(|p| changes?.files.iter().find(|f| f.path == p));
    let mut h = row()
        .key(keys::FILE_HEADER)
        .w_full()
        .px(16.0)
        .py(10.0)
        .gap(8.0)
        .center_y()
        .fill(t.chrome)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout();
    let Some(f) = file else { return h.child(rune::el().h(18.0)) };
    let letter = match f.status {
        ChangeStatus::Copied => 'A',
        ChangeStatus::TypeChanged => 'M',
        other => other.letter(),
    };
    let dir = f.path.rfind('/').map_or(0, |i| i + 1);
    let path = rich(f.path.clone(), vec![Span::new(0, dir).color(t.ink_muted)]).font(ty::MONO).color(t.ink).truncate();
    let mut stat = row().ml(6.0).gap(6.0).center_y();
    if let Some(a) = f.added.filter(|a| *a > 0) {
        stat = stat.child(text(format!("+{a}")).font(ty::MONO_11).color(t.added));
    }
    if let Some(r) = f.removed.filter(|r| *r > 0) {
        stat = stat.child(text(format!("−{r}")).font(ty::MONO_11).color(t.removed));
    }
    let viewed = s.pull_view().is_some_and(|v| v.draft.is_viewed(&f.path));
    h = h.child(ui::status(t, letter)).child(path.min_w(0.0)).child(stat).child(rune::el().grow(1.0));
    h.child(
        row()
            .key(keys::VIEWED)
            .gap(6.0)
            .center_y()
            .role("checkbox")
            .checked(viewed)
            .child(ui::checkbox(t, viewed))
            .child(text("Viewed").font(ty::CAPTION).color(t.ink_muted))
            .on_press(|s: &mut App| s.toggle_viewed()),
    )
}

fn diff(s: &App, t: &Theme) -> El {
    let note = |msg: String, color| col().padding(20.0).child(text(msg).font(ty::BODY).color(color).wrap());
    // One commit alone: its own diff, read only (comments belong to the pull request's lines).
    if let Some((_, changes)) = s.cockpit().and_then(|c| c.commit.as_ref()) {
        let Some(changes) = changes else {
            return note("Reading the commit…".to_owned(), t.ink_faint);
        };
        let body = match s.cockpit_file() {
            None => note("This commit changes no files.".to_owned(), t.ink_faint),
            Some(path) => file_diff(s, path, changes.diffs.get(path).map(Vec::as_slice).unwrap_or_default(), false, t),
        };
        return col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body);
    }
    let body = match s.pull_details().map(|d| &d.changes) {
        None | Some(None) => note("Reading the diff…".to_owned(), t.ink_faint),
        Some(Some(Err(e))) => note(format!("Git couldn't show this pull request's changes: {e}"), t.removed),
        Some(Some(Ok(changes))) => match s.cockpit_file() {
            None => note("This pull request changes no files.".to_owned(), t.ink_faint),
            Some(path) => {
                let hunks = changes.diffs.get(path).map(Vec::as_slice).unwrap_or_default();
                file_diff(s, path, hunks, true, t)
            }
        },
    };
    col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body)
}

/// Every hunk of the file, 12 apart; reviewing, with what's said about its lines under them.
fn file_diff(s: &App, path: &str, hunks: &[Hunk], review: bool, t: &Theme) -> El {
    let split = s.cockpit().is_some_and(|c| c.split);
    let threads: Vec<&Thread> = if review {
        s.open_pull_full().map(|p| p.threads.iter().filter(|th| th.path == path).collect()).unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut placed = vec![false; threads.len()];
    let mut c = col().key(keys::DIFF).w_full().px(16.0).py(14.0).gap(12.0);
    for (k, h) in hunks.iter().enumerate() {
        let head = row()
            .w_full()
            .py(6.0)
            .px(12.0)
            .fill(t.chrome)
            .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
            .border_layout()
            .child(text(h.header.clone()).font(ty::MONO_11.lh(18.0)).color(t.ink_muted).truncate());
        let mut card = col()
            .key(keys::hunk(k))
            .w_full()
            .radius(size::RADIUS_LG)
            .clip_rounded(size::RADIUS_LG)
            .fill(t.canvas)
            .box_shadow(Theme::ring(1.0, t.line))
            .child(head);
        let rows = if split { pairs(&h.lines) } else { h.lines.iter().map(|l| (Some(l), None)).collect() };
        for (a, b) in rows {
            card = card.child(if split {
                split_row(path, a, b, review, t)
            } else {
                a.map_or_else(rune::el, |l| line(path, l, review, t))
            });
            // A context line is on both sides of a split row; what's said about it shows once.
            let same = matches!((a, b), (Some(x), Some(y)) if std::ptr::eq(x, y));
            for side in [a, if same { None } else { b }].into_iter().flatten() {
                for (i, th) in threads.iter().enumerate() {
                    if !placed[i] && anchored(th.side, th.line, side) {
                        placed[i] = true;
                        card = card.child(thread(s, th, t));
                    }
                }
                if review {
                    card = card.child(drafts_at(s, path, side, t));
                }
            }
        }
        c = c.child(card);
    }
    // Threads on lines the diff no longer has.
    for (i, th) in threads.iter().enumerate() {
        if !placed[i] {
            c = c.child(ui::card(t).w_full().clip_rounded(size::RADIUS_LG).child(thread(s, th, t)));
        }
    }
    c
}

/// Where a comment on `line` of `side` belongs.
fn anchored(side: Side, at: Option<u32>, l: &HunkLine) -> bool {
    match side {
        Side::New => l.kind != LineKind::Removed && l.new == at,
        Side::Old => l.kind == LineKind::Removed && l.old == at,
    }
}

/// Context beside itself; a run of removals beside the run of additions that replaced it.
fn pairs(lines: &[HunkLine]) -> Vec<(Option<&HunkLine>, Option<&HunkLine>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].kind == LineKind::Context {
            out.push((Some(&lines[i]), Some(&lines[i])));
            i += 1;
            continue;
        }
        let removed: Vec<&HunkLine> = lines[i..].iter().take_while(|l| l.kind == LineKind::Removed).collect();
        i += removed.len();
        let added: Vec<&HunkLine> = lines[i..].iter().take_while(|l| l.kind == LineKind::Added).collect();
        i += added.len();
        for k in 0..removed.len().max(added.len()) {
            out.push((removed.get(k).copied(), added.get(k).copied()));
        }
    }
    out
}

/// What a line looks like: its ground, and the colour of its numbers and sign.
fn tone(l: &HunkLine, t: &Theme) -> (Option<rune::Color>, rune::Color, &'static str) {
    match l.kind {
        LineKind::Added => (Some(t.added_soft), t.added, "+"),
        LineKind::Removed => (Some(t.removed_soft), t.removed, "−"),
        LineKind::Context => (None, t.ink_faint, " "),
    }
}

fn number(n: Option<u32>, color: rune::Color) -> El {
    row()
        .w(36.0)
        .pr(8.0)
        .justify_end()
        .child(text(n.map(|n| n.to_string()).unwrap_or_default()).font(ty::MONO_11.lh(18.0)).color(color))
}

/// Pressing a line starts a comment on it, while reviewing.
fn commentable(el: El, path: &str, l: &HunkLine, new: bool, review: bool) -> El {
    let (side, n) = if new { (Side::New, l.new) } else { (Side::Old, l.old) };
    let Some(n) = n else { return el };
    let path = path.to_owned();
    let el = el.key(keys::diff_line(&path, new, n));
    if !review {
        return el;
    }
    el.on_press_tx(move |s: &mut App, tx| {
        s.compose(Target::Line { path: path.clone(), side, line: n });
        tx.focus(keys::LINE_COMMENT, true);
    })
}

/// `DiffHunk`'s line: 36 | 36 | 16 | the text, mono 12/18.
fn line(path: &str, l: &HunkLine, review: bool, t: &Theme) -> El {
    let (bg, mark, sign) = tone(l, t);
    let mut r = row()
        .w_full()
        .h(18.0)
        .center_y()
        .child(number(l.old, mark))
        .child(number(l.new, mark))
        .child(row().w(16.0).center().child(text(sign).font(ty::MONO).color(mark)))
        .child(row().grow(1.0).min_w(0.0).pr(12.0).child(text(l.text.clone()).font(ty::MONO).color(t.ink).truncate()));
    if let Some(bg) = bg {
        r = r.fill(bg);
    }
    commentable(r, path, l, l.kind != LineKind::Removed, review)
}

/// Split: the line as it was on the left, as it is on the right, each 36 | 16 | the text.
fn split_row(path: &str, a: Option<&HunkLine>, b: Option<&HunkLine>, review: bool, t: &Theme) -> El {
    let half = |l: Option<&HunkLine>, new: bool| {
        let mut h = row().grow(1.0).min_w(0.0).h(18.0).center_y();
        if new {
            h = h.border_sides([0.0, 0.0, 0.0, 1.0], t.line).border_layout();
        }
        let Some(l) = l else { return h };
        let (bg, mark, sign) = tone(l, t);
        let n = if new { l.new } else { l.old };
        h = h.child(number(n, mark)).child(row().w(16.0).center().child(text(sign).font(ty::MONO).color(mark))).child(
            row().grow(1.0).min_w(0.0).pr(12.0).child(text(l.text.clone()).font(ty::MONO).color(t.ink).truncate()),
        );
        if let Some(bg) = bg {
            h = h.fill(bg);
        }
        commentable(h, path, l, new, review)
    };
    row().w_full().child(half(a, false)).child(half(b, true))
}

/// `.thread`: raised, ruled above and below, 10 by 12; each comment an avatar beside the name,
/// where and when, and what was said (the hunk's mono); then the reply field.
fn thread(s: &App, th: &Thread, t: &Theme) -> El {
    let now = s.env.now();
    let mut block = block(t).key(keys::thread(&th.id));
    for (i, c) in th.comments.iter().enumerate() {
        let when = time::relative(c.at, now, &s.env.tz);
        let meta = match th.line {
            Some(l) if i == 0 => format!("· line {l} · {when}"),
            _ => format!("· {when}"),
        };
        block = block.child(said(s, t, &c.author, meta, None, &c.body).mt(if i == 0 { 0.0 } else { 10.0 }));
    }
    let view = s.pull_view();
    for r in view.iter().flat_map(|v| v.draft.replies_to(&th.id)) {
        block = block.child(said(s, t, &viewer(s), "·".to_owned(), Some("Pending"), &r.body).mt(10.0));
    }
    let composing = s.cockpit().and_then(|c| c.composer.as_ref()).filter(|c| c.target == Target::Reply(th.id.clone()));
    block.child(reply(th, composing, t).mt(10.0))
}

/// The reply field: a quiet well until it's pressed.
fn reply(th: &Thread, composing: Option<&Composer>, t: &Theme) -> El {
    let key = keys::reply(&th.id);
    let well = ui::well(t).w_full().px(12.0).py(8.0);
    match composing {
        Some(c) => well.child(
            text_input(&c.edit, ty::MONO)
                .key(keys::reply_field(&th.id))
                .on_edit(|s: &mut App| s.composer_edit())
                .on_submit(|s: &mut App, _tx| s.keep_comment())
                .placeholder("Reply…".to_owned(), t.ink_faint)
                .color(t.ink)
                .w_full(),
        ),
        None => {
            let id = th.id.clone();
            // The well keeps the key the person pressed; the field gets its own.
            well.key(key).role("textbox").child(text("Reply…").font(ty::MONO).color(t.ink_faint)).on_press_tx(
                move |s: &mut App, tx| {
                    s.compose(Target::Reply(id.clone()));
                    tx.focus(keys::reply_field(&id), true);
                },
            )
        }
    }
}

fn viewer(s: &App) -> User {
    s.pulls.viewer().cloned().unwrap_or(User { login: String::new(), name: "You".to_owned() })
}

/// The draft comments on a line, and the composer when it's open there.
fn drafts_at(s: &App, path: &str, l: &HunkLine, t: &Theme) -> El {
    let (side, n) = if l.kind == LineKind::Removed { (Side::Old, l.old) } else { (Side::New, l.new) };
    let (Some(v), Some(n)) = (s.pull_view(), n) else { return rune::el() };
    let mut out = col().w_full();
    for c in v.draft.comments_on(path, side, n) {
        out = out.child(block(t).child(said(s, t, &viewer(s), format!("· line {n} ·"), Some("Pending"), &c.body)));
    }
    let here = Target::Line { path: path.to_owned(), side, line: n };
    if let Some(c) = s.cockpit().and_then(|c| c.composer.as_ref()).filter(|c| c.target == here) {
        let field = text_input(&c.edit, ty::MONO)
            .key(keys::LINE_COMMENT)
            .on_edit(|s: &mut App| s.composer_edit())
            .on_submit(|s: &mut App, _tx| s.keep_comment())
            .placeholder(format!("Comment on line {n}… (Return keeps it for your review)"), t.ink_faint)
            .color(t.ink)
            .w_full();
        let me = viewer(s);
        out = out.child(
            block(t).child(
                row()
                    .w_full()
                    .gap(8.0)
                    .items_start()
                    .child(ui::avatar(t, &me.name, pull_parts::person_color(s, t, &me)).mt(1.0))
                    .child(ui::well(t).grow(1.0).min_w(0.0).px(12.0).py(8.0).child(field)),
            ),
        );
    }
    out
}

/// `.thread`'s box.
fn block(t: &Theme) -> El {
    col().w_full().px(12.0).py(10.0).fill(t.raised).border_sides([1.0, 0.0, 1.0, 0.0], t.line).border_layout()
}

/// One comment: avatar, name and where and when (and a Pending pill for a draft), the words.
fn said(s: &App, t: &Theme, who: &User, meta: String, pill: Option<&str>, body: &str) -> El {
    let mut head = row()
        .gap(4.0)
        .center_y()
        .child(text(who.name.clone()).font(ty::MONO_STRONG).color(t.ink))
        .child(text(meta).font(ty::MONO).color(t.ink_muted));
    if let Some(p) = pill {
        head = head.child(
            row()
                .h(16.0)
                .px(5.0)
                .radius(size::PILL)
                .center()
                .fill(t.accent_soft)
                .child(text(p.to_owned()).font(ty::MONO_11).color(t.accent)),
        );
    }
    let (words, spans) = pull_parts::inline(t, body, true);
    row()
        .w_full()
        .gap(8.0)
        .items_start()
        .child(ui::avatar(t, &who.name, pull_parts::person_color(s, t, who)).mt(1.0))
        .child(col().grow(1.0).min_w(0.0).child(head).child(rich(words, spans).font(ty::MONO).color(t.ink).w_full()))
}
