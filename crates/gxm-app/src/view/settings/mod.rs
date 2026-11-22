//! The repository settings dialog: 960 by 720 over the dimmed window, its sections down a
//! 224-wide sidebar, the open section's settings to the right under its title and the word on
//! the last change.

mod access;
mod automation;
mod branches;
mod local;
mod pages;
mod parts;
mod repo;
mod security;

use rune::{El, ScrollSpec, col, el, icon, row, text};

use crate::icons;
use crate::keys;
use crate::settings::{Event, Section, Status};
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// The scrim and the dialog, laid over the window; nothing while it's closed.
pub fn layer(s: &App, t: &Theme) -> Vec<El> {
    if !s.settings.open {
        return Vec::new();
    }
    let mut panel = row()
        .key(keys::SETTINGS_DIALOG)
        .role("dialog")
        .label("Repository settings".to_owned())
        .w(size::SETTINGS_W)
        .h(size::SETTINGS_H)
        .radius(size::RADIUS_LG)
        .fill(t.raised)
        .clip_rounded(size::RADIUS_LG)
        .modal(true, |s: &mut App| s.settings_event(Event::Close))
        .child(nav(s, t))
        .child(pane(s, t));
    for sh in t.shadow_overlay() {
        panel = panel.box_shadow(sh);
    }
    vec![
        el().key("settings-scrim").absolute(0.0, 0.0).w_full().h_full().fill(t.scrim).a11y_hidden(),
        el().key("settings-layer").absolute(0.0, 0.0).w_full().h_full().padding(16.0).center().child(panel),
    ]
}

/// The sidebar: the repository, then every section in its group, the danger zone apart.
fn nav(s: &App, t: &Theme) -> El {
    let (name, full) = match s.repo() {
        Some(r) => (
            r.summary.name.clone(),
            s.settings_jobs.admin.as_ref().map_or_else(|| s.env.tilde(&r.summary.path), |a| a.full_name()),
        ),
        None => (String::new(), String::new()),
    };
    let lane = t.lane(s.selected_repo.unwrap_or(0) + 1);
    let mut nav = col()
        .w(size::SETTINGS_NAV)
        .h_full()
        .px(10.0)
        .py(14.0)
        .gap(1.0)
        .fill(t.canvas)
        .border_sides([0.0, 1.0, 0.0, 0.0], t.line)
        .child(
            row().w_full().px(10.0).pb(6.0).gap(10.0).center_y().child(ui::tile(t, &name, lane)).child(
                col()
                    .min_w(0.0)
                    .grow(1.0)
                    .child(text(name).font(ty::REPO_NAME).color(t.ink).truncate())
                    .child(text(full).font(ty::REPO_META).color(t.ink_muted).truncate()),
            ),
        );
    for section in Section::ALL {
        if section == Section::DangerZone {
            nav = nav.child(el().grow(1.0)).child(el().w_full().h(1.0).my(8.0).fill(t.line));
        } else if let Some(g) = section.group() {
            nav = nav.child(row().px(10.0).pt(12.0).pb(4.0).child(ui::label(t, g)));
        }
        nav = nav.child(nav_item(s, t, section));
    }
    nav
}

fn nav_item(s: &App, t: &Theme, section: Section) -> El {
    let selected = s.settings.section == section;
    let danger = section == Section::DangerZone;
    let ink = match (selected, danger) {
        (_, true) => t.removed,
        (true, false) => t.ink,
        (false, false) => t.ink_muted,
    };
    let mut item = row()
        .key(keys::settings_section(section.title()))
        .role("button")
        .label(section.title().to_owned())
        .w_full()
        .h(26.0)
        .px(10.0)
        .center_y()
        .radius(size::RADIUS_SM)
        .child(text(section.title().to_owned()).font(ty::BODY).color(ink));
    if selected {
        item = item.fill(if danger { t.removed_soft } else { t.control });
        for sh in t.shadow_raised() {
            item = item.box_shadow(sh);
        }
    } else {
        item = item.hover(t.control_hover);
    }
    // A provider the app doesn't change settings on yet: its sections wait, saying so.
    if section.on_provider()
        && let Some(gxm_core::forge::NotConnected::NotYet(p)) = &s.settings_jobs.not_connected
    {
        return item.disabled(true).opacity(0.45).help(format!("{}'s settings are coming next", p.name()));
    }
    if section.on_provider() && s.settings_jobs.admin.is_none() {
        return item.disabled(true).opacity(0.45);
    }
    item.on_press(move |s: &mut App| s.settings_event(Event::Pick(section)))
}

/// The open section: its title and the word on the last change, why some of it is locked, and
/// its settings, scrolling.
fn pane(s: &App, t: &Theme) -> El {
    let m = &s.settings;
    let status = match &m.status {
        Status::Idle => row(),
        Status::Saving => row().child(text("Saving…").font(ty::CAPTION).color(t.ink_muted)),
        Status::Saved(said) => row()
            .gap(5.0)
            .center_y()
            .child(icon(&icons::CHECK, 12.0).color(t.added))
            .child(text(said.clone()).font(ty::CAPTION).color(t.added)),
        Status::Failed(why) => {
            row().max_w(360.0).child(text(why.clone()).font(ty::CAPTION).color(t.removed).truncate().help(why.clone()))
        }
    };
    let close = ui::button(t, Variant::Ghost, Some(&icons::X), "", None)
        .key(keys::SETTINGS_CLOSE)
        .label("Close".to_owned())
        .help("Close (Esc)")
        .on_press(|s: &mut App| s.settings_event(Event::Close));
    let head = row()
        .w_full()
        .h(size::TOOLBAR + 12.0)
        .pl(28.0)
        .pr(14.0)
        .gap(10.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout()
        .child(text(m.section.title().to_owned()).font(ty::DISPLAY_SM).color(t.ink).grow(1.0))
        .child(status.key(keys::SETTINGS_STATUS))
        .child(close);
    let mut page = col().key(keys::SETTINGS_PAGE).w_full().px(28.0).pt(20.0).pb(28.0).gap(22.0);
    if let Some(why) = m.locked(&s.settings_jobs).filter(|_| m.section.on_provider() || s.settings_jobs.admin.is_none())
    {
        page = page.child(notice(t, &why));
    }
    page = page.children(section(s, t));
    col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .child(head)
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page))
}

/// Why some settings are locked, in a card above them.
fn notice(t: &Theme, why: &str) -> El {
    row()
        .key(keys::SETTINGS_NOTICE)
        .w_full()
        .gap(10.0)
        .padding(12.0)
        .radius(size::RADIUS_LG)
        .fill(t.canvas)
        .box_shadow(Theme::ring(1.0, t.line))
        .child(icon(&icons::LOCK, 14.0).color(t.ink_muted))
        .child(text(why.to_owned()).font(ty::CAPTION).color(t.ink_muted).wrap().grow(1.0).min_w(0.0))
}

fn section(s: &App, t: &Theme) -> Vec<El> {
    // A provider section without a provider shows only why (above).
    if s.settings.section.on_provider() && s.settings_jobs.admin.is_none() {
        return Vec::new();
    }
    match s.settings.section {
        Section::General => repo::general(s, t),
        Section::Branches => branches::branches(s, t),
        Section::PullRequests => repo::pull_requests(s, t),
        Section::Rules => access::rules(s, t),
        Section::People => access::people(s, t),
        Section::Actions => automation::actions(s, t),
        Section::Secrets => automation::secrets(s, t),
        Section::Environments => automation::environments(s, t),
        Section::Webhooks => automation::webhooks(s, t),
        Section::Pages => pages::pages(s, t),
        Section::CodeSecurity => security::code_security(s, t),
        Section::DeployKeys => security::deploy_keys(s, t),
        Section::Autolinks => security::autolinks(s, t),
        Section::Remotes => local::remotes(s, t),
        Section::Identity => local::identity(s, t),
        Section::LargeFiles => local::large_files(s, t),
        Section::DangerZone => repo::danger_zone(s, t),
    }
}

/// What a section says while what it shows is still being read, or why it couldn't be.
pub(super) fn waiting(s: &App, t: &Theme, what: crate::settings::What) -> Vec<El> {
    match s.settings.errors.get(&what) {
        Some(e) => vec![text(e.clone()).font(ty::CAPTION).color(t.removed).wrap()],
        None => vec![text("Reading…").font(ty::CAPTION).color(t.ink_faint)],
    }
}
