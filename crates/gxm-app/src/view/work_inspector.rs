//! The inspector beside the work list, for the row picked (or the one stuck the longest):
//! its title, why it's stuck, its checks, the comments that aren't resolved, and what to do.

use gxm_core::forge::model::{CheckState, Pull};
use gxm_core::time;
use gxm_core::work::rules::{self, ThreadState};
use gxm_core::work::{Item, Section, Tone, Worktree, brief};
use rune::{El, ScrollSpec, col, icon, rich, row, text};

use super::bar;
use super::pull_parts::{self, person_color};
use crate::icons;
use crate::keys;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};
use crate::work::{Action, Job, Row};

pub fn inspector(s: &App, t: &Theme) -> El {
    let row_ = s.work_row();
    let url = row_.and_then(|r| match &r.entry.item {
        Item::Pull(n) => r.work.pull(*n).map(|p| p.summary.url.clone()),
        Item::Worktree(_) => None,
    });
    let mut open = ui::button(t, Variant::Ghost, Some(&icons::EXTERNAL), "", None).key(keys::OPEN_ON_PROVIDER);
    open = match url {
        Some(url) => open.help("Open on GitHub").on_press_tx(move |_: &mut App, tx| {
            // A browser that won't open leaves the person where they are.
            tx.open_url(&url).ok();
        }),
        None => open.disabled(true).opacity(0.6),
    };
    let top = bar(t).key(keys::INSPECTOR_BAR).pl(20.0).pr(12.0).child(rune::el().grow(1.0)).child(open);
    let body = match row_ {
        None => col().padding(20.0).child(text("Nothing open.").font(ty::BODY).color(t.ink_faint)),
        Some(r) => match &r.entry.item {
            Item::Pull(n) => match r.work.pull(*n) {
                Some(p) => pull(s, &r, p, t),
                None => col(),
            },
            Item::Worktree(path) => match r.work.local.worktrees.iter().find(|w| &w.path == path) {
                Some(w) => worktree(s, &r, w, t),
                None => col(),
            },
        },
    };
    col()
        .key(keys::WORK_INSPECTOR)
        .fill(t.chrome)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.line)
        .border_layout()
        .child(top)
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}

/// `STUCK THE LONGEST` when nothing is picked; else the row's section.
fn caption(s: &App, r: &Row<'_>) -> &'static str {
    if s.work.picked.is_none() {
        return "Stuck the longest";
    }
    match r.entry.section {
        Section::Review => "Needs your review",
        Section::Yours => "Your pull request",
        Section::Unproposed => "Worktree with no PR yet",
    }
}

fn head(t: &Theme, caption: &str, title: String, meta: String) -> El {
    col()
        .w_full()
        .gap(6.0)
        .child(ui::label(t, caption))
        .child(text(title).key(keys::WORK_TITLE).font(ty::DISPLAY_SM).color(t.ink).wrap())
        .child(text(meta).font(ty::CAPTION).color(t.ink_muted).wrap())
}

/// A raised card under its label, with a word on the right.
fn card(t: &Theme, label: &str, right: Option<(String, Tone)>) -> El {
    let mut top = row().w_full().pb(4.0).gap(8.0).center_y().child(ui::label(t, label)).child(rune::el().grow(1.0));
    if let Some((words, tone)) = right {
        top = top.child(text(words).font(ty::CAPTION).color(t.tone(tone).0));
    }
    ui::card(t).w_full().px(12.0).py(10.0).gap(4.0).child(top)
}

fn pull(s: &App, r: &Row<'_>, p: &Pull, t: &Theme) -> El {
    let now = s.env.now();
    let viewer = r.work.open.as_ref().map_or("", |o| o.viewer.login.as_str());
    let sum = &p.summary;
    let by = if sum.author.login == viewer { "you".to_owned() } else { sum.author.name.clone() };
    let meta = format!(
        "Opened {} by {by} · {} · {} → {}",
        time::relative(p.created, now, &s.env.tz),
        pull_parts::plural(p.commits as usize, "commit"),
        sum.head_ref,
        sum.base_ref
    );
    let conflict = r.work.conflict(sum.number);
    let mut c = col().w_full().padding(20.0).gap(14.0).child(head(
        t,
        caption(s, r),
        format!("#{} {}", sum.number, sum.title),
        meta,
    ));
    // Why it's stuck.
    let why = rules::stuck(p, viewer, conflict);
    if !why.is_empty() {
        let mut box_ = card(t, "Why it's stuck", None).key(keys::WORK_STUCK);
        for reason in &why {
            box_ = box_.child(
                row()
                    .w_full()
                    .gap(8.0)
                    .items_start()
                    .child(rune::el().size(6.0, 6.0).mt(6.0).radius(3.0).fill(t.tone(reason.tone()).0))
                    .child(text(reason.sentence(now)).font(ty::BODY).color(t.ink).wrap().grow(1.0).min_w(0.0)),
            );
        }
        c = c.child(box_);
    }
    if let Some(checks) = checks(s, r, p, t) {
        c = c.child(checks);
    }
    c = c.child(threads(s, p, viewer, t));
    c.child(actions(s, r, p, t))
}

/// The checks card: none without CI; one line when everything passed; otherwise failed first,
/// tinted, each with its own ↻, then Re-run failed checks.
fn checks(s: &App, r: &Row<'_>, p: &Pull, t: &Theme) -> Option<El> {
    let cs = rules::checks(p)?;
    let n = p.summary.number;
    let mut said = Vec::new();
    if cs.failed > 0 {
        said.push(format!("{} failing", cs.failed));
    }
    if cs.running > 0 {
        said.push(format!("{} running", cs.running));
    }
    said.push(format!("{} passed", cs.passed));
    let tone = if cs.failed > 0 {
        Tone::Bad
    } else if cs.running > 0 {
        Tone::Warn
    } else {
        Tone::Good
    };
    let mut box_ = card(t, "Checks", Some((said.join(" · "), tone))).key(keys::WORK_CHECKS);
    // A bar split in proportion: passed, running, failed.
    let mut strip = row().w_full().h(4.0).gap(2.0).radius(2.0).mb(6.0);
    for (count, color) in [(cs.passed, t.added), (cs.running, t.accent), (cs.failed, t.removed)] {
        if count > 0 {
            strip = strip.child(rune::el().grow(count as f32).h(4.0).radius(2.0).fill(color));
        }
    }
    box_ = box_.child(strip);
    if cs.all_passed() {
        let names: Vec<&str> = cs.rows.iter().map(|c| c.name.as_str()).collect();
        let took: u32 = cs.rows.iter().filter_map(|c| c.seconds).sum();
        return Some(
            box_.child(
                row()
                    .w_full()
                    .h(24.0)
                    .gap(8.0)
                    .center_y()
                    .child(icon(&icons::CHECK, 14.0).color(t.added))
                    .child(text(names.join(" · ")).font(ty::MONO_SM).color(t.ink).truncate().grow(1.0).min_w(0.0))
                    .child(text(pull_parts::duration(took)).font(ty::MONO_11).color(t.ink_faint)),
            ),
        );
    }
    let busy = s.work.running(n).is_some();
    for c in &cs.rows {
        let (glyph, color) = match c.state {
            CheckState::Failed => (&icons::X, t.removed),
            CheckState::Pending => (&icons::CLOCK, t.accent),
            CheckState::Passed => (&icons::CHECK, t.added),
            CheckState::Skipped => (&icons::CHEVRON, t.ink_faint),
        };
        let took = match c.state {
            CheckState::Pending => "running".to_owned(),
            _ => c.seconds.map(pull_parts::duration).unwrap_or_default(),
        };
        let mut line = row()
            .w_full()
            .h(26.0)
            .px(6.0)
            .gap(8.0)
            .center_y()
            .radius(size::RADIUS_SM)
            .child(icon(glyph, 14.0).color(color))
            .child(text(c.name.clone()).font(ty::MONO_SM).color(t.ink).truncate().grow(1.0).min_w(0.0))
            .child(text(took).font(ty::MONO_11).color(t.ink_faint));
        if c.state == CheckState::Failed {
            let label = format!("Re-run {}", c.name);
            let pressed = s.work.running(n) == Some(&Action::Rerun(c.name.clone()));
            let mut again = ui::icon_button(t, Variant::Secondary, &icons::REFRESH, None, &label, pressed)
                .key(keys::rerun(&c.name));
            again = if c.job.is_none() {
                again.disabled(true).opacity(0.6).help(format!("{} can only run again on GitHub", c.name))
            } else if busy {
                again.disabled(true)
            } else {
                let (repo, name) = (r.repo.to_path_buf(), c.name.clone());
                again.on_press(move |s: &mut App| {
                    s.act(Job { repo: repo.clone(), number: n, action: Action::Rerun(name.clone()) });
                })
            };
            line = line.fill(t.removed_soft).child(again);
        }
        box_ = box_.child(line);
    }
    if cs.failed > 0 {
        let pressed = s.work.running(n) == Some(&Action::RerunFailed);
        let mut all =
            ui::button_in(t, Variant::Secondary, Some(&icons::REFRESH), "Re-run failed checks", None, pressed)
                .key(keys::RERUN_FAILED)
                .w_full()
                .justify_center()
                .mt(6.0);
        all = if busy {
            all.disabled(true)
        } else {
            let repo = r.repo.to_path_buf();
            all.on_press(move |s: &mut App| s.act(Job { repo: repo.clone(), number: n, action: Action::RerunFailed }))
        };
        box_ = box_.child(all);
    }
    Some(box_)
}

/// Unresolved comments, those that need the viewer first, those waiting on others dimmed;
/// or, before anyone has reviewed it, a card saying so.
fn threads(s: &App, p: &Pull, viewer: &str, t: &Theme) -> El {
    let now = s.env.now();
    if !rules::reviewed(p) {
        return ui::card(t).key(keys::WORK_THREADS).w_full().px(12.0).py(10.0).child(
            row()
                .gap(10.0)
                .center_y()
                .child(
                    row()
                        .size(28.0, 28.0)
                        .radius(14.0)
                        .center()
                        .fill(t.well)
                        .child(icon(&icons::COMMENT, 14.0).color(t.ink_faint)),
                )
                .child(text("This PR hasn't been reviewed yet.").font(ty::BODY).color(t.ink_muted)),
        );
    }
    let open = rules::unresolved(p, viewer);
    let needs = open.iter().filter(|(_, st)| *st == ThreadState::NeedsYou).count();
    let right = match needs {
        0 => None,
        n => Some((format!("{n} need{} you", if n == 1 { "s" } else { "" }), Tone::Warn)),
    };
    let mut box_ = card(t, "Unresolved comments", right).key(keys::WORK_THREADS);
    if open.is_empty() {
        return box_.child(text("Every thread is resolved.").font(ty::BODY).color(t.ink_faint));
    }
    for (i, (th, state)) in open.iter().enumerate() {
        // What the viewer reads: the last word from someone else.
        let said = th.comments.iter().rev().find(|c| c.author.login != viewer).or(th.comments.last());
        let Some(said) = said else { continue };
        let place = th.line.map_or_else(|| format!("{} (outdated)", th.path), |l| format!("{}:{l}", th.path));
        let who = row()
            .gap(6.0)
            .center_y()
            .child(ui::avatar(t, &said.author.name, person_color(s, t, &said.author)))
            .child(
                text(format!("{} · {} ·", said.author.name, time::relative(said.at, now, &s.env.tz)))
                    .font(ty::CAPTION)
                    .color(t.ink_faint),
            )
            .child(text(place).font(ty::MONO_11).color(t.ink_muted).truncate());
        let (body, spans) = pull_parts::inline(t, said.body.trim(), false);
        let mut entry = col()
            .key(keys::thread(&th.id))
            .w_full()
            .py(8.0)
            .gap(3.0)
            .child(who)
            .child(rich(body, spans).font(ty::BODY).color(t.ink).w_full());
        if i > 0 {
            entry = entry.border_sides([1.0, 0.0, 0.0, 0.0], t.line).border_layout();
        }
        if let ThreadState::Waiting(on) = state {
            let replied = th.comments.last().map(|c| time::relative(c.at, now, &s.env.tz)).unwrap_or_default();
            let waiting = on.as_ref().map(|u| format!(" · waiting on {}", rules::first_name(u))).unwrap_or_default();
            entry = entry
                .child(text(format!("↳ You replied {replied}{waiting}")).font(ty::CAPTION).color(t.ink_faint))
                .opacity(0.6);
        }
        box_ = box_.child(entry);
    }
    box_
}

/// Resolve conflicts when there are any, then the brief for an agent and the terminal.
fn actions(s: &App, r: &Row<'_>, p: &Pull, t: &Theme) -> El {
    let mut c = col().w_full().gap(8.0);
    if let crate::work::Acting::Failed { job, message } = &s.work.acting
        && job.number == p.summary.number
    {
        c = c.child(text(message.clone()).key(keys::WORK_ERROR).font(ty::CAPTION).color(t.removed).wrap());
    }
    if r.work.conflict(p.summary.number).is_some() || p.mergeable == gxm_core::forge::model::Mergeable::Conflicts {
        let button = ui::button(t, Variant::Primary, None, "Resolve conflicts", None).w_full().justify_center();
        c = c.child(super::work::resolve_button(r, p, button).key(keys::RESOLVE_CONFLICTS));
    }
    let viewer = r.work.open.as_ref().map_or("", |o| o.viewer.login.as_str());
    let place = r.work.local.on_branch(&p.summary.head_ref).map(|w| s.env.tilde(&w.path));
    let text_ = brief::pull(p, viewer, r.work.conflict(p.summary.number), place.as_deref(), s.env.now());
    c.child(brief_and_terminal(t, text_, place))
}

fn brief_and_terminal(t: &Theme, brief: String, place: Option<String>) -> El {
    let copy = ui::button(t, Variant::Secondary, Some(&icons::COPY), "Copy agent brief", None)
        .key(keys::COPY_BRIEF)
        .grow(1.0)
        .justify_center()
        .help("Copies what an agent needs to pick this up, as Markdown")
        .on_press_tx(move |s: &mut App, tx| {
            tx.copy(&brief);
            s.toasts.show("Agent brief copied", tx);
        });
    let next = match place {
        Some(p) => format!("Coming next: a terminal in {p}"),
        None => "Coming next: a terminal in its worktree".to_owned(),
    };
    let terminal = ui::button(t, Variant::Secondary, None, "Open in terminal", None)
        .key(keys::OPEN_TERMINAL)
        .grow(1.0)
        .justify_center()
        .help(next)
        .disabled(true);
    row().w_full().gap(8.0).child(copy).child(terminal)
}

fn worktree(s: &App, r: &Row<'_>, w: &Worktree, t: &Theme) -> El {
    let now = s.env.now();
    let title = w.subject.clone().or_else(|| w.branch.clone()).unwrap_or_else(|| w.name.clone());
    let place = s.env.tilde(&w.path);
    let meta = format!("{} · {place}", w.branch.as_deref().unwrap_or("detached"));
    let base = &r.work.local.base;
    let mut state = card(t, "Worktree", None).key(keys::WORK_STUCK);
    let mut lines = vec![(w.tone(false), w.state(base))];
    if w.unpushed > 0 && w.changed > 0 {
        lines.push((Tone::Quiet, format!("{} unpushed", pull_parts::plural(w.unpushed as usize, "commit"))));
    }
    if let Some(at) = w.edited {
        lines.push((Tone::Quiet, format!("Edited {}", time::long(at, now, &s.env.tz))));
    }
    lines.push((Tone::Warn, "No pull request yet".to_owned()));
    for (tone, words) in lines {
        state = state.child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(rune::el().size(6.0, 6.0).radius(3.0).fill(t.tone(tone).0))
                .child(text(words).font(ty::BODY).color(t.ink)),
        );
    }
    let open_pr = ui::button(t, Variant::Primary, None, "Open PR", None)
        .w_full()
        .justify_center()
        .help("Coming next: open a pull request from this worktree")
        .disabled(true);
    let brief = brief::worktree(w, base, &place);
    col()
        .w_full()
        .padding(20.0)
        .gap(14.0)
        .child(head(t, caption(s, r), title, meta))
        .child(state)
        .child(col().w_full().gap(8.0).child(open_pr).child(brief_and_terminal(t, brief, Some(place))))
}
