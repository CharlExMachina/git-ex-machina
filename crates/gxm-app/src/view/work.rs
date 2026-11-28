//! The Work view: the toolbar, a line saying what's waiting, and the work list in three
//! sections, each oldest first. A row is the age (a bare number coloured by how stale it is),
//! the title with its meta line, the worktree tile and what to do next.

use gxm_core::forge::model::Pull;
use gxm_core::time;
use gxm_core::work::rules::{self, Next, Staleness};
use gxm_core::work::{Item, Section, Tone, Worktree};
use rune::{El, ScrollSpec, col, icon, row, text};

use super::pull_parts::person_color;
use crate::icons;
use crate::keys;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};
use crate::work::{Action, Job, Row};

pub fn pane(s: &App, t: &Theme) -> El {
    col()
        .fill(t.canvas)
        .child(super::toolbar::toolbar(s, t))
        .child(summary(s, t))
        .child(list(s, t).grow(1.0).min_h(0.0))
}

/// `atlas-web · 3 to review · 4 yours · 1 worktree with no PR yet · fetched 2 minutes ago`,
/// then why there are no pull requests, when there aren't.
fn summary(s: &App, t: &Theme) -> El {
    let rows = s.work_rows();
    let count = |section| rows.iter().filter(|r| r.entry.section == section).count();
    let (title, problem) = if s.work.everywhere {
        let repos = s.work.repos.iter().filter(|r| r.work.as_ref().is_some_and(|w| !w.entries.is_empty())).count();
        (format!("All open work · {}", plural(repos, "repository", "repositories")), None)
    } else {
        let problem = s.repo().and_then(|r| s.work.of(&r.summary.path)).and_then(|w| w.problem.clone());
        (s.repo().map(|r| r.summary.name.clone()).unwrap_or_default(), problem)
    };
    let mut parts = vec![
        format!("{} to review", count(Section::Review)),
        format!("{} yours", count(Section::Yours)),
        format!("{} with no PR yet", plural(count(Section::Unproposed), "worktree", "worktrees")),
    ];
    if !s.work.everywhere
        && let Some(at) = s.repo().and_then(|r| r.summary.fetched)
    {
        parts.push(format!("fetched {}", time::long(at, s.env.now(), &s.env.tz)));
    }
    let line = row()
        .key(keys::WORK_SUMMARY)
        .gap(6.0)
        .center_y()
        .child(text(title).font(ty::CAPTION_STRONG).color(t.ink))
        .child(text(format!("· {}", parts.join(" · "))).font(ty::CAPTION).color(t.ink_muted));
    let bar = row()
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
        .child(line);
    let right = match problem {
        Some(p) => text(p).key(keys::WORK_NOTICE).font(ty::CAPTION).color(t.ink_faint).truncate().min_w(0.0),
        None => text("Oldest first").font(ty::HINT).color(t.ink_faint),
    };
    // Settings, as beside the history's search: this repository's, and Agents for every one.
    let settings = ui::button_in(t, Variant::Secondary, Some(&icons::SETTINGS), "", None, s.settings.open)
        .key(keys::SETTINGS)
        .label("Settings".to_owned())
        .help("Settings: this repository's, and the coding agents every repository uses")
        .on_press(|s: &mut App| s.open_settings());
    bar.child(row().gap(12.0).center_y().min_w(0.0).child(right).child(settings))
}

/// The teal mark on a worktree's tile while an agent gxm started works in it.
fn agent_working(t: &Theme) -> El {
    row()
        .gap(4.0)
        .center_y()
        .child(rune::el().size(6.0, 6.0).radius(3.0).fill(t.agent()))
        .child(text("agent working").font(ty::TILE_STATE).color(t.agent()))
}

fn list(s: &App, t: &Theme) -> El {
    let rows = s.work_rows();
    let current = s.work_row().map(|r| (r.repo.to_path_buf(), r.entry.item.clone()));
    let mut list = col().key(keys::WORK_LIST).w_full().px(12.0).pt(6.0).pb(16.0);
    if rows.is_empty() {
        let note = if s.work.repos.is_empty() {
            "Reading what's open…"
        } else {
            "Nothing open. Merged work is out of the way."
        };
        return col().key(keys::WORK_LIST).w_full().center().child(text(note).font(ty::BODY).color(t.ink_muted));
    }
    for section in Section::ALL {
        let here: Vec<&Row<'_>> = rows.iter().filter(|r| r.entry.section == section).collect();
        if here.is_empty() {
            continue;
        }
        let id = match section {
            Section::Review => "review",
            Section::Yours => "yours",
            Section::Unproposed => "worktrees",
        };
        list = list.child(
            row()
                .key(keys::work_section(id))
                .w_full()
                .pt(14.0)
                .px(8.0)
                .pb(6.0)
                .gap(8.0)
                .center_y()
                .child(ui::label(t, section.title()))
                .child(rune::el().grow(1.0).h(1.0).fill(t.line)),
        );
        for r in here {
            let picked = current.as_ref().is_some_and(|(repo, item)| repo == r.repo && *item == r.entry.item);
            list = list.child(item(s, r, picked, t));
        }
    }
    col().w_full().scroll(ScrollSpec { stick_bottom: false }).child(list)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

/// One row: age, title and meta, worktree tile, action.
fn item(s: &App, r: &Row<'_>, picked: bool, t: &Theme) -> El {
    let now = s.env.now();
    let age = text(rules::age(r.entry.since, now)).font(ty::AGE).color(t.staleness(Staleness::of(r.entry.since, now)));
    let base = &r.work.local.base;
    let here = s.checkout_path();
    let (key, main, tile, action) = match &r.entry.item {
        Item::Pull(n) => {
            let Some(p) = r.work.pull(*n) else { return rune::el() };
            let wt = r.work.local.on_branch(&p.summary.head_ref);
            let conflicts =
                r.work.conflict(*n).is_some() || p.mergeable == gxm_core::forge::model::Mergeable::Conflicts;
            let is_here = wt.is_some_and(|w| here.as_ref() == Some(&w.path));
            let key = keys::work_pull(&r.repo_name(), *n);
            let mut tile = tile(t, wt, conflicts, is_here, base, &keys::work_tile(&key), r.entry.section);
            if wt.is_some_and(|w| s.agent_at.as_ref() == Some(&w.path)) {
                tile = tile.child(agent_working(t));
            }
            (key, pull_main(s, r, p, t), tile, pull_action(s, r, p, t))
        }
        Item::Worktree(path) => {
            let Some(w) = r.work.local.worktrees.iter().find(|w| &w.path == path) else { return rune::el() };
            let is_here = here.as_ref() == Some(&w.path);
            let open_pr = ui::button(t, Variant::Secondary, None, "Open PR", None)
                .help("Coming next: open a pull request from this worktree")
                .disabled(true);
            let key = keys::work_worktree(&w.name);
            let tile = tile(t, Some(w), false, is_here, base, &keys::work_tile(&key), r.entry.section);
            (key, worktree_main(s, r, w, t), tile, open_pr)
        }
    };
    let (repo, it) = (r.repo.to_path_buf(), r.entry.item.clone());
    let mut el = row()
        .key(key)
        .w_full()
        .px(10.0)
        .py(8.0)
        .gap(12.0)
        .center_y()
        .radius(size::RADIUS_MD)
        .child(row().w(size::WORK_AGE).child(age))
        .child(main.grow(1.0).min_w(0.0))
        .child(tile.w(size::WORK_TILE))
        .child(row().w(size::WORK_ACTION).justify_end().child(action))
        .on_press(move |s: &mut App| s.pick_work(repo.clone(), it.clone()));
    if picked {
        el = el.fill(t.raised);
        for sh in t.shadow_lifted() {
            el = el.box_shadow(sh);
        }
    } else {
        el = el.hover(t.control_hover);
    }
    el
}

/// `#118 Load avatars in parallel` over `perf/avatar-loading · Kenji reviewed`.
fn pull_main(s: &App, r: &Row<'_>, p: &Pull, t: &Theme) -> El {
    let title = row()
        .gap(6.0)
        .center_y()
        .child(text(format!("#{}", p.summary.number)).font(ty::MONO_SM).color(t.ink_faint))
        .child(text(p.summary.title.clone()).font(ty::BODY).color(t.ink).truncate());
    let mut meta = row().gap(10.0).center_y();
    if s.work.everywhere {
        meta = meta.child(text(r.repo_name()).font(ty::CAPTION_STRONG).color(t.ink_muted));
    }
    if r.entry.section == Section::Review {
        let who = &p.summary.author;
        meta = meta.child(
            row()
                .gap(5.0)
                .center_y()
                .child(ui::avatar(t, &who.name, person_color(s, t, who)))
                .child(text(who.name.clone()).font(ty::CAPTION).color(t.ink_faint)),
        );
    }
    meta = meta.child(text(p.summary.head_ref.clone()).font(ty::MONO_11).color(t.ink_faint).truncate());
    if r.entry.section == Section::Review
        && let Some((a, d)) = r.work.open.as_ref().and_then(|o| o.stats.get(&p.summary.number))
    {
        meta = meta.child(text(format!("+{a} −{d}")).font(ty::MONO_11).color(t.ink_faint));
    }
    if r.entry.section == Section::Yours {
        let (words, tone) = rules::review_state(p);
        meta = meta.child(match tone {
            Tone::Good => ui::signal(&icons::CHECK, t.added, words),
            _ => text(words).font(ty::CAPTION).color(t.ink_faint),
        });
    }
    col().gap(3.0).child(title).child(meta)
}

/// A worktree's latest commit over its branch and what's not pushed.
fn worktree_main(s: &App, r: &Row<'_>, w: &Worktree, t: &Theme) -> El {
    let title = w.subject.clone().or_else(|| w.branch.clone()).unwrap_or_else(|| w.name.clone());
    let mut meta = row().gap(10.0).center_y();
    if s.work.everywhere {
        meta = meta.child(text(r.repo_name()).font(ty::CAPTION_STRONG).color(t.ink_muted));
    }
    meta = meta.child(text(w.branch.clone().unwrap_or_default()).font(ty::MONO_11).color(t.ink_faint).truncate());
    if w.unpushed > 0 {
        meta = meta.child(text(format!("{} unpushed", w.unpushed)).font(ty::CAPTION).color(t.ink_faint));
    }
    col().gap(3.0).child(text(title).font(ty::BODY).color(t.ink).truncate()).child(meta)
}

/// What a pull request's row offers: Review, Resolve conflicts, Merge, or where it's waiting.
fn pull_action(s: &App, r: &Row<'_>, p: &Pull, t: &Theme) -> El {
    let n = p.summary.number;
    if r.entry.section == Section::Review {
        let repo = r.repo.to_path_buf();
        return ui::button(t, Variant::Secondary, Some(&icons::EYE), "Review", None)
            .key(keys::work_action(&r.repo_name(), n))
            .on_press(move |s: &mut App| s.review_pull(&repo, n));
    }
    match rules::next(p) {
        Next::ResolveConflicts => {
            resolve_button(r, p, ui::button(t, Variant::DangerSoft, None, "Resolve conflicts", None))
                .key(keys::work_action(&r.repo_name(), n))
        }
        Next::Merge => {
            let running = s.work.running(n) == Some(&Action::Merge);
            let label = if running { "Merging…" } else { "Merge" };
            let b = ui::button_in(t, Variant::Primary, Some(&icons::MERGE), label, None, running)
                .key(keys::work_action(&r.repo_name(), n))
                .help(format!("Squash and merge into {}", p.summary.base_ref));
            if running {
                b.disabled(true)
            } else {
                let repo = r.repo.to_path_buf();
                b.on_press(move |s: &mut App| s.act(Job { repo: repo.clone(), number: n, action: Action::Merge }))
            }
        }
        Next::Wait(word) => ui::pill(t, word, Tone::Quiet).key(keys::work_action(&r.repo_name(), n)),
    }
}

/// Resolve conflicts on `button`: it opens every conflict in a window of its own, merging the
/// base into the pull request's worktree; without a worktree there's nowhere to merge yet.
pub fn resolve_button(r: &Row<'_>, p: &Pull, button: El) -> El {
    let n = p.summary.number;
    if r.work.local.on_branch(&p.summary.head_ref).is_none() {
        return button
            .help(format!("Coming next: check out {} in a worktree to resolve it here", p.summary.head_ref))
            .disabled(true);
    }
    let repo = r.repo.to_path_buf();
    button
        .help(format!(
            "Merge {} into {} in its worktree, and resolve every conflict",
            p.summary.base_ref, p.summary.head_ref
        ))
        .on_press(move |s: &mut App| s.open_resolve(&repo, n))
}

/// The worktree column: a recessed tile with the worktree's name and state, coloured by its
/// state, ringed when it's the checkout picked; a "No worktree" outline when there's none.
pub fn tile(
    t: &Theme,
    w: Option<&Worktree>,
    conflicts: bool,
    here: bool,
    base: &str,
    key: &str,
    section: Section,
) -> El {
    let shell = row().key(key.to_owned()).px(9.0).py(5.0).gap(8.0).center_y().radius(size::RADIUS_SM).min_w(0.0);
    let glyph =
        |fg, bg| row().size(22.0, 22.0).radius(5.0).center().fill(bg).child(icon(&icons::WORKTREE, 12.0).color(fg));
    let words = |name: String, state: String, name_ink| {
        col()
            .min_w(0.0)
            .grow(1.0)
            .child(text(name).font(ty::TILE_NAME).color(name_ink).truncate())
            .child(text(state).font(ty::TILE_STATE).color(t.ink_faint).truncate())
    };
    match w {
        Some(w) => {
            let (fg, bg) = t.tone(w.tone(conflicts));
            let mut el = shell.fill(t.well);
            for sh in t.shadow_well() {
                el = el.box_shadow(sh);
            }
            if here {
                el = el.box_shadow(t.ring_here());
            }
            let name = if w.main { "main checkout".to_owned() } else { w.name.clone() };
            el.child(glyph(fg, bg)).child(words(name, w.state(base), t.ink))
        }
        None => {
            let hint = if section == Section::Review { "Check out to review" } else { "" };
            shell
                .box_shadow(Theme::ring(1.0, t.line))
                .child(row().size(22.0, 22.0).center().child(icon(&icons::PLUS, 12.0).color(t.ink_faint)))
                .child(words("No worktree".to_owned(), hint.to_owned(), t.ink_muted))
        }
    }
}
