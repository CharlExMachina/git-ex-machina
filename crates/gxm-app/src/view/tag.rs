//! Tagging: the toolbar's button for the selected commit, and the tag window it opens. The
//! window shows the commit it tags, the tag's name (the next version, where there are versions),
//! its message or release notes (taking the height the window has), Push to origin, what pushing
//! starts where the repository releases from tags, and why the commit may not be the one to tag.

use rune::{Cx, El, col, el, icon, row, text, text_input};

use gxm_core::release::NotesFrom;

use crate::icons;
use crate::keys;
use crate::state::App;
use crate::tag::{TagEvent, TagSheet};
use crate::tag_window::TagWindow;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// The toolbar's button for the selected commit: "Tag a release" where pushing a tag releases.
pub fn button(s: &App, t: &Theme, commit: &str) -> El {
    let releases = s.history.as_ref().is_some_and(|h| h.tagging.trigger.is_some());
    let label = if releases { "Tag a release" } else { "Create tag" };
    let id = commit.to_owned();
    ui::button(t, Variant::Secondary, Some(&icons::TAG), label, None)
        .key(keys::CREATE_TAG)
        .on_press(move |s: &mut App| s.open_tag(&id))
}

/// The tag window: its title bar, the form, and Cancel beside the primary button at the foot.
pub fn window(s: &TagWindow, cx: &mut Cx<TagWindow>) -> El {
    let t = Theme::new(cx.system_dark());
    let root = col().key(keys::TAG_WINDOW).w_full().h_full().fill(t.raised);
    let Some(tag) = &s.sheet else { return root };
    // The release notes' caret, for their popup and their rows; read only where there are notes,
    // as reading it runs the view again whenever the caret moves.
    let caret = tag.writes_notes().then(|| cx.caret(keys::NOTES)).flatten();
    let title = if tag.opening.tagging.trigger.is_some() { "Tag a release" } else { "Create a tag" };
    let bar =
        super::bar(&t).justify_center().child(text(title).key(keys::TAG_TITLE).font(ty::BODY_STRONG).color(t.ink));
    let body = col().w_full().grow(1.0).min_h(0.0).px(20.0).pt(16.0).gap(16.0).children(form(s, &t, tag, caret));
    let foot = row().w_full().padding(20.0).child(buttons(s, &t, tag));
    root.label(title.to_owned()).child(bar).child(body).child(foot).children(super::picker::layer(s, &t, caret))
}

/// The form: what it's for, the commit, the name, the message or notes (which grow), Push, and
/// what pushing starts, why the commit may be odd to tag, and what went wrong.
fn form(s: &TagWindow, t: &Theme, tag: &TagSheet, caret: Option<[f32; 4]>) -> Vec<El> {
    let f = &s.tag_fields;
    let trigger = tag.opening.tagging.trigger.as_ref();
    let remote = tag.opening.tagging.remote.as_deref();
    let about = match trigger {
        Some(_) => "A tag names this commit for good. Pushing a version tag releases it.",
        None => "A tag names this commit for good: a version, a milestone, a build to come back to.",
    };
    let commit = row()
        .key(keys::TAG_COMMIT)
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .radius(size::RADIUS_SM)
        .fill(t.canvas)
        .child(text(tag.opening.short.clone()).font(ty::MONO).color(t.ink_faint))
        .child(text(tag.opening.subject.clone()).font(ty::BODY).color(t.ink).truncate().grow(1.0).min_w(0.0));
    let name = ui::field(
        t,
        &f.name,
        "Name",
        "v1.0.0",
        keys::TAG_NAME,
        |s: &mut TagWindow| &mut s.tag_fields.name,
        |e| {
            e.on_submit(|s: &mut TagWindow, tx| s.tag_event(TagEvent::Submit, Some(tx)))
                .on_input_change(|s: &mut TagWindow, _| s.tag_event(TagEvent::NameEdited, None))
        },
    );
    let push = match remote {
        Some(remote) => ui::check(t, f.push, &format!("Push to {remote}"))
            .on_press(|s: &mut TagWindow| s.tag_event(TagEvent::TogglePush, None)),
        None => {
            ui::check(t, false, "Push").disabled(true).opacity(0.5).help("This repository has no remote to push to")
        }
    };
    let words = if tag.writes_notes() { super::notes::editor(s, t, tag, caret) } else { message(t, f) };
    let mut form =
        vec![text(about).font(ty::CAPTION).color(t.ink_muted).wrap(), commit, name, words, push.key(keys::TAG_PUSH)];
    if let Some(trigger) = trigger {
        let name = f.name.text.trim();
        let name = if name.is_empty() { "the tag" } else { name };
        let (said, releases) = match remote {
            _ if !trigger.runs_for(name) && name != "the tag" => (
                format!("{name} doesn't match {}, so it won't start {}.", trigger.patterns.join(" or "), trigger.what),
                false,
            ),
            Some(remote) if f.push => {
                let notes = match &trigger.notes {
                    NotesFrom::Tag => ", with these notes".to_owned(),
                    NotesFrom::File(file) => format!(", with the notes in {}", ref_name(file, name)),
                    NotesFrom::Generated => ", with notes GitHub writes from the pull requests".to_owned(),
                    NotesFrom::Unknown => String::new(),
                };
                (format!("Pushing {name} to {remote} starts {} ({}){notes}.", trigger.what, trigger.file_name()), true)
            }
            _ => (format!("{name} stays on this computer, so it won't start {} yet.", trigger.what), false),
        };
        let ink = if releases { t.ink } else { t.ink_muted };
        form.push(
            row()
                .key(keys::TAG_RELEASE)
                .w_full()
                .center_y()
                .gap(8.0)
                .padding(10.0)
                .radius(size::RADIUS_SM)
                .fill(t.canvas)
                .child(icon(&icons::PUSH, 14.0).color(if releases { t.accent } else { t.ink_faint }))
                .child(text(said).font(ty::CAPTION).color(ink).wrap().grow(1.0).min_w(0.0)),
        );
    }
    let warnings = tag.warnings(f.push);
    if !warnings.is_empty() {
        let mut box_ = col().key(keys::TAG_WARNING).w_full().gap(2.0);
        for w in warnings {
            box_ = box_.child(text(w).font(ty::CAPTION).color(t.accent).wrap());
        }
        form.push(box_);
    }
    if let crate::sheet::Phase::Editing { error: Some(e) } = &tag.phase {
        form.push(text(e.clone()).key(keys::TAG_ERROR).font(ty::CAPTION).color(t.removed).wrap());
    }
    form
}

/// The tag's message, for a repository that doesn't release from tags: as many lines as it
/// takes, in the height the window has.
fn message(t: &Theme, f: &crate::tag::TagFields) -> El {
    let area = text_input(&f.message, ty::BODY)
        .key(keys::TAG_MESSAGE)
        .label("Message".to_owned())
        .on_edit(|s: &mut TagWindow| &mut s.tag_fields.message)
        .placeholder("What it is, for the tag's notes (optional)", t.ink_faint)
        .color(t.ink)
        .w_full()
        .min_w(0.0);
    let mut well = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .padding(10.0)
        .radius(size::RADIUS_SM)
        .fill(t.well)
        .role("textbox")
        .child(area);
    for sh in t.shadow_well() {
        well = well.box_shadow(sh);
    }
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(6.0)
        .child(text("Message").font(ty::CAPTION_STRONG).color(t.ink_muted))
        .child(well)
}

/// A workflow's path with the tag's name where it names the tag (`$GITHUB_REF_NAME`).
fn ref_name(path: &str, tag: &str) -> String {
    ["${{ github.ref_name }}", "${GITHUB_REF_NAME}", "$GITHUB_REF_NAME"]
        .iter()
        .fold(path.to_owned(), |p, v| p.replace(v, tag))
}

/// Cancel (it throws away what was written), then the primary button, held down while git
/// works.
fn buttons(s: &TagWindow, t: &Theme, tag: &TagSheet) -> El {
    let working = !tag.phase.editing();
    let pushing = s.tag_fields.push && tag.opening.tagging.remote.is_some();
    let label = match (working, pushing) {
        (true, true) => "Pushing…",
        (true, false) => "Tagging…",
        (false, true) => "Tag and push",
        (false, false) => "Create tag",
    };
    let cancel = ui::button(t, Variant::Secondary, None, "Cancel", None).key(keys::TAG_CANCEL);
    let cancel = if working {
        cancel.disabled(true).opacity(0.6)
    } else {
        cancel.on_press_tx(|s: &mut TagWindow, tx| s.tag_event(TagEvent::Cancel, Some(tx)))
    };
    let confirm = ui::button_in(t, Variant::Primary, Some(&icons::TAG), label, None, working).key(keys::TAG_CONFIRM);
    let confirm = if working {
        confirm.disabled(true)
    } else {
        confirm.on_press_tx(|s: &mut TagWindow, tx| s.tag_event(TagEvent::Submit, Some(tx)))
    };
    row().w_full().gap(8.0).justify_end().child(el().grow(1.0)).child(cancel).child(confirm)
}
