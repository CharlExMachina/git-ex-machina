//! The tag sheet: the commit it tags, the tag's name (the next version, where there are
//! versions) and message, Push to origin, what pushing starts where the repository releases from
//! tags, and why the commit may not be the one to tag.

use rune::{El, col, icon, row, text};

use gxm_core::release::NotesFrom;

use crate::icons;
use crate::keys;
use crate::sheet::Event;
use crate::state::{App, Subject};
use crate::tag::{TagEvent, TagSheet};
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// The toolbar's button for the selected commit: "Tag a release" where pushing a tag releases.
pub fn button(s: &App, t: &Theme, commit: &str) -> El {
    let releases = s.history.as_ref().is_some_and(|h| h.tagging.trigger.is_some());
    let label = if releases { "Tag a release" } else { "Create tag" };
    let id = commit.to_owned();
    ui::button(t, Variant::Secondary, Some(&icons::TAG), label, None)
        .key(keys::CREATE_TAG)
        .on_press_tx(move |s: &mut App, tx| s.tag_event(TagEvent::Open(id.clone()), Some(tx)))
}

/// The sheet's title, what it's for, its body, its buttons and its width: wide when it holds
/// release notes to write.
pub fn sheet(s: &App, t: &Theme, tag: &TagSheet) -> (String, String, Vec<El>, El, f32) {
    let f = &s.tag_fields;
    let trigger = tag.tagging.trigger.as_ref();
    let remote = tag.tagging.remote.as_deref();
    let (title, about) = match trigger {
        Some(_) => ("Tag a release", "A tag names this commit for good. Pushing a version tag releases it."),
        None => ("Create a tag", "A tag names this commit for good: a version, a milestone, a build to come back to."),
    };
    let commit = row()
        .key(keys::TAG_COMMIT)
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .radius(size::RADIUS_SM)
        .fill(t.canvas)
        .child(text(tag.short.clone()).font(ty::MONO).color(t.ink_faint))
        .child(text(tag.subject.clone()).font(ty::BODY).color(t.ink).truncate().grow(1.0).min_w(0.0));
    let submit = |e: El| e.on_submit(|s: &mut App, tx| s.tag_event(TagEvent::Submit, Some(tx)));
    let name = ui::field(
        t,
        &f.name,
        "Name",
        "v1.0.0",
        keys::TAG_NAME,
        |s: &mut App| &mut s.tag_fields.name,
        |e| submit(e).on_input_change(|s: &mut App, _| s.tag_event(TagEvent::NameEdited, None)),
    );
    let message = ui::field(
        t,
        &f.message,
        "Message",
        "What it is, for the tag's notes (optional)",
        keys::TAG_MESSAGE,
        |s: &mut App| &mut s.tag_fields.message,
        submit,
    );
    let push = match remote {
        Some(remote) => ui::check(t, f.push, &format!("Push to {remote}"))
            .on_press(|s: &mut App| s.tag_event(TagEvent::TogglePush, None)),
        None => {
            ui::check(t, false, "Push").disabled(true).opacity(0.5).help("This repository has no remote to push to")
        }
    };
    let writes_notes = tag.writes_notes();
    let words = if writes_notes { super::notes::editor(s, t) } else { message };
    let mut body = vec![commit, name, words, push.key(keys::TAG_PUSH)];
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
        body.push(
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
    let warnings = warnings(s, tag);
    if !warnings.is_empty() {
        let mut box_ = col().key(keys::TAG_WARNING).w_full().gap(2.0);
        for w in warnings {
            box_ = box_.child(text(w).font(ty::CAPTION).color(t.accent).wrap());
        }
        body.push(box_);
    }
    if let crate::sheet::Phase::Editing { error: Some(e) } = &tag.phase {
        body.push(super::sheet::error(t, e));
    }
    let width = if writes_notes { size::SHEET_WIDE } else { size::SHEET };
    (title.to_owned(), about.to_owned(), body, buttons(s, t, tag), width)
}

/// A workflow's path with the tag's name where it names the tag (`$GITHUB_REF_NAME`).
fn ref_name(path: &str, tag: &str) -> String {
    ["${{ github.ref_name }}", "${GITHUB_REF_NAME}", "$GITHUB_REF_NAME"]
        .iter()
        .fold(path.to_owned(), |p, v| p.replace(v, tag))
}

/// Why this commit may not be the one to tag, once the inspector has read which refs contain it.
fn warnings(s: &App, tag: &TagSheet) -> Vec<String> {
    let details = s.history.as_ref().and_then(|h| h.details.as_ref());
    let tagged =
        |d: &&crate::state::Details| matches!(&d.subject, Subject::Commit(id) | Subject::Head(id) if *id == tag.commit);
    match details.filter(tagged).and_then(|d| d.info.as_ref()) {
        Some(info) => tag.tagging.warnings(&info.contained_in, s.tag_fields.push),
        None => Vec::new(),
    }
}

/// Cancel, then the primary button, held down while git works.
fn buttons(s: &App, t: &Theme, tag: &TagSheet) -> El {
    let working = !tag.phase.editing();
    let pushing = s.tag_fields.push && tag.tagging.remote.is_some();
    let label = match (working, pushing) {
        (true, true) => "Pushing…",
        (true, false) => "Tagging…",
        (false, true) => "Tag and push",
        (false, false) => "Create tag",
    };
    let cancel = ui::button(t, Variant::Secondary, None, "Cancel", None).key(keys::SHEET_CANCEL);
    let cancel = if working {
        cancel.disabled(true).opacity(0.6)
    } else {
        cancel.on_press(|s: &mut App| s.sheet_event(Event::Dismiss, None))
    };
    let confirm = ui::button_in(t, Variant::Primary, Some(&icons::TAG), label, None, working).key(keys::TAG_CONFIRM);
    let confirm = if working {
        confirm.disabled(true)
    } else {
        confirm.on_press_tx(|s: &mut App, tx| s.tag_event(TagEvent::Submit, Some(tx)))
    };
    row().w_full().gap(8.0).justify_end().child(cancel).child(confirm)
}
