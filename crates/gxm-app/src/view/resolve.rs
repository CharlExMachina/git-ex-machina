//! The resolve-conflicts window: the title bar with its progress, the conflicted files down the
//! left with their conflicts, the conflict shown in the middle ([`super::resolve_panes`]), a
//! tick per conflict down the right edge, and Abort merge and Commit merge & push at the foot.
//! While the coding agent works, a strip under the title bar says so.

use gxm_core::resolve::flow::{Agent, Phase};
use gxm_core::resolve::{By, Merge};
use rune::{Cx, El, ScrollSpec, col, icon, row, text};

use super::resolve_panes;
use crate::icons;
use crate::keys;
use crate::resolve_window::ResolveWindow;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};
use gxm_core::resolve::Event;

pub fn window(s: &ResolveWindow, cx: &mut Cx<ResolveWindow>) -> El {
    let t = Theme::new(cx.system_dark());
    let root = col().key(keys::RESOLVE_WINDOW).w_full().h_full().fill(t.canvas).label("Resolve conflicts".to_owned());
    let Some(target) = &s.flow.target else { return root };
    let root = root.child(title_bar(s, &t));
    let body = match (&s.flow.phase, &s.flow.merge) {
        (Phase::Failed(why), _) => failed(&t, why),
        (_, Some(m)) => row()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .child(sidebar(s, &t, m))
            .child(resolve_panes::centre(s, &t, m).grow(1.0).min_w(0.0).h_full())
            .child(minimap(s, &t, m)),
        (_, None) => col().w_full().grow(1.0).center().gap(8.0).child(
            text(format!("Fetching {} and merging it into {}…", target.base, target.branch))
                .key(keys::RESOLVE_ERROR)
                .font(ty::BODY)
                .color(t.ink_muted),
        ),
    };
    let mut root = root.children(agent_strip(s, &t)).child(body);
    if let Some(m) = &s.flow.merge {
        root = root.child(footer(s, &t, m));
    }
    root
}

/// `Resolve conflicts · #118 Load avatars in parallel` over where the merge is, and on the
/// right how many are resolved, with a thin bar.
fn title_bar(s: &ResolveWindow, t: &Theme) -> El {
    let Some(target) = &s.flow.target else { return row() };
    let lights = super::sidebar::LIGHTS_W;
    let words = col()
        .min_w(0.0)
        .grow(1.0)
        .child(
            text(format!("Resolve conflicts · #{} {}", target.number, target.title))
                .key(keys::RESOLVE_TITLE)
                .font(ty::BODY_STRONG)
                .color(t.ink)
                .truncate(),
        )
        .child(
            text(format!("Merging {} into {} · worktree {}", target.base, target.branch, target.worktree_name))
                .key(keys::RESOLVE_SUBTITLE)
                .font(ty::CAPTION)
                .color(t.ink_muted)
                .truncate(),
        );
    let mut bar = super::bar(t).pl(if lights > 0.0 { 12.0 + lights + 12.0 } else { 20.0 }).pr(20.0).child(words);
    if let Some(m) = &s.flow.merge {
        let (total, done) = (m.total(), m.total() - m.open());
        let share = if total == 0 { 1.0 } else { done as f32 / total as f32 };
        let mut track = row().w(size::RESOLVE_PROGRESS).h(4.0).radius(2.0).fill(t.line);
        if share > 0.0 {
            track = track.child(rune::el().w(size::RESOLVE_PROGRESS * share).h(4.0).radius(2.0).fill(t.added));
        }
        bar = bar.child(
            col()
                .key(keys::RESOLVE_PROGRESS)
                .gap(5.0)
                .child(text(format!("{done} of {total} resolved")).font(ty::CAPTION_STRONG).color(t.ink).truncate())
                .child(track),
        );
    }
    bar
}

fn failed(t: &Theme, why: &str) -> El {
    col()
        .w_full()
        .grow(1.0)
        .center()
        .gap(12.0)
        .padding(40.0)
        .child(text("The merge didn't start").font(ty::TITLE).color(t.ink))
        .child(text(why.to_owned()).key(keys::RESOLVE_ERROR).font(ty::BODY).color(t.ink_muted).wrap())
        .child(
            ui::button(t, Variant::Secondary, None, "Close", None)
                .key(keys::RESOLVE_CLOSE)
                .on_press_tx(|s: &mut ResolveWindow, tx| s.send(Event::Close, Some(tx))),
        )
}

/// How long ago, for the agent's strip: `12s ago`, `3m ago`.
fn since(secs: f64) -> String {
    let secs = secs.max(0.0) as u64;
    if secs < 5 {
        return "just now".to_owned();
    }
    if secs < 60 { format!("{secs}s ago") } else { format!("{}m ago", secs / 60) }
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") }
}

/// The coding agent: at work (who, on how many, since when, its latest event, and Stop), or
/// what its run came to.
fn agent_strip(s: &ResolveWindow, t: &Theme) -> Option<El> {
    let strip = || {
        row()
            .key(keys::AGENT_STATUS)
            .w_full()
            .px(20.0)
            .py(8.0)
            .gap(10.0)
            .center_y()
            .fill(t.agent_soft())
            .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
            .border_layout()
    };
    let dot = || rune::el().size(8.0, 8.0).radius(4.0).fill(t.agent());
    let busy = |name: &str, conflicts: usize, ago: Option<String>, last: &str| {
        let mut said = format!("{name} is resolving {}", plural(conflicts, "conflict"));
        if let Some(ago) = ago {
            said.push_str(&format!(" · started {ago}"));
        }
        strip()
            .child(dot())
            .child(text(said).font(ty::BODY_STRONG).color(t.ink))
            .child(text(last.to_owned()).font(ty::MONO_11).color(t.ink_muted).truncate().grow(1.0).min_w(0.0))
            .child(
                ui::button(t, Variant::Secondary, None, "Stop", None)
                    .key(keys::AGENT_STOP)
                    .help("Stop the agent: what it already wrote stays, for you to review")
                    .on_press_tx(|s: &mut ResolveWindow, tx| s.send(Event::Stop, Some(tx))),
            )
    };
    match &s.flow.agent {
        Agent::Starting { name, conflicts } => Some(busy(name, *conflicts, None, "Starting…")),
        Agent::Working { name, conflicts, since: at, last } => {
            Some(busy(name, *conflicts, Some(since(s.now - at)), last))
        }
        Agent::Rereading { name, .. } => Some(busy(name, 0, None, "Reading what it wrote…")),
        Agent::Idle => s.flow.agent_said.as_ref().map(|said| {
            strip()
                .child(icon(&icons::SPARKLE, 14.0).color(t.agent()))
                .child(text(said.clone()).font(ty::BODY).color(t.ink).wrap().grow(1.0).min_w(0.0))
        }),
    }
}

/// Every conflicted file, with what's left in it, and its conflicts under it: the line, then
/// what it sits in. Lockfiles written again, and files to resolve elsewhere, at the end.
fn sidebar(s: &ResolveWindow, t: &Theme, m: &Merge) -> El {
    let mut list =
        col().w_full().px(10.0).pt(12.0).pb(12.0).gap(2.0).child(ui::label(t, "Conflicted files").mb(6.0).ml(6.0));
    let mut n = 0;
    for file in &m.files {
        let open = file.open();
        let state = if open == 0 {
            row()
                .gap(4.0)
                .center_y()
                .child(icon(&icons::CHECK, 12.0).color(t.added))
                .child(text("done").font(ty::CAPTION).color(t.added))
        } else {
            row()
                .gap(5.0)
                .center_y()
                .child(rune::el().size(6.0, 6.0).radius(3.0).fill(t.removed))
                .child(text(format!("{open} left")).font(ty::CAPTION).color(t.removed))
        };
        list = list.child(
            row()
                .key(keys::resolve_file(&file.path))
                .w_full()
                .h(size::ROW_FILE)
                .px(6.0)
                .gap(8.0)
                .center_y()
                .child(text(file.path.clone()).font(ty::MONO_SM).color(t.ink).truncate().grow(1.0).min_w(0.0))
                .child(state),
        );
        for (i, c) in file.conflicts.iter().enumerate() {
            let here = n == s.flow.current;
            let dot = if c.resolution.is_some() { t.added } else { t.removed };
            let at = n;
            let mut line = row()
                .key(keys::resolve_conflict(n))
                .w_full()
                .h(26.0)
                .pl(16.0)
                .pr(6.0)
                .gap(8.0)
                .center_y()
                .radius(size::RADIUS_SM)
                .child(rune::el().size(6.0, 6.0).radius(3.0).fill(dot))
                .child(text(format!(":{}", file.line_of(i))).font(ty::MONO_11).color(t.ink_muted))
                .child(
                    text(c.scope.clone().unwrap_or_default())
                        .font(ty::MONO_11)
                        .color(t.ink_faint)
                        .truncate()
                        .grow(1.0)
                        .min_w(0.0),
                )
                .on_press_tx(move |s: &mut ResolveWindow, tx| s.send(Event::Go(at), Some(tx)));
            if let Some(r) = &c.resolution {
                if r.by == By::Agent {
                    line = line.child(text("agent").font(ty::CAPTION).color(t.agent()));
                }
                line = line.child(icon(&icons::CHECK, 12.0).color(t.added));
            }
            line = if here { line.fill(t.raised).box_shadow(t.ring_here()) } else { line.hover(t.control_hover) };
            list = list.child(line);
            n += 1;
        }
    }
    for r in &m.regenerated {
        list = list.child(side_note(t, &r.path, &format!("base's, written again with {}", r.with), true));
    }
    for (path, tool) in &m.lockfiles {
        list = list.child(side_note(t, path, &format!("a lockfile: {tool} isn't installed to write it again"), false));
    }
    for path in &m.elsewhere {
        list = list.child(side_note(t, path, "can't be shown here: resolve it in a terminal", false));
    }
    let hint = row()
        .w_full()
        .px(16.0)
        .py(10.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.line)
        .border_layout()
        .child(text("⌥↓ next conflict · ⌥↑ previous").font(ty::HINT).color(t.ink_faint));
    col()
        .key(keys::RESOLVE_FILES)
        .w(size::RESOLVE_SIDEBAR)
        .h_full()
        .fill(t.chrome)
        .border_sides([0.0, 1.0, 0.0, 0.0], t.line)
        .border_layout()
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list))
        .child(hint)
}

/// A file the sidebar lists without conflicts to walk: a lockfile, or one to resolve elsewhere.
fn side_note(t: &Theme, path: &str, note: &str, done: bool) -> El {
    let glyph = if done { icon(&icons::CHECK, 12.0).color(t.added) } else { icon(&icons::X, 12.0).color(t.accent) };
    col()
        .key(keys::resolve_file(path))
        .w_full()
        .px(6.0)
        .py(4.0)
        .gap(2.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(text(path.to_owned()).font(ty::MONO_SM).color(t.ink).truncate().grow(1.0).min_w(0.0))
                .child(glyph),
        )
        .child(text(note.to_owned()).font(ty::CAPTION).color(t.ink_faint).wrap())
}

/// A tick per conflict down the right edge: green once resolved, red while open, larger for the
/// one shown. Each jumps to its conflict.
fn minimap(s: &ResolveWindow, t: &Theme, m: &Merge) -> El {
    let mut strip = col()
        .key(keys::RESOLVE_MINIMAP)
        .label(plural(m.total(), "conflict"))
        .w(size::RESOLVE_MINIMAP)
        .h_full()
        .pt(14.0)
        .gap(8.0)
        .center()
        .fill(t.chrome)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.line)
        .border_layout();
    let mut ticks = col().gap(8.0).center();
    let mut n = 0;
    for file in &m.files {
        for c in &file.conflicts {
            let color = if c.resolution.is_some() { t.added } else { t.removed };
            let (w, h) = if n == s.flow.current { (10.0, 6.0) } else { (6.0, 3.0) };
            let at = n;
            ticks = ticks.child(
                row()
                    .size(size::RESOLVE_MINIMAP, 10.0)
                    .center()
                    .child(rune::el().size(w, h).radius(1.5).fill(color))
                    .on_press_tx(move |s: &mut ResolveWindow, tx| s.send(Event::Go(at), Some(tx))),
            );
            n += 1;
        }
    }
    strip = strip.child(ticks).child(rune::el().grow(1.0));
    strip
}

/// Abort merge, with what it does; then what's left, and Commit merge & push.
fn footer(s: &ResolveWindow, t: &Theme, m: &Merge) -> El {
    let f = &s.flow;
    let aborting = matches!(f.phase, Phase::Aborting(_));
    let help = "git merge --abort: the worktree goes back to its last commit";
    let abort = if f.editable() {
        ui::button(t, Variant::Secondary, None, "Abort merge", None)
            .on_press_tx(|s: &mut ResolveWindow, tx| s.send(Event::Abort, Some(tx)))
    } else if aborting {
        ui::button_in(t, Variant::Secondary, None, "Aborting…", None, true).disabled(true)
    } else {
        ui::button_disabled(t, None, "Abort merge")
    }
    .key(keys::ABORT_MERGE)
    .help(help);
    let open = m.open();
    let left = match open {
        0 if m.elsewhere.is_empty() => "No conflicts left".to_owned(),
        0 => plural(m.elsewhere.len(), "file") + " to resolve elsewhere",
        n => format!("{} left", plural(n, "conflict")),
    };
    let committed = matches!(f.phase, Phase::Committed(_));
    let busy = matches!(f.phase, Phase::Committing(_) | Phase::Committed(Some(_)));
    let label = match f.phase {
        Phase::Committing(_) => "Committing…",
        Phase::Committed(Some(_)) => "Pushing…",
        Phase::Committed(None) => "Push again",
        _ => "Commit merge & push",
    };
    let mut commit = ui::button_in(t, Variant::Primary, Some(&icons::PUSH), label, None, busy).key(keys::COMMIT_MERGE);
    commit = if committed && !busy {
        commit
            .help("The merge is committed: push it")
            .on_press_tx(|s: &mut ResolveWindow, tx| s.send(Event::Push, Some(tx)))
    } else if f.committable() {
        let branch = f.target.as_ref().map(|t| t.branch.clone()).unwrap_or_default();
        commit
            .help(format!("Commit the merge and push {branch}"))
            .on_press_tx(|s: &mut ResolveWindow, tx| s.send(Event::Commit, Some(tx)))
    } else if busy {
        commit.disabled(true)
    } else {
        ui::button_disabled(t, Some(&icons::PUSH), label).key(keys::COMMIT_MERGE).help("Resolve every conflict first")
    };
    let mut foot = col().w_full();
    if let Some(e) = &f.error {
        foot = foot.child(
            row()
                .w_full()
                .px(20.0)
                .pt(10.0)
                .child(text(e.clone()).key(keys::RESOLVE_ERROR).font(ty::CAPTION).color(t.removed).wrap()),
        );
    }
    foot.key(keys::RESOLVE_FOOTER).fill(t.chrome).border_sides([1.0, 0.0, 0.0, 0.0], t.line).border_layout().child(
        row()
            .w_full()
            .px(20.0)
            .py(12.0)
            .gap(12.0)
            .center_y()
            .child(abort)
            .child(text("puts the worktree back exactly as it was").font(ty::CAPTION).color(t.ink_faint))
            .child(rune::el().grow(1.0))
            .child(text(left).key(keys::RESOLVE_LEFT).font(ty::CAPTION_STRONG).color(if open == 0 {
                t.added
            } else {
                t.ink_muted
            }))
            .child(commit),
    )
}
