//! A sheet over the window (Add, Clone): a 460-wide panel raised over a dimmed window, with a
//! title, what it's for, its fields, what went wrong, and Cancel beside the primary button.

use rune::{El, col, el, row, text};

use crate::keys;
use crate::sheet::{self, Event, Phase, Sheet};
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// The scrim and the panel, laid over the window; nothing while no sheet is open.
pub fn layer(s: &App, t: &Theme) -> Vec<El> {
    let f = &s.fields;
    let (title, about, fields, phase, confirm) = match &s.sheet {
        Sheet::Closed => return Vec::new(),
        Sheet::Add(phase) => (
            "Add a repository",
            "A repository already on this computer: its folder, or any folder inside it.",
            vec![ui::field(
                t,
                &f.path,
                "Folder",
                "~/code/project",
                keys::ADD_PATH,
                |s: &mut App| &mut s.fields.path,
                submit,
            )],
            phase,
            ("Add", keys::ADD_CONFIRM),
        ),
        Sheet::Clone(phase) => (
            "Clone a repository",
            "Paste its URL from GitHub, GitLab or Bitbucket, or type owner/name for GitHub. It clones over SSH.",
            vec![
                ui::field(
                    t,
                    &f.url,
                    "Repository",
                    "git@github.com:owner/name.git",
                    keys::CLONE_URL,
                    |s: &mut App| &mut s.fields.url,
                    submit,
                ),
                ui::field(t, &f.into, "Into", "~/code", keys::CLONE_INTO, |s: &mut App| &mut s.fields.into, submit),
                plan(s, t, phase),
            ],
            phase,
            ("Clone", keys::CLONE_CONFIRM),
        ),
    };
    let mut panel = col()
        .key(keys::SHEET)
        .role("dialog")
        .label(title.to_owned())
        .w(size::SHEET)
        .padding(20.0)
        .gap(16.0)
        .radius(size::RADIUS_LG)
        .fill(t.raised)
        .modal(true, |s: &mut App| s.sheet_event(Event::Dismiss, None))
        .child(
            col()
                .w_full()
                .gap(4.0)
                .child(text(title.to_owned()).font(ty::TITLE).color(t.ink))
                .child(text(about.to_owned()).font(ty::CAPTION).color(t.ink_muted).wrap()),
        )
        .children(fields);
    for sh in t.shadow_overlay() {
        panel = panel.box_shadow(sh);
    }
    if let Phase::Editing { error: Some(e) } = phase {
        panel = panel.child(text(e.clone()).key(keys::SHEET_ERROR).font(ty::CAPTION).color(t.removed).wrap());
    }
    panel = panel.child(buttons(t, phase, confirm));
    vec![
        el().key("sheet-scrim").absolute(0.0, 0.0).w_full().h_full().fill(t.scrim).a11y_hidden(),
        el().key("sheet-layer").absolute(0.0, 0.0).w_full().h_full().padding(16.0).center().child(panel),
    ]
}

/// Return in a field does what the primary button does.
fn submit(field: El) -> El {
    field.on_submit(|s: &mut App, tx| s.sheet_event(Event::Submit, Some(tx)))
}

/// Cancel (it stops a clone that's running), then the primary button, held down while it works.
fn buttons(t: &Theme, phase: &Phase, (label, key): (&str, &str)) -> El {
    let working = !phase.editing();
    let cancel = ui::button(t, Variant::Secondary, None, "Cancel", None).key(keys::SHEET_CANCEL);
    let cancel = match phase {
        Phase::Cancelling => cancel.disabled(true).opacity(0.6),
        _ => cancel.on_press(|s: &mut App| s.sheet_event(Event::Dismiss, None)),
    };
    let confirm = ui::button_in(t, Variant::Primary, None, label, None, working).key(key);
    let confirm = if working {
        confirm.disabled(true)
    } else {
        confirm.on_press_tx(|s: &mut App, tx| s.sheet_event(Event::Submit, Some(tx)))
    };
    row().w_full().gap(8.0).justify_end().child(cancel).child(confirm)
}

/// Where the clone comes from and where it lands, as the fields stand; git's progress while it
/// runs.
fn plan(s: &App, t: &Theme, phase: &Phase) -> El {
    let line = |label: &str, value: String, ink| {
        row()
            .w_full()
            .gap(8.0)
            .child(text(label.to_owned()).font(ty::CAPTION).color(t.ink_faint).w(36.0))
            .child(text(value).font(ty::MONO).color(ink).truncate().grow(1.0).min_w(0.0))
    };
    let mut box_ = col().key(keys::CLONE_PLAN).w_full().gap(2.0).padding(10.0).radius(size::RADIUS_SM).fill(t.canvas);
    match sheet::plan(&s.env, &s.fields) {
        Some((source, dest)) => {
            box_ = box_.child(line("From", source.url, t.ink)).child(line("To", s.env.tilde(&dest), t.ink));
        }
        None => {
            box_ = box_.child(text("Where it clones from and to shows here").font(ty::CAPTION).color(t.ink_faint));
        }
    }
    if !phase.editing() {
        let said = match (phase, &s.sheet_jobs.shown) {
            (Phase::Cancelling, _) => "Cancelling…".to_owned(),
            (_, Some(progress)) => progress.clone(),
            (_, None) => "Connecting…".to_owned(),
        };
        box_ = box_.child(line("", said, t.ink_muted).mt(4.0));
    }
    box_
}
