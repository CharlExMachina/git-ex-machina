//! Updates on screen: the button at the sidebar's foot (Update to …, the download's progress,
//! Restart to update), and the sheet that offers the restart, with the release's notes.

use rune::{El, ScrollSpec, col, el, row, text};

use crate::icons;
use crate::keys;
use crate::sheet::Event;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};
use crate::update::{Note, Update, UpdateEvent, notes};

/// The update button, and under it the download's progress or why it failed; nothing while
/// there's no update.
pub fn control(s: &App, t: &Theme) -> Option<El> {
    let take = |b: El| b.on_press_tx(|s: &mut App, tx| s.update_event(UpdateEvent::Take, Some(tx)));
    let (button, under) = match &s.update {
        Update::Quiet => return None,
        Update::Available(r) => (
            take(ui::button(t, Variant::Secondary, Some(&icons::DOWNLOAD), &format!("Update to {}", r.version), None))
                .help(format!("Download git-ex-machina {}; it takes over from the next start", r.version)),
            None,
        ),
        Update::Downloading { release, done, total } => {
            let frac = if *total == 0 { 0.0 } else { (*done as f32 / *total as f32).clamp(0.0, 1.0) };
            let label = format!("Downloading {}… {}%", release.version, (frac * 100.0).floor());
            let bar = row()
                .w_full()
                .h(3.0)
                .radius(size::PILL)
                .fill(t.line)
                .child(el().w_frac(frac).h_full().radius(size::PILL).fill(t.accent));
            (ui::button_in(t, Variant::Secondary, Some(&icons::DOWNLOAD), &label, None, true).disabled(true), Some(bar))
        }
        Update::Failed { why, .. } => (
            take(ui::button(t, Variant::Secondary, Some(&icons::RESTART), "Retry update", None))
                .help("Download the update again"),
            Some(text(why.clone()).key(keys::UPDATE_ERROR).font(ty::CAPTION).color(t.removed).wrap()),
        ),
        Update::Ready(r) => (
            take(ui::button(t, Variant::Primary, Some(&icons::RESTART), "Restart to update", None))
                .help(format!("git-ex-machina {} is installed; restart to use it", r.version)),
            None,
        ),
        Update::Restarting(_) => {
            (ui::button_in(t, Variant::Primary, Some(&icons::RESTART), "Restarting…", None, true).disabled(true), None)
        }
    };
    let mut c = col().w_full().gap(6.0).child(row().child(button.key(keys::UPDATE)));
    if let Some(under) = under {
        c = c.child(under);
    }
    Some(c)
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
