//! The repository sidebar: the traffic-light bar, the filter, pinned repositories, the rest,
//! and "Clone repository" at the foot.

use rune::{El, col, icon, row, text};

use super::bar;
use crate::icons;
use crate::keys;
use crate::sheet::Event;
use crate::state::{App, RepoEntry};
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// Room for the window's traffic lights, which macOS draws inside the window (three 12-point
/// lights 8 apart). Elsewhere the window manager draws its own title bar, so there's none.
pub const LIGHTS_W: f32 = if cfg!(target_os = "macos") { 52.0 } else { 0.0 };

pub fn sidebar(s: &App, t: &Theme) -> El {
    let mut top = bar(t).px(12.0);
    if LIGHTS_W > 0.0 {
        top = top.child(rune::el().w(LIGHTS_W).h(12.0).key("traffic-lights"));
    }
    let top = top
        .child(rune::el().grow(1.0))
        .child(
            ui::button(t, Variant::Ghost, Some(&icons::SIDEBAR), "", None)
                .key(keys::SIDEBAR_TOGGLE)
                .help("Hide the sidebar (⌃⌘S)")
                .on_press(|s: &mut App| s.sidebar_open = false),
        )
        .child(
            ui::button(t, Variant::Ghost, Some(&icons::PLUS), "", None)
                .key(keys::ADD_REPO)
                .help("Add a repository from this computer")
                .on_press_tx(|s: &mut App, tx| s.sheet_event(Event::OpenAdd, Some(tx))),
        );
    let filter = row().pt(12.0).px(8.0).child(
        ui::search_field(
            t,
            &s.filter,
            "Filter repositories",
            "⌘K",
            keys::REPO_FILTER,
            |s: &mut App| &mut s.filter,
            |f| f,
        )
        .grow(1.0),
    );
    let mut list = col().w_full().grow(1.0).min_h(0.0).child(top).child(filter);
    for (title, pinned) in [("Pinned", true), ("Repositories", false)] {
        let repos = s.visible_repos(pinned);
        if repos.is_empty() {
            continue;
        }
        list = list.child(row().pt(18.0).px(16.0).pb(8.0).child(ui::label(t, title)));
        let mut rows = col().px(8.0).key(format!("section-{}", title.to_lowercase()));
        for (i, r) in repos {
            rows = rows.child(repo_row(s, i, r, t));
        }
        list = list.child(rows);
    }
    let footer = row()
        .w_full()
        .padding(12.0)
        .gap(8.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], t.line)
        .border_layout()
        .child(
            ui::button(t, Variant::Ghost, Some(&icons::PLUS), "Clone repository", None)
                .key(keys::CLONE)
                .help("Clone a repository over SSH")
                .on_press_tx(|s: &mut App, tx| s.sheet_event(Event::OpenClone, Some(tx))),
        );
    col().fill(t.chrome).border_sides([0.0, 1.0, 0.0, 0.0], t.line).border_layout().child(list).child(footer)
}

/// `RepoRow`: 36 tall, 8 a side, radius 6, gap 10; the selected one raised.
fn repo_row(s: &App, i: usize, r: &RepoEntry, t: &Theme) -> El {
    let st = &r.summary.status;
    let lane = t.lane(i + 1);
    let branch = st.branch.as_deref().unwrap_or("detached");
    let meta = row()
        .gap(4.0)
        .center_y()
        .child(icon(&icons::BRANCH, 11.0).color(t.ink_muted))
        .child(text(branch.to_owned()).font(ty::REPO_META).color(t.ink_muted).truncate())
        .child(text(format!("· {}", r.summary.owner())).font(ty::REPO_META).color(t.ink_muted));
    let name = col()
        .grow(1.0)
        .min_w(0.0)
        .child(text(r.summary.name.clone()).font(ty::REPO_NAME).color(t.ink).truncate())
        .child(meta);
    let mut side = row().gap(6.0).center_y();
    if st.ahead > 0 {
        side = side.child(
            text(format!("↑{}", st.ahead)).font(ty::MONO_11).color(t.ink_muted).help(format!("{} to push", st.ahead)),
        );
    }
    if st.behind > 0 {
        side = side.child(
            text(format!("↓{}", st.behind)).font(ty::MONO_11).color(t.ink_muted).help(format!("{} to pull", st.behind)),
        );
    }
    if st.changed > 0 {
        side = side.child(ui::changes(t, st.changed));
    }
    let mut row_el = row()
        .key(keys::repo(&r.summary.name))
        .h(size::ROW_REPO)
        .px(8.0)
        .gap(10.0)
        .center_y()
        .radius(size::RADIUS_SM)
        .child(ui::tile(t, &r.summary.name, lane))
        .child(name)
        .child(side)
        .on_press(move |s: &mut App| s.select_repo(i));
    if s.selected_repo == Some(i) {
        row_el = row_el.fill(t.raised);
        for sh in t.shadow_lifted() {
            row_el = row_el.box_shadow(sh);
        }
    } else {
        row_el = row_el.hover(t.control_hover);
    }
    row_el
}
