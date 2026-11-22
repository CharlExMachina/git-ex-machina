//! Chips in the notes: a task's checkbox, an attached file, and a person or pull request picked
//! from the popup `@` and `#` open. Each is a Rune pill (it stands whole: the caret steps over
//! it, Backspace takes it at once); the notes write it as GitHub's markdown for it.

use std::path::PathBuf;

use gxm_core::notes::{Chip, asset_name};
use rune::{Block, Key, KeyEvent, TextEdit};

use super::convert::{caret_of, caret_to};
use super::pad::{self, PAD};
use super::{Kind, NotesEditor};

/// What the popup lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickKind {
    /// People to mention, after `@`.
    People,
    /// Pull requests, after `#`.
    Pulls,
}

/// The popup `@` or `#` opened: in which part, from which byte (the `@` itself), what's been
/// typed after it, and which row is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub part: u64,
    pub kind: PickKind,
    pub start: usize,
    pub query: String,
    pub active: usize,
}

/// Where the line holding `byte` starts and ends.
fn line_of(text: &str, byte: usize) -> (usize, usize) {
    let byte = byte.min(text.len());
    let start = text[..byte].rfind('\n').map_or(0, |n| n + 1);
    let end = text[byte..].find('\n').map_or(text.len(), |n| byte + n);
    (start, end)
}

/// The task checkbox starting the line at `start`: its end, and whether it's checked.
fn task_at(edit: &TextEdit, start: usize) -> Option<(usize, bool)> {
    edit.pills().find_map(|(range, id)| match Chip::of_id(id) {
        Some(Chip::Task(done)) if range.start == start => Some((range.end, done)),
        _ => None,
    })
}

fn put_chip(edit: &mut TextEdit, at: std::ops::Range<usize>, chip: &Chip) {
    edit.replace_with_pill(at, chip.id(), &chip.label());
}

impl NotesEditor {
    /// The caret's line as a task, unchecked; a task's checkbox checked; a checked one
    /// unchecked. Taking a task back to a bullet is the bullet button's.
    pub fn toggle_task(&mut self) {
        let id = self.current();
        if self.index(id).is_some_and(|i| self.parts[i].kind != Kind::Body) {
            self.lower(id);
        }
        let id = self.at;
        let Some(p) = self.part_mut(id) else { return };
        let caret = caret_of(&p.edit);
        let edit = &mut p.edit;
        let (start, _) = line_of(&edit.text, edit.focus);
        match task_at(edit, start) {
            Some((end, done)) => {
                put_chip(edit, start..end, &Chip::Task(!done));
                caret_to(edit, caret);
            }
            None => {
                // A task's line is its checkbox and words, as GitHub draws it: no bullet.
                if edit.current_block() != Block::Paragraph {
                    edit.toggle_block(edit.current_block());
                }
                // An empty item's pad goes: the checkbox stands in its place.
                if pad::pad_line(&edit.text, edit.focus) == Some(start) {
                    edit.set_selection(start, start + PAD.len_utf8());
                    edit.insert("");
                }
                let chip = Chip::Task(false);
                put_chip(edit, start..start, &chip);
                caret_to(edit, caret + chip.label().len());
                pad::pad(edit);
            }
        }
    }

    /// Whether the caret's line is a task.
    pub fn on_task(&self) -> bool {
        let id = self.current();
        self.parts.iter().find(|p| p.id == id).is_some_and(|p| {
            let (start, _) = line_of(&p.edit.text, p.edit.focus);
            task_at(&p.edit, start).is_some()
        })
    }

    /// The caret's line's task checkbox, and the space after it, gone.
    pub(super) fn untask(&mut self, id: u64) {
        let Some(p) = self.part_mut(id) else { return };
        let edit = &mut p.edit;
        let (start, _) = line_of(&edit.text, edit.focus);
        if let Some((end, _)) = task_at(edit, start) {
            let end = if edit.text[end..].starts_with(' ') { end + 1 } else { end };
            let caret = edit.focus;
            edit.set_selection(start, end);
            edit.insert("");
            let back = caret.saturating_sub(end - start).max(start);
            edit.anchor = back;
            edit.focus = back;
        }
    }

    /// Keys at a pad or a task's checkbox, in body `i` with nothing selected: true when taken.
    /// The arrows step over a pad as though the line were empty; Return, Backspace and Delete
    /// see it empty too. Return in a task starts the next task; in an empty one it ends the list.
    pub(super) fn step_over(&mut self, i: usize, ev: &KeyEvent) -> bool {
        let edit = &mut self.parts[i].edit;
        let caret = edit.focus;
        let pad_len = PAD.len_utf8();
        let bare = !ev.mods.cmd && !ev.mods.alt && !ev.mods.ctrl;
        if let Some(start) = pad::pad_line(&edit.text, caret) {
            match ev.key {
                Key::Left if bare && caret == start + pad_len => edit.focus = start,
                Key::Right if bare && caret == start => edit.focus = start + pad_len,
                Key::Backspace | Key::Delete | Key::Enter if bare => {
                    edit.set_selection(start, start + pad_len);
                    edit.insert("");
                }
                _ => return false,
            }
            edit.anchor = edit.focus;
            return false;
        }
        let (start, end) = line_of(&edit.text, caret);
        let Some((chip_end, _)) = task_at(edit, start) else { return false };
        let after = if edit.text[chip_end..].starts_with(' ') { chip_end + 1 } else { chip_end };
        match ev.key {
            Key::Enter if bare && !ev.mods.shift => {
                if edit.text[after..end].trim().is_empty() {
                    // An empty task: its checkbox goes, which ends the list, as in an empty item.
                    edit.set_selection(start, end);
                    edit.insert("");
                    return true;
                }
                edit.insert("\n");
                edit.set_block(Block::Paragraph);
                let at = edit.focus;
                put_chip(edit, at..at, &Chip::Task(false));
                true
            }
            Key::Backspace if bare && (caret == after || caret == chip_end) => {
                edit.set_selection(start, after);
                edit.insert("");
                true
            }
            _ => false,
        }
    }

    /// Attaches the file at `path`: a chip where the caret is (an image shows in the notes),
    /// under the name the release will keep it by.
    pub fn add_file(&mut self, path: PathBuf) {
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let base = asset_name(&file);
        let mut name = base.clone();
        let mut n = 2;
        while self.files.iter().any(|(f, _)| *f == name) {
            name = match base.rsplit_once('.') {
                Some((stem, ext)) => format!("{stem}-{n}.{ext}"),
                None => format!("{base}-{n}"),
            };
            n += 1;
        }
        self.files.push((name.clone(), path));
        let id = self.current();
        let Some(p) = self.part_mut(id) else { return };
        let at = p.edit.focus;
        put_chip(&mut p.edit, at..at, &Chip::File(name));
        p.edit.insert(" ");
        pad::pad(&mut p.edit);
        self.at = id;
    }

    /// After an edit in part `id`: opens the popup when the caret follows `@` or `#` (and what's
    /// typed after it), and closes it otherwise.
    pub fn sense_pick(&mut self, id: u64) {
        let Some(i) = self.index(id) else { return };
        let edit = &self.parts[i].edit;
        let caret = edit.focus;
        let (start, _) = line_of(&edit.text, caret);
        let word = edit.text[start..caret]
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .last()
            .map_or(caret, |(k, _)| start + k);
        let kind = match edit.text[start..word].chars().next_back() {
            Some('@') => PickKind::People,
            Some('#') => PickKind::Pulls,
            _ => {
                self.pick = None;
                return;
            }
        };
        // `@` and `#` are a byte each.
        let at = word - 1;
        let before = edit.text[start..at].chars().next_back();
        let in_pill = edit.pills().any(|(range, _)| range.contains(&at));
        let opens =
            edit.anchor == caret && !in_pill && before.is_none_or(|b| b.is_whitespace() || b == '(' || b == PAD);
        self.pick = opens.then(|| {
            let query = edit.text[word..caret].to_owned();
            let active = match &self.pick {
                Some(p) if p.part == id && p.start == at && p.query == query => p.active,
                _ => 0,
            };
            Pick { part: id, kind, start: at, query, active }
        });
    }

    /// The popup's row `by` rows on, among `count`.
    pub fn move_pick(&mut self, by: i32, count: usize) {
        if let Some(p) = self.pick.as_mut().filter(|_| count > 0) {
            let n = i32::try_from(count).unwrap_or(i32::MAX);
            let at = (i32::try_from(p.active).unwrap_or(0) + by).rem_euclid(n);
            p.active = usize::try_from(at).unwrap_or(0);
        }
    }

    /// `chip` in place of the `@` or `#` and what was typed after it, and a space.
    pub fn choose(&mut self, chip: &Chip) {
        let Some(pick) = self.pick.take() else { return };
        let Some(p) = self.part_mut(pick.part) else { return };
        let caret = p.edit.focus.max(pick.start);
        put_chip(&mut p.edit, pick.start..caret, chip);
        p.edit.insert(" ");
        self.at = pick.part;
    }
}

#[cfg(test)]
mod tests {
    use gxm_core::notes;
    use rune::Mods;

    use super::*;
    use crate::notes::convert::{caret_of, caret_to};

    fn key(key: Key) -> KeyEvent {
        KeyEvent { key, mods: Mods::default() }
    }

    fn editor(md: &str) -> NotesEditor {
        NotesEditor::from_notes(&notes::parse(md))
    }

    /// Notes of one line of plain text, read as typed rather than as markdown.
    fn typed(text: &str) -> NotesEditor {
        NotesEditor::from_notes(&notes::Notes { parts: vec![notes::Part::Body(notes::Rich::plain(text))] })
    }

    /// The caret to byte `at` of part 0's text (without pads), as though typing there.
    fn caret_at(e: &mut NotesEditor, at: usize) -> u64 {
        let p = &mut e.parts[0];
        caret_to(&mut p.edit, at);
        e.at = p.id;
        p.id
    }

    #[test]
    fn the_task_button_makes_a_task_checks_it_and_unchecks_it() {
        let mut e = editor("- Ship it\n");
        caret_at(&mut e, 7);
        e.toggle_task();
        assert_eq!(e.markdown(None), "- [ ] Ship it\n");
        assert_eq!(e.parts[0].edit.current_block(), Block::Paragraph, "no bullet beside the checkbox");
        e.toggle_task();
        assert_eq!(e.markdown(None), "- [x] Ship it\n");
        e.toggle_task();
        assert_eq!(e.markdown(None), "- [ ] Ship it\n");
        e.toggle_block(Block::Bullet(0));
        assert_eq!(e.markdown(None), "- Ship it\n", "the bullet button takes the checkbox away");
    }

    #[test]
    fn return_in_a_task_starts_the_next_and_in_an_empty_one_ends_the_list() {
        let mut e = editor("- [x] One\n");
        let id = caret_at(&mut e, notes::Chip::Task(true).label().len() + 3);
        assert!(e.key(id, &key(Key::Enter), None));
        assert_eq!(e.markdown(None), "- [x] One\n", "an empty task writes nothing");
        assert!(e.on_task(), "the new line is a task");
        assert!(e.key(id, &key(Key::Enter), None));
        assert!(!e.on_task(), "an empty task's Return ends the list");
        assert_eq!(e.markdown(None), "- [x] One\n");
    }

    #[test]
    fn at_and_hash_open_the_popup_only_where_a_word_starts() {
        let mut e = typed("Thanks @ke");
        let id = caret_at(&mut e, 10);
        e.sense_pick(id);
        let pick = e.pick.clone().unwrap();
        assert_eq!((pick.kind, pick.query.as_str(), pick.start), (PickKind::People, "ke", 7));
        e.choose(&Chip::Mention("kenji-mori".to_owned()));
        assert_eq!(e.markdown(None), "Thanks @kenji-mori\n");
        assert!(e.pick.is_none());
        // Inside a word (an email) it stays shut; after a space it opens for pull requests.
        let mut mail = typed("ana@example");
        let id = caret_at(&mut mail, 11);
        mail.sense_pick(id);
        assert!(mail.pick.is_none());
        let mut pr = typed("See #");
        let id = caret_at(&mut pr, 5);
        pr.sense_pick(id);
        assert_eq!(pr.pick.map(|p| (p.kind, p.query)), Some((PickKind::Pulls, String::new())));
    }

    #[test]
    fn attached_files_keep_names_github_keeps_and_never_two_alike() {
        let mut e = NotesEditor::default();
        e.add_file(PathBuf::from("/tmp/My shot.png"));
        e.add_file(PathBuf::from("/elsewhere/My shot.png"));
        let names: Vec<&str> = e.files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["My-shot.png", "My-shot-2.png"]);
        assert_eq!(
            e.markdown(Some("https://x/releases/download/v1")),
            "![My-shot.png](https://x/releases/download/v1/My-shot.png) ![My-shot-2.png](https://x/releases/download/v1/My-shot-2.png)\n"
        );
    }

    #[test]
    fn the_caret_steps_over_an_empty_bullet_s_pad() {
        let mut e = editor("- one\n");
        let id = caret_at(&mut e, 3);
        // Return continues the list: Rune's doing, done here by hand.
        e.parts[0].edit.insert("\n");
        e.parts[0].edit.set_block(Block::Bullet(0));
        pad::pad(&mut e.parts[0].edit);
        let edit = &e.parts[0].edit;
        assert!(edit.text.ends_with(PAD), "the empty item holds the pad");
        assert_eq!(caret_of(edit), 4, "and the caret is past it");
        // Backspace sees the item empty: the pad goes, and the field has the key.
        assert!(!e.key(id, &key(Key::Backspace), None));
        assert!(!e.parts[0].edit.text.contains(PAD));
    }
}
