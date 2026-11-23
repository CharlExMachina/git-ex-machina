//! Between the editor's fields (Rune's rich text, with pills for chips and the pad in empty
//! list lines) and the notes (`gxm_core::notes`), and the caret in the text without the pads.

use gxm_core::notes::{self, Chip, Line, Rich, Span};
use rune::{Block, Mark, MarkRange, TextEdit};

use super::Kind;
use super::pad::{self, PAD};

/// The marks markdown can hold (no underline). Pills, the chips, are always allowed.
fn marks() -> [Mark; 5] {
    [Mark::Bold, Mark::Italic, Mark::Strike, Mark::Code, Mark::Link(String::new())]
}

pub(super) fn configure(kind: Kind, edit: TextEdit) -> TextEdit {
    match kind {
        Kind::Body => edit.rich().allow_marks(marks()).allow_blocks([
            Block::Bullet(0),
            Block::Numbered(0),
            Block::Quote,
            Block::Code,
        ]),
        Kind::Heading(_) | Kind::Task(_) => edit.rich().allow_marks(marks()).allow_blocks([]),
        Kind::Fold => edit.rich().allow_marks([]).allow_blocks([]),
    }
}

/// A field holding `rich`, its empty list lines padded, the caret at its end.
pub(super) fn edit_of(kind: Kind, rich: &Rich) -> TextEdit {
    let marks = rich
        .marks
        .iter()
        .map(|s| {
            let mark = match &s.mark {
                notes::Mark::Bold => Mark::Bold,
                notes::Mark::Italic => Mark::Italic,
                notes::Mark::Strike => Mark::Strike,
                notes::Mark::Code => Mark::Code,
                notes::Mark::Link(url) => Mark::Link(url.clone()),
                notes::Mark::Chip(chip) => Mark::Pill(chip.id()),
            };
            MarkRange::new(s.start..s.end, mark)
        })
        .collect();
    let blocks = rich
        .lines
        .iter()
        .map(|l| match l {
            Line::Paragraph => Block::Paragraph,
            Line::Bullet(n) => Block::Bullet(*n),
            Line::Numbered(n) => Block::Numbered(*n),
            Line::Quote => Block::Quote,
            Line::Code => Block::Code,
        })
        .collect();
    let mut edit = configure(kind, TextEdit::from_parts(rich.text.clone(), marks, blocks));
    pad::pad(&mut edit);
    edit
}

/// What a field holds, without its pads.
pub(super) fn rich_of(edit: &TextEdit) -> Rich {
    let plain = |byte: usize| pad::plain_offset(&edit.text, byte);
    let marks = edit
        .marks()
        .iter()
        .filter_map(|m| {
            let mark = match &m.mark {
                Mark::Bold => notes::Mark::Bold,
                Mark::Italic => notes::Mark::Italic,
                Mark::Strike => notes::Mark::Strike,
                Mark::Code => notes::Mark::Code,
                Mark::Link(url) => notes::Mark::Link(url.clone()),
                Mark::Pill(id) => notes::Mark::Chip(Chip::of_id(id)?),
                _ => return None,
            };
            let (start, end) = (plain(m.start), plain(m.end));
            (start < end).then_some(Span { start, end, mark })
        })
        .collect();
    let lines = edit
        .blocks()
        .iter()
        .map(|b| match b {
            Block::Bullet(n) => Line::Bullet(*n),
            Block::Numbered(n) => Line::Numbered(*n),
            Block::Quote => Line::Quote,
            Block::Code => Line::Code,
            Block::Paragraph => Line::Paragraph,
        })
        .collect();
    Rich { text: pad::strip(&edit.text), marks, lines }
}

pub(super) fn plain_text(edit: &TextEdit) -> String {
    pad::strip(&edit.text)
}

pub(super) fn plain_len(edit: &TextEdit) -> usize {
    edit.text.len() - edit.text.matches(PAD).count() * PAD.len_utf8()
}

/// The caret, as a byte of the text without the pads.
pub(super) fn caret_of(edit: &TextEdit) -> usize {
    pad::plain_offset(&edit.text, edit.focus)
}

/// The caret to `plain`, a byte of the text without the pads.
pub(super) fn caret_to(edit: &mut TextEdit, plain: usize) {
    let byte = pad::padded_offset(&edit.text, plain);
    edit.anchor = byte;
    edit.focus = byte;
}
