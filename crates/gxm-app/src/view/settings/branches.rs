//! Branches: the default branch, deleting branches once their pull requests merge (on the
//! provider, and the local copy too), forgetting deleted remote branches, and tidying up the
//! branches already merged.

use gxm_core::forge::admin::{Change, Role, Write};
use gxm_core::local::LocalChange;
use gxm_core::tidy::Branch;
use rune::{El, icon, row, text};

use super::parts::{self, Place};
use crate::icons;
use crate::keys::{setting, setting_item};
use crate::settings::{Action, What};
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

pub fn branches(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let provider = s.settings_jobs.provider();
    let full = s.settings_jobs.admin.as_ref().map(|a| a.full_name()).unwrap_or_default();
    let can = m.admin();
    let mut out = Vec::new();
    if let Some(r) = m.repo() {
        let mut chips = row().w_full().gap(6.0).center_y().flex_wrap();
        let mut names: Vec<&String> = m.remote_branches().iter().collect();
        if !names.contains(&&r.default_branch) {
            names.insert(0, &r.default_branch);
        }
        for b in names {
            chips = chips.child(branch_chip(t, b, *b == r.default_branch, can));
        }
        out.push(parts::group(
            t,
            "Default branch",
            vec![
                row()
                    .w_full()
                    .gap(12.0)
                    .child(chips.grow(1.0).min_w(0.0))
                    .child(parts::tag(t, Place::Provider(provider))),
                parts::note(t, "Pull requests merge into it, and clones start on it."),
            ],
        ));
    }
    let mut after = Vec::new();
    if let Some(r) = m.repo() {
        after.push(parts::toggle(
            t,
            "delete-branch-on-merge",
            r.delete_branch_on_merge,
            &format!("Delete a branch on {provider} when its pull request merges"),
            Some(&format!("Covers every merge into {full}, from any app or the website.")),
            Some(Place::Provider(provider)),
            can,
            |v| Action::Forge(Write::Change(Change::DeleteBranchOnMerge(v))),
        ));
    }
    let local = m.local();
    after.push(parts::toggle(
        t,
        "delete-local-merged",
        local.is_some_and(|l| l.delete_merged),
        "Delete my local copy too",
        Some("When a merge you make here lands, if nothing on the branch is missing from the merge."),
        Some(Place::Here),
        local.is_some(),
        |v| Action::Local(LocalChange::DeleteMerged(v)),
    ));
    out.push(parts::group(t, "After a pull request merges", after));
    out.push(parts::group(
        t,
        "When fetching",
        vec![parts::toggle(
            t,
            "fetch-prune",
            local.is_some_and(|l| l.fetch_prune),
            "Forget remote branches that were deleted",
            Some(&format!("A fetch drops origin/… branches {provider} no longer has (git’s fetch.prune).")),
            Some(Place::Here),
            local.is_some(),
            |v| Action::Local(LocalChange::FetchPrune(v)),
        )],
    ));
    out.push(parts::group(t, "Merged branches", tidy(s, t, provider)));
    out
}

/// A branch to make the default, or the default.
fn branch_chip(t: &Theme, name: &str, selected: bool, can: bool) -> El {
    let mut c = row()
        .key(setting_item("default-branch", name))
        .role("button")
        .label(name.to_owned())
        .h(24.0)
        .px(10.0)
        .gap(5.0)
        .center_y()
        .radius(size::PILL)
        .fill(if selected { t.accent_soft } else { t.control_hover })
        .child(icon(&icons::BRANCH, 11.0).color(if selected { t.accent } else { t.ink_muted }))
        .child(text(name.to_owned()).font(ty::CONTROL).color(if selected { t.accent } else { t.ink }));
    if selected || !can {
        if !selected {
            c = c.disabled(true).opacity(0.5);
        }
        return c;
    }
    let to = name.to_owned();
    c = c
        .hover(t.control)
        .on_press(move |s: &mut App| s.settings_do(Action::Forge(Write::Change(Change::DefaultBranch(to.clone())))));
    c
}

/// The branches that are done, ticked to delete; the ones that stay, folded, each with why.
fn tidy(s: &App, t: &Theme, provider: &str) -> Vec<El> {
    let m = &s.settings;
    let Some(all) = m.branches() else { return super::waiting(s, t, What::Branches) };
    let (done, kept): (Vec<&Branch>, Vec<&Branch>) = all.iter().partition(|b| b.done());
    let mut rows: Vec<El> = Vec::new();
    rows.push(
        row()
            .gap(8.0)
            .center_y()
            .child(text("Ready to delete").font(ty::BODY_STRONG).color(t.ink))
            .child(ui::tab_count(t, done.len()))
            .child(rune::el().grow(1.0))
            .child(parts::note(t, "Their work is in the default branch")),
    );
    if done.is_empty() {
        rows.push(parts::note(t, "Nothing to tidy up: every branch has work the default branch doesn't."));
    }
    for b in &done {
        let ticked = !m.untick.contains(&b.name);
        let name = b.name.clone();
        rows.push(
            parts::item("branch", &b.name)
                .role("checkbox")
                .label(b.name.clone())
                .child(ui::check_box(t, ticked))
                .child(parts::named(t, &b.name, &format!("{} · {}", b.place(), b.reason()), true))
                .on_press(move |s: &mut App| {
                    if !s.settings.untick.remove(&name) {
                        s.settings.untick.insert(name.clone());
                    }
                }),
        );
    }
    let show = m.show_kept;
    rows.push(
        row()
            .key(setting("show-kept"))
            .role("button")
            .label("Kept".to_owned())
            .gap(8.0)
            .center_y()
            .child(icon(if show { &icons::CHEVRON } else { &icons::CHEVRON_RIGHT }, 12.0).color(t.ink_muted))
            .child(text("Kept").font(ty::BODY_STRONG).color(t.ink_muted))
            .child(ui::tab_count(t, kept.len()))
            .child(rune::el().grow(1.0))
            .child(parts::note(t, "Work that's nowhere else, or still under review"))
            .on_press(|s: &mut App| s.settings.show_kept = !s.settings.show_kept),
    );
    if show {
        for b in &kept {
            rows.push(parts::item("branch", &b.name).opacity(0.75).child(row().w(16.0)).child(parts::named(
                t,
                &b.name,
                &format!("{} · {}", b.place(), b.reason()),
                true,
            )));
        }
    }
    let ticked = m.ticked();
    let (remote, here) =
        (ticked.iter().filter(|b| b.remote.is_some()).count(), ticked.iter().filter(|b| b.local.is_some()).count());
    // Deleting on the provider needs push access there.
    let can_push = m.repo().is_some_and(|r| r.role >= Role::Write) || remote == 0;
    let label = match ticked.len() {
        1 => "Delete 1 branch".to_owned(),
        n => format!("Delete {n} branches"),
    };
    let mut foot = vec![parts::button(
        t,
        "delete-branches",
        &label,
        Variant::Primary,
        !ticked.is_empty() && can_push,
        move |s: &mut App| {
            let ticked = s.settings.ticked();
            s.settings_do(Action::DeleteBranches(ticked));
        },
    )];
    if !ticked.is_empty() {
        foot.push(parts::note(t, &format!("{remote} on {provider} and {here} on this computer")));
    }
    rows.push(parts::controls(foot));
    vec![parts::list(t, rows)]
}
