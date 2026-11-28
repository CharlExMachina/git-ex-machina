//! The checkout control at the toolbar's left: which checkout of the repository Fetch, Pull,
//! Push and History act on. Closed, it names the checkout, its folder and its state; open, it
//! lists the main checkout, then every worktree with its branch, pull request and state.

use std::path::PathBuf;

use gxm_core::work::{Tone, Work, Worktree};
use rune::{El, col, icon, row, text};

use crate::icons;
use crate::keys;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui;

/// The selected repository's checkouts, as the last work read found them.
fn checkouts(s: &App) -> Option<&Work> {
    let repo = s.repo()?;
    s.work.of(&repo.summary.path)?.work.as_ref()
}

/// The checkout in use, as the work read knows it.
fn current(s: &App) -> Option<&Worktree> {
    let path = s.checkout_path()?;
    checkouts(s)?.local.worktrees.iter().find(|w| w.path == path)
}

/// A checkout's name as the control says it: the main checkout by its branch.
fn name_of(w: &Worktree) -> String {
    if w.main { w.branch.clone().unwrap_or_else(|| w.name.clone()) } else { w.name.clone() }
}

pub fn control(s: &App, t: &Theme) -> El {
    let Some(repo) = s.repo() else { return rune::el() };
    let path = s.checkout_path().unwrap_or_else(|| repo.summary.path.clone());
    let (name, state) = match (current(s), checkouts(s)) {
        (Some(w), Some(_)) => (name_of(w), format!("{} · {}", s.env.tilde(&w.path), compact(w))),
        _ => {
            let branch = s.status().and_then(|st| st.branch.clone()).unwrap_or_else(|| repo.summary.name.clone());
            (branch, s.env.tilde(&path))
        }
    };
    let mut c = row()
        .key(keys::CHECKOUT_PICKER)
        .role("button")
        .label(format!("Checkout: {name}"))
        .help("The checkout Fetch, Pull, Push and History act on")
        .w(size::CHECKOUT_W)
        .h(36.0)
        .pl(6.0)
        .pr(10.0)
        .gap(9.0)
        .center_y()
        .radius(size::RADIUS_SM)
        .fill(t.control)
        .hover(t.control_hover)
        .press(t.control_press)
        .no_window_drag()
        .child(
            row()
                .size(24.0, 24.0)
                .radius(size::RADIUS_SM)
                .center()
                .fill(t.accent_soft)
                .child(icon(&icons::WORKTREE, 14.0).color(t.accent)),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(text(name).font(ty::TILE_NAME).color(t.ink).truncate())
                .child(text(state).font(ty::TILE_STATE).color(t.ink_faint).truncate()),
        )
        .child(icon(&icons::CHEVRON, 12.0).color(t.ink_faint))
        .on_press(|s: &mut App| s.checkout_menu = !s.checkout_menu);
    for sh in t.shadow_raised() {
        c = c.box_shadow(sh);
    }
    c
}

/// A checkout's state in a few marks: `clean · ↑4`, `±3`, `2 conflicted · ↓1`.
fn compact(w: &Worktree) -> String {
    let mut parts = vec![if w.conflicted > 0 {
        format!("{} conflicted", w.conflicted)
    } else if w.changed > 0 {
        format!("±{}", w.changed)
    } else {
        "clean".to_owned()
    }];
    if w.unpushed > 0 {
        parts.push(format!("↑{}", w.unpushed));
    }
    if w.behind > 0 {
        parts.push(format!("↓{}", w.behind));
    }
    parts.join(" · ")
}

/// The open menu over the window, under the control. Escape or a click outside closes it.
pub fn menu(s: &App, t: &Theme) -> Vec<El> {
    let Some(work) = checkouts(s) else { return Vec::new() };
    let here = s.checkout_path();
    let pulls = work.open.as_ref().map(|o| o.pulls.as_slice()).unwrap_or_default();
    let item = |w: &Worktree| {
        let number = w.branch.as_deref().and_then(|b| pulls.iter().find(|p| p.summary.head_ref == b));
        let conflicts = number.is_some_and(|p| work.conflict(p.summary.number).is_some());
        let tone = if w.main { Tone::Quiet } else { w.tone(conflicts) };
        let on = here.as_ref() == Some(&w.path);
        let mut second = w.branch.clone().unwrap_or_else(|| "detached".to_owned());
        if w.main {
            second = s.env.tilde(&w.path);
        } else if let Some(p) = number {
            second.push_str(&format!(" · #{}", p.summary.number));
        }
        let glyph = if w.main {
            let (fg, bg) = if on { (t.on_accent, t.accent) } else { (t.accent, t.accent_soft) };
            row()
                .size(24.0, 24.0)
                .radius(size::RADIUS_SM)
                .center()
                .fill(bg)
                .child(icon(&icons::WORKTREE, 14.0).color(fg))
        } else {
            row().size(24.0, 24.0).center().child(icon(&icons::WORKTREE, 14.0).color(t.tone(tone).0))
        };
        let mut el = row()
            .key(keys::checkout_item(&w.name))
            .role("menuitem")
            .w_full()
            .px(9.0)
            .py(7.0)
            .gap(10.0)
            .center_y()
            .radius(size::RADIUS_SM)
            .child(glyph)
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .child(text(name_of(w)).font(ty::BODY_STRONG).color(t.ink).truncate())
                    .child(text(second).font(ty::MONO_11).color(t.ink_faint).truncate()),
            )
            .child(text(compact(w)).font(ty::CAPTION).color(t.ink_faint));
        if on {
            el = el.fill(t.accent_soft).child(icon(&icons::CHECK, 14.0).color(t.accent));
        } else {
            el = el.hover(t.control_hover);
        }
        let pick: Option<PathBuf> = (!w.main).then(|| w.path.clone());
        el.on_press(move |s: &mut App| s.pick_checkout(pick.clone()))
    };
    let caption = |words: &str| row().px(9.0).pt(6.0).pb(3.0).child(ui::label(t, words));
    let rule = || rune::el().w_full().h(1.0).my(5.0).fill(t.line);
    let soon = |key: &str, glyph, words: &str, next: &str| {
        row()
            .key(key.to_owned())
            .role("menuitem")
            .w_full()
            .px(9.0)
            .py(7.0)
            .gap(10.0)
            .center_y()
            .child(row().size(24.0, 24.0).center().child(icon(glyph, 14.0).color(t.ink_faint)))
            .child(text(words.to_owned()).font(ty::BODY).color(t.ink_faint))
            .help(next.to_owned())
            .disabled(true)
    };
    let mut menu = col()
        .key(keys::CHECKOUT_MENU)
        .role("menu")
        .label("Checkouts".to_owned())
        .w(size::CHECKOUT_MENU_W)
        .padding(6.0)
        .gap(1.0)
        .radius(size::RADIUS_MD)
        .fill(t.raised)
        .modal(true, |s: &mut App| s.checkout_menu = false);
    let (main, others): (Vec<&Worktree>, Vec<&Worktree>) = work.local.worktrees.iter().partition(|w| w.main);
    menu = menu.child(caption("Main checkout")).children(main.into_iter().map(item));
    if !others.is_empty() {
        menu = menu.child(rule()).child(caption("Worktrees")).children(others.into_iter().map(item));
    }
    menu = menu
        .child(rule())
        .child(soon(
            keys::CHECKOUT_NEW_WORKTREE,
            &icons::PLUS,
            "New worktree…",
            "Coming next: start a worktree for new work",
        ))
        .child(soon(
            keys::CHECKOUT_SWITCH_BRANCH,
            &icons::BRANCH,
            "Switch branch in main checkout…",
            "Coming next: switch the main checkout to another branch",
        ));
    for sh in t.shadow_overlay() {
        menu = menu.box_shadow(sh);
    }
    // Under the control, which starts 16 in from the toolbar's left.
    let left = if s.sidebar_open { super::SIDEBAR_W + 16.0 } else { 16.0 };
    vec![
        rune::el()
            .key("checkout-menu-layer")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .child(menu.absolute(left, size::TOOLBAR - 4.0)),
    ]
}
