//! The release notes editor in the tag sheet: a formatting bar like GitHub's, then the notes as
//! they'll read (headings, collapsed sections, rich text with mentions, references, tasks and
//! attached files), or the markdown they'll be sent as. GitHub's draft starts them.

use gxm_core::notes::Chip;
use rune::{Block, El, Icon, Mark, PillStyle, ScrollSpec, Tx, col, el, row, text, text_input};

use crate::icons;
use crate::keys;
use crate::notes::{Draft, Kind, NotesEditor};
use crate::state::App;
use crate::tag::{TagEvent, TagSheet};
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// The notes, under their heading: Write or Markdown, GitHub's draft, the bar and the parts.
/// `caret` is the notes' caret as last drawn, in window points.
pub fn editor(s: &App, t: &Theme, tag: &TagSheet, caret: Option<[f32; 4]>) -> El {
    let n = &s.tag_fields.notes;
    let asking = n.draft == Draft::Asking;
    let draft = ui::button_in(
        t,
        Variant::Ghost,
        Some(&icons::SPARKLE),
        if asking { "Drafting…" } else { "Draft from pull requests" },
        None,
        asking,
    )
    .key(keys::NOTES_GENERATE)
    .help("GitHub's notes for this tag: the pull requests merged since the last one, folded under a summary for you to write");
    let draft = if asking {
        draft.disabled(true)
    } else {
        draft.on_press_tx(|s: &mut App, tx| s.tag_event(TagEvent::Draft, Some(tx)))
    };
    let shown = n.markdown_shown;
    let write = ui::segment_label(t, "Write", !shown)
        .key(keys::NOTES_WRITE)
        .on_press(|s: &mut App| s.tag_fields.notes.markdown_shown = false);
    let source = ui::segment_label(t, "Markdown", shown)
        .key(keys::NOTES_SOURCE)
        .on_press(|s: &mut App| s.tag_fields.notes.markdown_shown = true);
    let header = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(text("Release notes").font(ty::CAPTION_STRONG).color(t.ink_muted))
        .child(el().grow(1.0))
        .child(draft)
        .child(ui::segmented(t, vec![(write, !shown), (source, shown)]));
    let inside = if shown {
        // The files' links name their commit, made as they're attached ("…" until it is).
        let commit = n.files_commit.as_deref().unwrap_or("…");
        let markdown = n.markdown(tag.files_base(commit).as_deref());
        col().w_full().padding(12.0).child(
            text(markdown)
                .key(keys::NOTES_MARKDOWN)
                .font(ty::MONO)
                .color(t.ink_muted)
                .wrap()
                .wrap_anywhere()
                .selectable(true),
        )
    } else {
        parts(t, n, caret)
    };
    let mut well = col().w_full().radius(size::RADIUS_SM).fill(t.well).clip_rounded(size::RADIUS_SM);
    for sh in t.shadow_well() {
        well = well.box_shadow(sh);
    }
    if !shown {
        well = well.child(bar(t, n, tag.files_base("").is_some())).child(el().w_full().h(1.0).fill(t.line));
    }
    well = well.child(
        col().w_full().max_h(size::NOTES_MAX_H).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(inside),
    );
    let mut box_ =
        col().key(keys::NOTES).w_full().gap(6.0).child(header).child(well).on_drop_files(|s: &mut App, files, tx| {
            for f in files {
                s.tag_fields.notes.add_file(f.clone());
            }
            s.tag_event(TagEvent::FilesChanged, None);
            refocus(s, tx);
        });
    if let Some(url) = &n.link {
        box_ = box_.child(link_row(t, url));
    }
    if let Draft::Failed(why) = &n.draft {
        box_ = box_.child(text(why.clone()).font(ty::CAPTION).color(t.removed).wrap());
    }
    box_
}

/// The formatting bar: heading, bold, italic, strikethrough, code, link; bullets, numbers,
/// tasks, quote, code block; a collapsed section; and attaching a file (on GitHub, `attaches`).
/// Each lights while the caret is in what it makes, and none takes focus from the notes.
fn bar(t: &Theme, n: &NotesEditor, attaches: bool) -> El {
    let (kind, block) = n.current_kind();
    let task = matches!(kind, Kind::Task(_));
    let tool = |glyph: &'static Icon, label: &str, key: &str, on: bool, act: fn(&mut NotesEditor)| {
        ui::icon_button(t, Variant::Ghost, glyph, Some(if on { t.accent } else { t.ink_muted }), label, on)
            .key(key)
            .focusable(false)
            .on_press_tx(move |s: &mut App, tx| {
                act(&mut s.tag_fields.notes);
                refocus(s, tx);
            })
    };
    let gap = || el().w(1.0).h(16.0).fill(t.line).mx(4.0);
    let link = ui::icon_button(
        t,
        Variant::Ghost,
        &icons::LINK,
        Some(t.ink_muted),
        "Link (paste an address)",
        n.link.is_some(),
    )
    .key(keys::NOTES_LINK)
    .focusable(false)
    .on_press_tx(|s: &mut App, tx| {
        let notes = &mut s.tag_fields.notes;
        notes.at = notes.current();
        let url = notes.parts.iter().find(|p| p.id == notes.at).and_then(|p| p.edit.active_link().map(str::to_owned));
        notes.link = Some(rune::TextEdit::new(url.unwrap_or_default()));
        tx.focus(keys::NOTES_LINK_URL, true);
    });
    let attach = ui::icon_button(
        t,
        Variant::Ghost,
        &icons::ATTACH,
        Some(t.ink_muted),
        "Attach files (or drop them here)",
        false,
    )
    .key(keys::NOTES_ATTACH)
    .focusable(false);
    let attach = if attaches {
        attach.on_press_tx(|s: &mut App, tx| {
            s.tag_fields.notes.at = s.tag_fields.notes.current();
            let options =
                rune::FileDialogOptions { title: "Attach a file to the release".to_owned(), ..Default::default() };
            tx.open_file(options, |s: &mut App, picked, tx| {
                if let Ok(Some(path)) = picked {
                    s.tag_fields.notes.add_file(path);
                    s.tag_event(TagEvent::FilesChanged, None);
                }
                refocus(s, tx);
            });
        })
    } else {
        attach.disabled(true).opacity(0.5).help("Attached files go up to GitHub, with the tag")
    };
    row()
        .w_full()
        .center_y()
        .gap(2.0)
        .px(6.0)
        .py(4.0)
        .child(tool(&icons::HEADING, "Heading", keys::NOTES_HEADING, matches!(kind, Kind::Heading(_)), |n| {
            n.toggle_heading();
        }))
        .child(tool(&icons::BOLD, "Bold (⌘B)", keys::NOTES_BOLD, n.has_mark(&Mark::Bold), |n| {
            n.toggle_mark(Mark::Bold);
        }))
        .child(tool(&icons::ITALIC, "Italic (⌘I)", keys::NOTES_ITALIC, n.has_mark(&Mark::Italic), |n| {
            n.toggle_mark(Mark::Italic);
        }))
        .child(tool(&icons::STRIKE, "Strikethrough", keys::NOTES_STRIKE, n.has_mark(&Mark::Strike), |n| {
            n.toggle_mark(Mark::Strike);
        }))
        .child(tool(&icons::CODE, "Code", keys::NOTES_CODE, n.has_mark(&Mark::Code), |n| n.toggle_mark(Mark::Code)))
        .child(link)
        .child(gap())
        .child(tool(
            &icons::BULLETS,
            "Bulleted list",
            keys::NOTES_BULLETS,
            !task && matches!(block, Block::Bullet(_)),
            |n| {
                n.toggle_block(Block::Bullet(0));
            },
        ))
        .child(tool(&icons::NUMBERS, "Numbered list", keys::NOTES_NUMBERS, matches!(block, Block::Numbered(_)), |n| {
            n.toggle_block(Block::Numbered(0));
        }))
        .child(tool(&icons::TASKS, "Task list", keys::NOTES_TASKS, task, |n| n.toggle_task()))
        .child(tool(&icons::QUOTE, "Quote", keys::NOTES_QUOTE, block == Block::Quote, |n| n.toggle_block(Block::Quote)))
        .child(tool(&icons::CODE_BLOCK, "Code block", keys::NOTES_CODE_BLOCK, block == Block::Code, |n| {
            n.toggle_block(Block::Code);
        }))
        .child(gap())
        .child(tool(
            &icons::FOLD,
            "Collapsed section: what follows folds under this line",
            keys::NOTES_FOLD,
            kind == Kind::Fold,
            |n| n.toggle_fold(),
        ))
        .child(gap())
        .child(attach)
}

/// The caret back in the notes, in the part it was in.
pub(super) fn refocus(s: &mut App, tx: &mut Tx) {
    tx.focus(keys::notes_part(s.tag_fields.notes.at), true);
}

/// How a chip looks: a mention or reference on the accent's tint, a task's checkbox as a box
/// (accent once checked), an attached file with its kind's icon.
fn chip_style(t: Theme) -> impl Fn(&str) -> PillStyle + 'static {
    move |id| match Chip::of_id(id) {
        Some(Chip::Mention(_)) => PillStyle::new().bg(t.accent_soft).fg(t.accent).icon(&icons::PERSON),
        Some(Chip::Pull(_)) => PillStyle::new().bg(t.accent_soft).fg(t.accent).icon(&icons::PULL_REQUEST),
        Some(Chip::File(name)) if Chip::is_image(&name) => PillStyle::new().bg(t.control).fg(t.ink).icon(&icons::IMAGE),
        Some(Chip::File(_)) => PillStyle::new().bg(t.control).fg(t.ink).icon(&icons::FILE),
        Some(Chip::Task(_)) | None => PillStyle::new(),
    }
}

/// Whether the caret is on the first and last rows `rect` (a field, as drawn) holds.
fn rows(caret: [f32; 4], rect: [f32; 4]) -> (bool, bool) {
    let half = caret[3] * 0.5;
    (caret[1] < rect[1] + half, caret[1] + caret[3] > rect[1] + rect[3] - half)
}

/// Every part, as it will read: headings in their size, a section's summary behind its
/// chevron with what folds under it indented, text in the body's.
fn parts(t: &Theme, n: &NotesEditor, caret: Option<[f32; 4]>) -> El {
    let mut list = col().w_full().gap(4.0).padding(12.0);
    // The parts under the open collapsed section, drawn together on one rule.
    let mut folded: Option<Vec<El>> = None;
    let empty = n.is_empty();
    let mut tasks = 0;
    for (i, p) in n.parts.iter().enumerate() {
        let id = p.id;
        let font = match p.kind {
            Kind::Heading(1) => ty::NOTES_H1,
            Kind::Heading(2) => ty::NOTES_H2,
            Kind::Heading(_) => ty::NOTES_H3,
            Kind::Fold => ty::BODY_STRONG,
            Kind::Task(_) | Kind::Body => ty::BODY,
        };
        let edges = caret.filter(|_| p.edit.focused).zip(n.rects.get(&id)).map(|(c, r)| rows(c, *r));
        let mut field = text_input(&p.edit, font)
            .key(keys::notes_part(id))
            .label(match p.kind {
                Kind::Heading(_) => "Heading".to_owned(),
                Kind::Fold => "Collapsed section's summary".to_owned(),
                Kind::Task(_) => "Task".to_owned(),
                Kind::Body => "Release notes".to_owned(),
            })
            .on_edit(move |s: &mut App| match s.tag_fields.notes.parts.iter().position(|p| p.id == id) {
                Some(i) => &mut s.tag_fields.notes.parts[i].edit,
                // The part went (joined to another) while an edit was on its way.
                None => &mut s.scratch,
            })
            .on_input_change(move |s: &mut App, _| {
                let notes = &mut s.tag_fields.notes;
                notes.repad();
                notes.sense_pick(id);
            })
            .on_input_key(move |s: &mut App, ev, tx| {
                if super::picker::key(s, id, ev, tx) {
                    return true;
                }
                let taken = s.tag_fields.notes.key(id, ev, edges);
                if taken {
                    refocus(s, tx);
                }
                taken
            })
            .on_drawn_layout(move |s: &mut App, rect| {
                s.tag_fields.notes.rects.insert(id, rect);
            })
            .rich_style(t.rich())
            .pill_style(chip_style(*t))
            .color(t.ink)
            .w_full()
            .min_w(0.0);
        if p.kind != Kind::Body {
            field = field.single_line();
        }
        if empty && i == 0 {
            field = field.placeholder("What's in this release? Or draft it from the pull requests.", t.ink_faint);
        }
        let line = match p.kind {
            Kind::Fold => {
                if let Some(inside) = folded.take() {
                    list = list.child(fold_body(t, inside));
                }
                folded = Some(Vec::new());
                row()
                    .w_full()
                    .center_y()
                    .gap(6.0)
                    .mt(if i > 0 { 6.0 } else { 0.0 })
                    .child(rune::icon(&icons::CHEVRON_RIGHT, 12.0).color(t.ink_faint))
                    .child(field.grow(1.0))
                    .child(text("collapsed").font(ty::HINT).color(t.ink_faint))
            }
            Kind::Heading(_) => row().w_full().mt(if i > 0 { 6.0 } else { 0.0 }).child(field),
            Kind::Task(done) => {
                // The checkbox is a control of its own: a click checks the task off.
                let check = ui::check_box(t, done)
                    .key(keys::notes_task(tasks))
                    .role("checkbox")
                    .label(if done { "Done" } else { "Not done" })
                    .focusable(false)
                    .on_press_tx(move |s: &mut App, tx| {
                        s.tag_fields.notes.check(id);
                        refocus(s, tx);
                    });
                tasks += 1;
                row().w_full().center_y().gap(8.0).pl(4.0).child(check).child(field.grow(1.0))
            }
            Kind::Body => row().w_full().child(field),
        };
        match folded.as_mut() {
            Some(inside) if p.kind != Kind::Fold => inside.push(line),
            _ => list = list.child(line),
        }
    }
    if let Some(inside) = folded {
        list = list.child(fold_body(t, inside));
    }
    list
}

/// What a collapsed section holds: indented on a rule, as GitHub draws it open.
fn fold_body(t: &Theme, inside: Vec<El>) -> El {
    row()
        .w_full()
        .gap(10.0)
        .child(el().w(1.0).h_full().fill(t.line).ml(5.0))
        .child(col().gap(4.0).grow(1.0).min_w(0.0).children(inside))
}

/// The link's address, under the bar: Return applies it, an empty one takes the link away.
fn link_row(t: &Theme, url: &rune::TextEdit) -> El {
    let field = ui::field(t, url, "Link", "https://…", keys::NOTES_LINK_URL, link_edit, |e| {
        e.on_submit(|s: &mut App, tx| {
            if let Some(url) = s.tag_fields.notes.link.take() {
                s.tag_fields.notes.apply_link(&url.text);
            }
            refocus(s, tx);
        })
        .on_input_key(|s: &mut App, ev, tx| {
            if ev.key != rune::Key::Escape {
                return false;
            }
            s.tag_fields.notes.link = None;
            refocus(s, tx);
            true
        })
    });
    row().w_full().child(field)
}

fn link_edit(s: &mut App) -> &mut rune::TextEdit {
    if s.tag_fields.notes.link.is_none() {
        return &mut s.scratch;
    }
    s.tag_fields.notes.link.get_or_insert_with(|| rune::TextEdit::new(""))
}
