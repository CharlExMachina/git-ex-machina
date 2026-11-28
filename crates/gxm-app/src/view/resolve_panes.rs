//! The conflict shown, in the resolve window's middle: `‹ Conflict 4 of 5 ›` with its place and
//! Ask agent to solve; both sides side by side, each with a few lines of context and what it
//! offers; the result, editable; and why the base changed these lines.

use gxm_core::resolve::flow::Agent;
use gxm_core::resolve::{Conflict, File, Merge, Side, Target, Why, markers};
use gxm_core::time;
use rune::{El, ScrollSpec, col, row, text, text_input};

use crate::icons;
use crate::keys;
use crate::resolve_window::ResolveWindow;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};
use gxm_core::resolve::Event;

pub fn centre(s: &ResolveWindow, t: &Theme, m: &Merge) -> El {
    let Some(target) = &s.flow.target else { return col() };
    let n = s.flow.current;
    let Some((f, c)) = m.conflict(n) else {
        return col()
            .center()
            .child(text("Nothing to resolve: main merged cleanly.").font(ty::BODY).color(t.ink_muted));
    };
    let i = f.conflicts.iter().position(|x| std::ptr::eq(x, c)).unwrap_or(0);
    let panes = row()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(12.0)
        .child(pane(s, t, target, f, i, c, false).grow(1.0).min_w(0.0).h_full())
        .child(pane(s, t, target, f, i, c, true).grow(1.0).min_w(0.0).h_full());
    col().w_full().child(nav(s, t, m, f, i)).child(
        col()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .padding(16.0)
            .gap(12.0)
            .child(panes)
            .child(result(s, t))
            .child(why(s, t, target, c)),
    )
}

/// `‹ Conflict 4 of 5 ›`, `path:line`, and the one primary button: Ask agent to solve.
fn nav(s: &ResolveWindow, t: &Theme, m: &Merge, f: &File, i: usize) -> El {
    let step = |key: &'static str, glyph, label: &str, by: isize| {
        ui::icon_button(t, Variant::Ghost, glyph, None, label, false)
            .key(key)
            .on_press_tx(move |s: &mut ResolveWindow, tx| s.send(Event::Step(by), Some(tx)))
    };
    let counter = row()
        .key(keys::RESOLVE_NAV)
        .gap(4.0)
        .center_y()
        .child(step(keys::RESOLVE_PREV, &icons::BACK, "Previous conflict (⌥↑)", -1))
        .child(text(format!("Conflict {} of {}", s.flow.current + 1, m.total())).font(ty::BODY_STRONG).color(t.ink))
        .child(step(keys::RESOLVE_NEXT, &icons::CHEVRON_RIGHT, "Next conflict (⌥↓)", 1));
    row()
        .w_full()
        .h(size::TOOLBAR)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout()
        .child(counter)
        .child(
            text(format!("{}:{}", f.path, f.line_of(i)))
                .key(keys::RESOLVE_PLACE)
                .font(ty::MONO_SM)
                .color(t.ink_muted)
                .truncate()
                .min_w(0.0),
        )
        .child(rune::el().grow(1.0))
        .child(ask(s, t, m))
}

/// Ask agent to solve: the agent the person uses gets every open conflict; without one, it
/// says where to add one.
fn ask(s: &ResolveWindow, t: &Theme, m: &Merge) -> El {
    let label = "Ask agent to solve";
    let b = ui::button(t, Variant::Primary, Some(&icons::SPARKLE), label, None).key(keys::ASK_AGENT);
    let open = m.open();
    let why_not = match (s.agent, &s.flow.agent) {
        (None, _) => Some("No agent harness yet. Add Claude Code or Codex in Settings › Agents.".to_owned()),
        (_, Agent::Idle) if open == 0 => Some("Every conflict is resolved".to_owned()),
        (_, Agent::Idle) if !s.flow.editable() => Some("Wait for git to finish".to_owned()),
        (_, Agent::Idle) => None,
        (Some(h), _) => Some(format!("{} is at work", h.name())),
    };
    match (why_not, s.agent) {
        (Some(why), _) => {
            ui::button_disabled(t, Some(&icons::SPARKLE), label).key(keys::ASK_AGENT).label(why.clone()).help(why)
        }
        (None, Some(h)) => {
            let said = format!("Ask {} to solve {}", h.name(), plural(open, "conflict"));
            b.label(said.clone())
                .help(format!("{said}, in the worktree, with full access. It doesn't commit: you review, then commit."))
                .on_press_tx(move |s: &mut ResolveWindow, tx| s.send(Event::Ask(h.name().to_owned()), Some(tx)))
        }
        (None, None) => ui::button_disabled(t, Some(&icons::SPARKLE), label).key(keys::ASK_AGENT),
    }
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") }
}

/// One side: its header with what it offers, then its lines with a few of context around them,
/// numbered, its own lines tinted in its lane's colour.
fn pane(s: &ResolveWindow, t: &Theme, target: &Target, f: &File, i: usize, c: &Conflict, base: bool) -> El {
    let (ink, tint) = t.side(base);
    let editable = s.flow.editable();
    let take = |key: &'static str, label: &str, side: Side, help: String| {
        if editable {
            ui::button(t, Variant::Secondary, None, label, None)
                .key(key)
                .help(help)
                .on_press_tx(move |s: &mut ResolveWindow, tx| s.send(Event::Take(side), Some(tx)))
        } else {
            ui::button_disabled(t, None, label).key(key).help(help)
        }
    };
    let (title, buttons, lines) = if base {
        let who = match c.why.as_ref().and_then(|w| w.pull.as_ref()) {
            Some(p) => format!("{} · #{} {} · {}", target.base, p.number, p.title, p.author),
            None => target.base.clone(),
        };
        let buttons = vec![
            take(keys::USE_MAIN, &format!("Use {}", target.base), Side::Base, format!("Take {}'s lines", target.base)),
            take(keys::USE_BOTH, "Use both", Side::Both, format!("Yours, then {}'s", target.base)),
        ];
        (who, buttons, &c.hunk.theirs)
    } else {
        let buttons = vec![take(keys::USE_YOURS, "Use yours", Side::Yours, format!("Take {}'s lines", target.branch))];
        (format!("Yours · {}", target.branch), buttons, &c.hunk.ours)
    };
    let head = row()
        .w_full()
        .h(36.0)
        .px(10.0)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout()
        .child(rune::el().size(8.0, 8.0).radius(4.0).fill(ink))
        .child(text(title).font(ty::CAPTION_STRONG).color(t.ink).truncate().grow(1.0).min_w(0.0))
        .children(buttons);
    let (above, below) = f.context(i);
    let start = f.line_of(i).saturating_sub(above.len());
    let mut code = col().w_full().py(6.0);
    let mut no = start;
    let mut line = |code: El, l: &str, changed: bool| {
        let mut r = row()
            .w_full()
            .px(8.0)
            .gap(8.0)
            .child(text(no.to_string()).font(ty::MONO_11).color(t.ink_faint).w(size::LINE_NO))
            .child(
                text(l.to_owned())
                    .font(ty::MONO)
                    .color(if changed { t.ink } else { t.ink_muted })
                    .truncate()
                    .min_w(0.0),
            );
        if changed {
            r = r.fill(tint).border_sides([0.0, 0.0, 0.0, 2.0], ink);
        }
        no += 1;
        code.child(r)
    };
    for l in &above {
        code = line(code, l, false);
    }
    if lines.is_empty() {
        code = code.child(
            row()
                .w_full()
                .px(8.0)
                .py(2.0)
                .fill(tint)
                .child(text("(deleted here)").font(ty::CAPTION).color(t.ink_faint)),
        );
    }
    for l in lines.lines() {
        code = line(code, l, true);
    }
    for l in &below {
        code = line(code, l, false);
    }
    let key = if base { keys::RESOLVE_THEIRS } else { keys::RESOLVE_YOURS };
    ui::well(t)
        .key(key)
        .radius(size::RADIUS_MD)
        .child(head)
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(code))
}

/// The resolution as it stands, editable: marked edited once changed by hand, with Reset and
/// Mark resolved & next.
fn result(s: &ResolveWindow, t: &Theme) -> El {
    let editable = s.flow.editable();
    let edited = s.edited();
    let resolved =
        s.flow.merge.as_ref().and_then(|m| m.conflict(s.flow.current)).is_some_and(|(_, c)| c.resolution.is_some());
    let marked = markers::has_markers(&s.result.text);
    let mut reset = ui::button(t, Variant::Ghost, Some(&icons::RESTART), "Reset", None).key(keys::RESOLVE_RESET);
    reset = if editable && (edited || resolved) {
        reset.help("Back to the conflict, markers and all").on_press_tx(|s: &mut ResolveWindow, tx| {
            if s.edited()
                && s.flow
                    .merge
                    .as_ref()
                    .and_then(|m| m.conflict(s.flow.current))
                    .is_some_and(|(_, c)| c.resolution.is_none())
            {
                // Only the pane changed: put back what it was filled with.
                s.result = rune::TextEdit::new(s.filled.clone());
            } else {
                s.send(Event::Reset, Some(tx));
            }
        })
    } else {
        ui::button_disabled(t, Some(&icons::RESTART), "Reset").key(keys::RESOLVE_RESET)
    };
    let label = "Mark resolved & next ›";
    let mark = if !editable {
        ui::button_disabled(t, None, label).key(keys::MARK_RESOLVED)
    } else if marked {
        ui::button_disabled(t, None, label).key(keys::MARK_RESOLVED).help("Remove the conflict markers first")
    } else {
        ui::button(t, Variant::Primary, None, label, None)
            .key(keys::MARK_RESOLVED)
            .help("Resolve it with this, and go to the next open conflict")
            .on_press_tx(|s: &mut ResolveWindow, tx| s.send(Event::Resolve(s.result.text.clone()), Some(tx)))
    };
    let mut head = row()
        .w_full()
        .h(36.0)
        .px(10.0)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout()
        .child(ui::label(t, "Result"));
    if edited {
        head = head.child(text("edited").key(keys::RESOLVE_EDITED).font(ty::CAPTION_STRONG).color(t.accent));
    }
    head = head.child(rune::el().grow(1.0)).child(reset).child(mark);
    let field = text_input(&s.result, ty::MONO)
        .key(keys::RESOLVE_RESULT)
        .label("Result".to_owned())
        .on_edit(|s: &mut ResolveWindow| &mut s.result)
        .color(t.ink)
        .w_full()
        .min_w(0.0);
    let field = if editable { field } else { field.disabled(true) };
    ui::well(t).w_full().h(size::RESOLVE_RESULT).radius(size::RADIUS_MD).child(head).child(
        col().w_full().grow(1.0).min_h(0.0).px(12.0).py(8.0).scroll(ScrollSpec { stick_bottom: false }).child(field),
    )
}

/// `Why main changed this: #129 Fix avatar cache eviction. Released as v0.4.0. Merged Nov 9.`
fn why(s: &ResolveWindow, t: &Theme, target: &Target, c: &Conflict) -> El {
    let said = match &c.why {
        Some(w) => sentence(s, target, w),
        None => format!("Why {} changed this: its history doesn't say.", target.base),
    };
    row()
        .key(keys::RESOLVE_WHY)
        .w_full()
        .px(12.0)
        .py(8.0)
        .gap(8.0)
        .center_y()
        .radius(size::RADIUS_SM)
        .fill(t.raised)
        .child(rune::el().size(6.0, 6.0).radius(3.0).fill(t.side(true).0))
        .child(text(said).font(ty::CAPTION).color(t.ink_muted).wrap().grow(1.0).min_w(0.0))
}

fn sentence(s: &ResolveWindow, target: &Target, w: &Why) -> String {
    let tz = &s.env.tz;
    let stop = |t: &str| if t.ends_with(['.', '!', '?']) { t.to_owned() } else { format!("{t}.") };
    match &w.pull {
        Some(p) => {
            let mut said = format!("Why {} changed this: #{} {}", target.base, p.number, stop(&p.title));
            if let Some(r) = &p.reason {
                said.push_str(&format!(" {}", stop(r)));
            }
            if let Some(at) = p.merged {
                said.push_str(&format!(" Merged {}.", time::day(at, tz)));
            }
            said
        }
        None => format!(
            "Why {} changed this: {} ({}, by {}, no pull request). Committed {}.",
            target.base,
            w.subject,
            &w.commit[..w.commit.len().min(7)],
            w.author,
            time::day(w.at, tz)
        ),
    }
}
