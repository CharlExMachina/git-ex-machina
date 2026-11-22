//! A zero-width space in every empty list or quote line. Rune draws the caret on a line with
//! no glyphs at the line's left edge, not at its list indent, so in an empty bullet it stood
//! before the bullet. With a glyph to stand on, the caret is where the text will go. The space
//! is only ever the whole of its line: typing replaces it, the arrow keys, Return and Backspace
//! step over it (see `NotesEditor::key`), and the notes never carry it (`rich_of` strips it).

use rune::{Block, TextEdit};

pub const PAD: char = '\u{200B}';
const PAD_LEN: usize = PAD.len_utf8();

/// Whether `block` indents its line, so an empty one needs the pad.
fn indents(block: Block) -> bool {
    matches!(block, Block::Bullet(_) | Block::Numbered(_) | Block::Quote)
}

/// Puts the pad in each empty indented line and takes it out of every other, keeping the
/// caret (and a selection's ends) where they were.
pub fn pad(edit: &mut TextEdit) {
    let (mut anchor, mut focus) = (edit.anchor, edit.focus);
    let blocks = edit.blocks().to_vec();
    let mut starts: Vec<usize> = vec![0];
    starts.extend(edit.text.match_indices('\n').map(|(i, _)| i + 1));
    // Last line first, so the earlier lines' offsets hold while the later ones change.
    for (n, &start) in starts.iter().enumerate().rev() {
        let end = starts.get(n + 1).map_or(edit.text.len(), |next| next - 1);
        let line = &edit.text[start..end];
        let block = blocks.get(n).copied().unwrap_or_default();
        if line.is_empty() && indents(block) {
            replace(edit, start..start, &PAD.to_string());
            for at in [&mut anchor, &mut focus] {
                if *at >= start {
                    *at += PAD_LEN;
                }
            }
        } else if line.contains(PAD) && (line.len() > PAD_LEN || !indents(block)) {
            let pads: Vec<usize> = line.match_indices(PAD).map(|(i, _)| start + i).collect();
            for &p in pads.iter().rev() {
                replace(edit, p..p + PAD_LEN, "");
                for at in [&mut anchor, &mut focus] {
                    if *at > p {
                        *at = (*at - PAD_LEN).max(p);
                    }
                }
            }
        }
    }
    edit.anchor = anchor.min(edit.text.len());
    edit.focus = focus.min(edit.text.len());
}

fn replace(edit: &mut TextEdit, range: std::ops::Range<usize>, with: &str) {
    edit.set_selection(range.start, range.end);
    edit.insert(with);
}

/// `byte` in padded text, as a byte of the text without the pads.
pub fn plain_offset(padded: &str, byte: usize) -> usize {
    let byte = byte.min(padded.len());
    byte - padded[..byte].matches(PAD).count() * PAD_LEN
}

/// `plain`, a byte of the text without the pads, in padded text: after a pad that stands there.
pub fn padded_offset(padded: &str, plain: usize) -> usize {
    let mut seen = 0;
    for (i, c) in padded.char_indices() {
        if c == PAD {
            if seen == plain {
                return i + PAD_LEN;
            }
            continue;
        }
        if seen == plain {
            return i;
        }
        seen += c.len_utf8();
    }
    padded.len()
}

/// The line holding `byte` when it's nothing but the pad: where it starts.
pub fn pad_line(text: &str, byte: usize) -> Option<usize> {
    let start = text[..byte.min(text.len())].rfind('\n').map_or(0, |n| n + 1);
    let end = text[start..].find('\n').map_or(text.len(), |n| start + n);
    (text[start..end] == *PAD.to_string()).then_some(start)
}

/// `text` without its pads.
pub fn strip(text: &str) -> String {
    text.replace(PAD, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(text: &str, blocks: &[Block], caret: usize) -> TextEdit {
        let mut e = TextEdit::from_parts(text, Vec::new(), blocks.to_vec());
        e.anchor = caret;
        e.focus = caret;
        e
    }

    #[test]
    fn empty_bullets_hold_the_pad_and_lines_with_words_lose_it() {
        let mut e = edit("one\n\ntwo\n", &[Block::Bullet(0), Block::Bullet(0), Block::Paragraph, Block::Quote], 4);
        pad(&mut e);
        assert_eq!(e.text, "one\n\u{200B}\ntwo\n\u{200B}");
        assert_eq!(e.focus, 4 + PAD_LEN, "the caret stands after the pad, at the indent");
        assert_eq!(e.blocks(), [Block::Bullet(0), Block::Bullet(0), Block::Paragraph, Block::Quote]);
        // Typed after the pad: the pad goes, the caret stays after what was typed.
        let mut typed = edit("\u{200B}x", &[Block::Bullet(0)], PAD_LEN + 1);
        pad(&mut typed);
        assert_eq!((typed.text.as_str(), typed.focus), ("x", 1));
        // A line that stopped being a list item loses it.
        let mut plain = edit("\u{200B}", &[Block::Paragraph], PAD_LEN);
        pad(&mut plain);
        assert_eq!((plain.text.as_str(), plain.focus), ("", 0));
    }

    #[test]
    fn offsets_map_between_padded_and_plain_text() {
        let padded = "a\n\u{200B}\nb";
        assert_eq!(plain_offset(padded, 2 + PAD_LEN), 2);
        assert_eq!(padded_offset(padded, 2), 2 + PAD_LEN, "the caret goes after the pad");
        assert_eq!(padded_offset(padded, 4), padded.len());
        assert_eq!(plain_offset(padded, padded.len()), 4);
        assert_eq!(pad_line(padded, 2 + PAD_LEN), Some(2));
        assert_eq!(pad_line(padded, 0), None);
        assert_eq!(strip(padded), "a\n\nb");
    }
}
