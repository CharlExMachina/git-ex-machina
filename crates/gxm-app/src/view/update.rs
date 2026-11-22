//! Updates on screen: the icon button at the sidebar's foot (check for one, download it, its
//! progress, restart into it), and the sheet that offers the restart, with the release's notes.

use rune::{El, ScrollSpec, col, el, row, text};

use crate::icons;
use crate::keys;
use crate::sheet::Event;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};
use crate::update::{Note, Update, UpdateEvent, notes};

/// The update's icon button, and what goes under its row: the download's progress, or why the
/// download failed. Nothing when the app can't update itself (it isn't an AppImage).
pub fn control(s: &App, t: &Theme) -> Option<(El, Option<El>)> {
    s.env.install.appimage.as_ref()?;
    let version = &s.env.install.version;
    let check = |b: El| b.on_press_tx(|s: &mut App, tx| s.update_event(UpdateEvent::Check, Some(tx)));
    let take = |b: El| b.on_press_tx(|s: &mut App, tx| s.update_event(UpdateEvent::Take, Some(tx)));
    let icon = |variant, glyph, ink, label: &str| ui::icon_button(t, variant, glyph, ink, label, false);
    let (button, under) = match &s.update {
        Update::Idle => (check(icon(Variant::Ghost, &icons::REFRESH, None, "Check for updates")), None),
        Update::Checking { .. } => {
            (icon(Variant::Ghost, &icons::REFRESH, Some(t.ink_faint), "Checking for updates…").disabled(true), None)
        }
        Update::UpToDate => (
            check(icon(
                Variant::Ghost,
                &icons::CHECK,
                Some(t.added),
                &format!("Up to date: git-ex-machina {version} is the latest. Check again"),
            )),
            None,
        ),
        Update::CheckFailed(why) => (
            check(icon(
                Variant::Ghost,
                &icons::REFRESH,
                Some(t.removed),
                &format!("Couldn't check for updates: {why}"),
            )),
            None,
        ),
        Update::Available(r) => (
            take(icon(Variant::Secondary, &icons::DOWNLOAD, Some(t.accent), &format!("Update to {}", r.version))),
            None,
        ),
        Update::Downloading { release, done, total } => {
            let frac = if *total == 0 { 0.0 } else { (*done as f32 / *total as f32).clamp(0.0, 1.0) };
            let label = format!("Downloading {}… {}%", release.version, (frac * 100.0).floor());
            let bar = row()
                .w_full()
                .h(2.0)
                .radius(size::PILL)
                .fill(t.line)
                .child(el().w_frac(frac).h_full().radius(size::PILL).fill(t.accent));
            let button = ui::icon_button(t, Variant::Secondary, &icons::DOWNLOAD, Some(t.accent), &label, true);
            (button.disabled(true), Some(bar))
        }
        Update::Failed { why, .. } => (
            take(icon(Variant::Ghost, &icons::DOWNLOAD, Some(t.removed), "Retry update")),
            Some(text(why.clone()).key(keys::UPDATE_ERROR).font(ty::CAPTION).color(t.removed).wrap()),
        ),
        Update::Ready(r) => {
            (take(icon(Variant::Primary, &icons::RESTART, None, &format!("Restart to update to {}", r.version))), None)
        }
        Update::Restarting(_) => {
            (ui::icon_button(t, Variant::Primary, &icons::RESTART, None, "Restarting…", true).disabled(true), None)
        }
    };
    Some((button.key(keys::UPDATE), under))
}

/// The restart sheet's title, what it's for, body and buttons: the release's notes, and Later
/// beside Restart now.
pub fn sheet(s: &App, t: &Theme, error: Option<&str>) -> (String, String, Vec<El>, El) {
    let restarting = matches!(s.update, Update::Restarting(_));
    let Some(release) = s.update.release() else {
        return (String::new(), String::new(), Vec::new(), row());
    };
    let title = format!("git-ex-machina {} is ready", release.version);
    let about = "It's installed, and takes over from the next start. Restart now to use it; your repositories and settings stay as they are."
        .to_owned();
    let mut lines = col().w_full().gap(6.0);
    let written = notes(&release.notes);
    if written.is_empty() {
        lines = lines.child(text("This release has no notes.").font(ty::BODY).color(t.ink_muted));
    }
    for note in written {
        lines = lines.child(match note {
            Note::Heading(h) => text(h).font(ty::BODY_STRONG).color(t.ink).wrap().mt(4.0),
            Note::Text(p) => text(p).font(ty::BODY).color(t.ink_muted).wrap(),
            Note::Bullet(b) => row()
                .w_full()
                .gap(8.0)
                .child(text("•").font(ty::BODY).color(t.ink_faint))
                .child(text(b).font(ty::BODY).color(t.ink_muted).wrap().grow(1.0).min_w(0.0)),
        });
    }
    let page = release.page.clone();
    let heading = row()
        .w_full()
        .center_y()
        .child(text("Release notes").font(ty::CAPTION_STRONG).color(t.ink_muted))
        .child(el().grow(1.0))
        .child(ui::button(t, Variant::Ghost, None, "On GitHub", None).help("Open the release's page").on_press_tx(
            move |_: &mut App, tx| {
                // A page the browser refuses changes nothing here.
                tx.open_url(&page).ok();
            },
        ));
    let box_ = ui::well(t)
        .key(keys::UPDATE_NOTES)
        .w_full()
        .max_h(240.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(lines.padding(12.0));
    let mut body = vec![col().w_full().gap(6.0).child(heading).child(box_)];
    if let Some(e) = error {
        body.push(super::sheet::error(t, e));
    }
    let later = ui::button(t, Variant::Secondary, None, "Later", None)
        .key(keys::UPDATE_LATER)
        .help("Keep working; the next start runs the new version")
        .on_press(|s: &mut App| s.sheet_event(Event::Dismiss, None));
    let restart = ui::button_in(
        t,
        Variant::Primary,
        None,
        if restarting { "Restarting…" } else { "Restart now" },
        None,
        restarting,
    )
    .key(keys::UPDATE_RESTART);
    let restart = if restarting {
        restart.disabled(true)
    } else {
        restart.on_press_tx(|s: &mut App, tx| s.update_event(UpdateEvent::Restart, Some(tx)))
    };
    (title, about, body, row().w_full().gap(8.0).justify_end().child(later).child(restart))
}
