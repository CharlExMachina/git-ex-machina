//! The release notes editor in the tag sheet: a formatting bar like GitHub's, then the notes as
//! they'll read (headings, collapsed sections, rich text), or the markdown they'll be sent as.
//! GitHub's draft starts them.

use rune::{Block, El, Icon, Mark, ScrollSpec, Tx, col, el, row, text, text_input};

use crate::icons;
use crate::keys;
use crate::notes::{Draft, Kind, NotesEditor};
use crate::state::App;
use crate::tag::TagEvent;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// The notes, under their heading: Write or Markdown, GitHub's draft, the bar and the parts.
pub fn editor(s: &App, t: &Theme) -> El {
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
        col().w_full().padding(12.0).child(
            text(n.markdown()).key(keys::NOTES_MARKDOWN).font(ty::MONO).color(t.ink_muted).wrap().selectable(true),
        )
    } else {
        parts(t, n)
    };
    let mut well = col().w_full().radius(size::RADIUS_SM).fill(t.well).clip_rounded(size::RADIUS_SM);
    for sh in t.shadow_well() {
        well = well.box_shadow(sh);
    }
    if !shown {
        well = well.child(bar(t, n)).child(el().w_full().h(1.0).fill(t.line));
    }
    well = well.child(
        col().w_full().max_h(size::NOTES_MAX_H).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(inside),
    );
    let mut box_ = col().key(keys::NOTES).w_full().gap(6.0).child(header).child(well);
    if let Some(url) = &n.link {
        box_ = box_.child(link_row(t, url));
    }
    if let Draft::Failed(why) = &n.draft {
        box_ = box_.child(text(why.clone()).font(ty::CAPTION).color(t.removed).wrap());
    }
    box_
}

/// The formatting bar: heading, bold, italic, strikethrough, code, link; bullets, numbers, quote,
/// code block; a collapsed section. Each lights while the caret is in what it makes, and none
/// takes focus from the notes.
fn bar(t: &Theme, n: &NotesEditor) -> El {
    let (kind, block) = n.current_kind();
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
        .child(tool(&icons::BULLETS, "Bulleted list", keys::NOTES_BULLETS, matches!(block, Block::Bullet(_)), |n| {
            n.toggle_block(Block::Bullet(0));
        }))
        .child(tool(&icons::NUMBERS, "Numbered list", keys::NOTES_NUMBERS, matches!(block, Block::Numbered(_)), |n| {
            n.toggle_block(Block::Numbered(0));
        }))
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
            |n| {
                n.toggle_fold();
            },
        ))
}

/// The caret back in the notes, in the part it was in.
fn refocus(s: &mut App, tx: &mut Tx) {
    tx.focus(keys::notes_part(s.tag_fields.notes.at), true);
}

/// Every part, as it will read: headings in their size, a section's summary behind its
/// chevron with what folds under it indented, text in the body's.
fn parts(t: &Theme, n: &NotesEditor) -> El {
    let mut list = col().w_full().gap(4.0).padding(12.0);
    // The parts under the open collapsed section, drawn together on one rule.
    let mut folded: Option<Vec<El>> = None;
    let empty = n.is_empty();
    for (i, p) in n.parts.iter().enumerate() {
        let id = p.id;
        let font = match p.kind {
            Kind::Heading(1) => ty::NOTES_H1,
            Kind::Heading(2) => ty::NOTES_H2,
            Kind::Heading(_) => ty::NOTES_H3,
            Kind::Fold => ty::BODY_STRONG,
            Kind::Body => ty::BODY,
        };
        let mut field = text_input(&p.edit, font)
            .key(keys::notes_part(id))
            .label(match p.kind {
                Kind::Heading(_) => "Heading".to_owned(),
                Kind::Fold => "Collapsed section's summary".to_owned(),
                Kind::Body => "Release notes".to_owned(),
            })
            .on_edit(move |s: &mut App| match s.tag_fields.notes.parts.iter().position(|p| p.id == id) {
                Some(i) => &mut s.tag_fields.notes.parts[i].edit,
                // The part went (joined to another) while an edit was on its way.
                None => &mut s.scratch,
            })
            .on_input_key(move |s: &mut App, ev, tx| {
                let taken = s.tag_fields.notes.key(id, ev);
                if taken {
                    refocus(s, tx);
                }
                taken
            })
            .rich_style(t.rich())
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
    let field = ui::field(
        t,
        url,
        "Link",
        "https://…",
        keys::NOTES_LINK_URL,
        |s: &mut App| link_edit(s),
        |e| {
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
        },
    );
    row().w_full().child(field)
}

fn link_edit(s: &mut App) -> &mut rune::TextEdit {
    if s.tag_fields.notes.link.is_none() {
        return &mut s.scratch;
    }
    s.tag_fields.notes.link.get_or_insert_with(|| rune::TextEdit::new(""))
}
