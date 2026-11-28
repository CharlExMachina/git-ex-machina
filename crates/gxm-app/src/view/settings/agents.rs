//! The agent harnesses the app drives, for every repository: each one found on this computer
//! with its version and sign-in (or why it can't be used yet), the one to use, and how to add
//! one when there's none.

use gxm_core::harness::{Auth, DRIVERS, HarnessStatus};
use rune::{El, col, icon, row, text};

use super::parts;
use crate::icons;
use crate::keys;
use crate::settings::{Action, Section, What};
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

pub fn agents(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(a) = m.agents() else { return super::waiting(s, t, What::Agents) };
    let checking = m.reading.contains(&What::Agents);
    let mut out = vec![parts::note(
        t,
        "git-ex-machina isn't an agent itself: it drives the coding agent you already use, in a pull request's worktree. It finds each one on your PATH; you sign in from its own CLI.",
    )];
    if a.statuses.iter().all(|h| !h.installed()) {
        out.push(empty(t));
    } else {
        let preferred = a.preferred();
        let rows = a.statuses.iter().map(|h| agent_row(t, h, preferred == Some(h.harness))).collect();
        out.push(parts::group(t, "On this computer", vec![parts::list(t, rows)]));
    }
    let mut again =
        parts::controls(vec![parts::button(t, "check-agents", "Check again", Variant::Secondary, !checking, |s| {
            s.settings_event(crate::settings::Event::Pick(Section::Agents));
        })]);
    if checking {
        again = again.child(text("Checking…").font(ty::CAPTION).color(t.ink_muted));
    }
    out.push(again);
    out
}

/// No harness installed: what to install, and that signing in happens in its own CLI.
fn empty(t: &Theme) -> El {
    let mut card = col()
        .key(keys::AGENTS_EMPTY)
        .w_full()
        .gap(12.0)
        .padding(16.0)
        .radius(size::RADIUS_LG)
        .fill(t.canvas)
        .box_shadow(Theme::ring(1.0, t.line))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(icon(&icons::SPARKLE, 14.0).color(t.ink_muted))
                .child(text("No coding agent found").font(ty::BODY_STRONG).color(t.ink)),
        )
        .child(parts::note(t, "Install one from a terminal and sign in with its CLI, then check again here."));
    for d in DRIVERS {
        let install = d.install_hint().split('`').nth(1).unwrap_or_default().to_owned();
        card = card.child(
            row()
                .w_full()
                .gap(12.0)
                .center_y()
                .child(text(d.name().to_owned()).font(ty::BODY).color(t.ink).w(96.0))
                .child(ui::well(t).px(8.0).py(4.0).child(text(install).font(ty::MONO).color(t.ink))),
        );
    }
    card
}

/// One harness: its name, what it is here, and choosing it.
fn agent_row(t: &Theme, h: &HarnessStatus, preferred: bool) -> El {
    let id = h.harness.id();
    let about = match (&h.version, &h.auth) {
        (None, _) if !h.installed() => "Not installed".to_owned(),
        (v, Auth::SignedIn(who)) => {
            let who = who.as_ref().map_or_else(|| "Signed in".to_owned(), |w| format!("Signed in as {w}"));
            v.as_ref().map_or(who.clone(), |v| format!("{v} · {who}"))
        }
        (Some(v), _) => v.clone(),
        (None, _) => "Installed".to_owned(),
    };
    let mut words = col()
        .grow(1.0)
        .min_w(0.0)
        .child(text(h.harness.name().to_owned()).font(ty::BODY_STRONG).color(t.ink))
        .child(text(about).font(ty::HINT).color(t.ink_muted).truncate());
    if let Some(why) = &h.problem {
        words = words
            .child(text(why.clone()).font(ty::HINT).color(if h.installed() { t.removed } else { t.ink_faint }).wrap());
    }
    let mut r = parts::item("agent", id).child(row().size(16.0, 16.0).center().child(status_icon(t, h))).child(words);
    if preferred {
        r = r.child(ui::signal(&icons::CHECK, t.added, "Preferred"));
    } else if h.usable() {
        let harness = h.harness;
        r = r.child(parts::button(
            t,
            &format!("prefer-{id}"),
            &format!("Use {}", h.harness.name()),
            Variant::Secondary,
            true,
            move |s| {
                s.settings_do(Action::PreferAgent(harness));
            },
        ));
    }
    r
}

/// Ready, needing attention, or absent, at a glance.
fn status_icon(t: &Theme, h: &HarnessStatus) -> El {
    match (h.installed(), h.usable()) {
        (_, true) => icon(&icons::CHECK, 14.0).color(t.added),
        (true, false) => icon(&icons::LOCK, 14.0).color(t.removed),
        (false, _) => icon(&icons::X, 14.0).color(t.ink_faint),
    }
}
