//! The release notes editor: headings, collapsed sections and rich text between them, each part
//! a field of its own (Rune's rich text has no headings). Arrow keys, Return and Backspace move
//! across the parts as though they were one document; the formatting bar works on the part the
//! caret is in. The notes go out as markdown (`gxm_core::notes`).

mod chips;
mod convert;
pub mod pad;

use gxm_core::notes::{self, Chip, Line, Notes, Part, Rich, Span};
use rune::{Block, Key, KeyEvent, Mark, TextEdit};

pub use chips::{Pick, PickKind};
use convert::{caret_of, caret_to, edit_of, plain_len, plain_text, rich_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Heading(u8),
    /// A collapsed section's summary: the parts after it, up to the next one, fold under it.
    Fold,
    /// A task list's item, checked or not: a checkbox to click, and its words.
    Task(bool),
    Body,
}

pub struct EditPart {
    pub id: u64,
    pub kind: Kind,
    pub edit: TextEdit,
}

/// GitHub's draft: not asked for, on its way, or why it didn't come.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Draft {
    Idle,
    Asking,
    Failed(String),
}

pub struct NotesEditor {
    pub parts: Vec<EditPart>,
    next: u64,
    /// The part the caret was last in.
    pub at: u64,
    /// Markdown (what will be sent) instead of the editor.
    pub markdown_shown: bool,
    /// The link's address while one is being made.
    pub link: Option<TextEdit>,
    pub draft: Draft,
    /// Files attached: each one's name in the notes, and where it is.
    pub files: Vec<(String, std::path::PathBuf)>,
    /// The commit holding them, once made: the notes link the files by it.
    pub files_commit: Option<String>,
    /// The `@` or `#` popup, while one is open.
    pub pick: Option<Pick>,
    /// Where each part's field was last drawn, in window points: whether the caret is on its
    /// first or last row as drawn.
    pub rects: std::collections::HashMap<u64, [f32; 4]>,
}

impl Default for NotesEditor {
    fn default() -> Self {
        NotesEditor::from_notes(&Notes::default())
    }
}

impl NotesEditor {
    pub fn from_notes(notes: &Notes) -> NotesEditor {
        let mut e = NotesEditor {
            parts: Vec::new(),
            next: 0,
            at: 0,
            markdown_shown: false,
            link: None,
            draft: Draft::Idle,
            files: Vec::new(),
            files_commit: None,
            pick: None,
            rects: std::collections::HashMap::new(),
        };
        for part in &notes.parts {
            match part {
                Part::Heading(level, rich) => {
                    e.push(Kind::Heading(*level), rich);
                }
                Part::Fold(summary) => {
                    e.push(Kind::Fold, &Rich::plain(summary));
                }
                Part::Body(rich) => {
                    for (kind, rich) in split_tasks(rich) {
                        e.push(kind, &rich);
                    }
                }
            }
        }
        e.normalize();
        e.at = e.parts.first().map_or(0, |p| p.id);
        e
    }

    pub fn notes(&self) -> Notes {
        let mut parts: Vec<Part> = Vec::new();
        for p in &self.parts {
            let rich = match p.kind {
                Kind::Heading(level) => {
                    parts.push(Part::Heading(level, rich_of(&p.edit)));
                    continue;
                }
                Kind::Fold => {
                    parts.push(Part::Fold(plain_text(&p.edit)));
                    continue;
                }
                // A task is a line of text led by its checkbox, among the text around it.
                Kind::Task(done) => task_line(done, &rich_of(&p.edit)),
                Kind::Body => rich_of(&p.edit),
            };
            match parts.last_mut() {
                Some(Part::Body(before)) => before.append(&rich),
                _ => parts.push(Part::Body(rich)),
            }
        }
        Notes { parts }
    }

    /// The notes as the tag will carry them, attached files linked under `assets` (the release's
    /// download URL).
    pub fn markdown(&self, assets: Option<&str>) -> String {
        notes::markdown_with(&self.notes(), assets)
    }

    pub fn is_empty(&self) -> bool {
        self.parts.iter().all(|p| plain_text(&p.edit).trim().is_empty())
    }

    /// Every field's pads where they belong (after an edit Rune made).
    pub fn repad(&mut self) {
        for p in &mut self.parts {
            pad::pad(&mut p.edit);
        }
    }

    fn push(&mut self, kind: Kind, rich: &Rich) -> u64 {
        let id = self.fresh();
        self.parts.push(EditPart { id, kind, edit: edit_of(kind, rich) });
        id
    }

    fn fresh(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn index(&self, id: u64) -> Option<usize> {
        self.parts.iter().position(|p| p.id == id)
    }

    pub fn part_mut(&mut self, id: u64) -> Option<&mut EditPart> {
        self.parts.iter_mut().find(|p| p.id == id)
    }

    /// The part the caret is in: the focused field, else the one it was last in.
    pub fn current(&self) -> u64 {
        self.parts.iter().find(|p| p.edit.focused).map_or(self.at, |p| p.id)
    }

    /// GitHub's draft: the editor's notes when it's empty, else after what's there (without its
    /// summary line, which is already written). The caret goes to the first empty line.
    pub fn take_draft(&mut self, draft: &Notes) {
        if self.is_empty() {
            *self = NotesEditor::from_notes(draft);
            if let Some(first) = self.parts.first_mut() {
                let end = plain_len(&first.edit);
                caret_to(&mut first.edit, end);
                self.at = first.id;
            }
            return;
        }
        let rest = Notes { parts: draft.parts.iter().skip(1).cloned().collect() };
        for part in NotesEditor::from_notes(&rest).parts {
            let id = self.fresh();
            self.parts.push(EditPart { id, ..part });
        }
        self.normalize();
    }

    /// Heading on the caret's line, or back to text.
    pub fn toggle_heading(&mut self) {
        let id = self.current();
        match self.index(id).map(|i| self.parts[i].kind) {
            Some(Kind::Body) => self.lift(id, Kind::Heading(3)),
            Some(Kind::Heading(_)) => self.lower(id),
            Some(Kind::Fold | Kind::Task(_)) => self.rekind(id, Kind::Heading(3)),
            None => {}
        }
    }

    /// The caret's line as a task, unchecked, or a task back to text.
    pub fn toggle_task(&mut self) {
        let id = self.current();
        match self.index(id).map(|i| self.parts[i].kind) {
            Some(Kind::Body) => self.lift(id, Kind::Task(false)),
            Some(Kind::Task(_)) => self.lower(id),
            Some(Kind::Heading(_) | Kind::Fold) => self.rekind(id, Kind::Task(false)),
            None => {}
        }
    }

    /// Task `id` checked, or unchecked.
    pub fn check(&mut self, id: u64) {
        let at = self.at;
        if let Some(Kind::Task(done)) = self.index(id).map(|i| self.parts[i].kind) {
            self.rekind(id, Kind::Task(!done));
            self.at = at;
        }
    }

    /// A collapsed section from the caret's line (its summary), or back to text.
    pub fn toggle_fold(&mut self) {
        let id = self.current();
        match self.index(id).map(|i| self.parts[i].kind) {
            Some(Kind::Body) => self.lift(id, Kind::Fold),
            Some(Kind::Fold) => self.lower(id),
            Some(Kind::Heading(_) | Kind::Task(_)) => self.rekind(id, Kind::Fold),
            None => {}
        }
    }

    pub fn toggle_mark(&mut self, mark: Mark) {
        let id = self.current();
        if let Some(p) = self.part_mut(id) {
            p.edit.toggle_mark(mark);
        }
        self.at = id;
    }

    /// A list, quote or code line; on a heading, the heading turns back into text first.
    pub fn toggle_block(&mut self, block: Block) {
        let id = self.current();
        if self.index(id).is_some_and(|i| self.parts[i].kind != Kind::Body) {
            self.lower(id);
        }
        let id = self.at;
        if let Some(p) = self.part_mut(id) {
            p.edit.toggle_block(block);
            pad::pad(&mut p.edit);
        }
    }

    /// Whether the caret's part has `mark` where the caret is.
    pub fn has_mark(&self, mark: &Mark) -> bool {
        let id = self.current();
        self.parts.iter().find(|p| p.id == id).is_some_and(|p| p.edit.has_mark(mark))
    }

    /// The caret's line's kind of block, or heading/section.
    pub fn current_kind(&self) -> (Kind, Block) {
        let id = self.current();
        self.parts
            .iter()
            .find(|p| p.id == id)
            .map_or((Kind::Body, Block::Paragraph), |p| (p.kind, p.edit.current_block()))
    }

    /// Links the selection to `url` (the address itself, linked, where nothing is selected).
    pub fn apply_link(&mut self, url: &str) {
        let id = self.at;
        let Some(p) = self.part_mut(id) else { return };
        let url = url.trim();
        if url.is_empty() {
            p.edit.set_link(None);
            return;
        }
        if p.edit.anchor == p.edit.focus {
            let at = p.edit.focus;
            p.edit.insert(url);
            p.edit.set_selection(at, at + url.len());
        }
        p.edit.set_link(Some(url));
        let end = p.edit.anchor.max(p.edit.focus);
        p.edit.anchor = end;
        p.edit.focus = end;
        pad::pad(&mut p.edit);
    }

    /// Splits the caret's line out of a body as a part of `kind`.
    fn lift(&mut self, id: u64, kind: Kind) {
        let Some(i) = self.index(id) else { return };
        let edit = &self.parts[i].edit;
        let rich = rich_of(edit);
        let caret = caret_of(edit);
        let line = rich.text[..caret].matches('\n').count();
        let column = caret - rich.text[..caret].rfind('\n').map_or(0, |n| n + 1);
        let count = rich.line_count();
        let mut made = Vec::new();
        if let Some(before) = rich.lines_between(0, line) {
            made.push((Kind::Body, before));
        }
        let mut own = rich.lines_between(line, line + 1).unwrap_or_default();
        own.lines = vec![Line::Paragraph];
        made.push((kind, own));
        if let Some(after) = rich.lines_between(line + 1, count) {
            made.push((Kind::Body, after));
        }
        let mut parts = Vec::new();
        let mut lifted = 0;
        for (k, r) in made {
            let id = self.fresh();
            let mut edit = edit_of(k, &r);
            if k == kind {
                lifted = id;
                caret_to(&mut edit, column);
            }
            parts.push(EditPart { id, kind: k, edit });
        }
        self.parts.splice(i..=i, parts);
        self.at = lifted;
        self.normalize();
    }

    /// A heading or summary back to a line of text, joined to the text around it.
    fn lower(&mut self, id: u64) {
        let Some(i) = self.index(id) else { return };
        let caret = caret_of(&self.parts[i].edit);
        let mut rich = rich_of(&self.parts[i].edit);
        rich.lines = vec![Line::Paragraph; rich.line_count()];
        let mut edit = edit_of(Kind::Body, &rich);
        caret_to(&mut edit, caret);
        self.parts[i] = EditPart { id, kind: Kind::Body, edit };
        self.at = id;
        self.normalize();
    }

    fn rekind(&mut self, id: u64, kind: Kind) {
        let Some(i) = self.index(id) else { return };
        let caret = caret_of(&self.parts[i].edit);
        let mut edit = edit_of(kind, &rich_of(&self.parts[i].edit));
        caret_to(&mut edit, caret);
        self.parts[i] = EditPart { id, kind, edit };
        self.at = id;
    }

    /// Text runs as one part: neighbouring bodies join, the caret staying where it was; empty
    /// bodies between headings go (unless the caret is in one); and the notes end in a body,
    /// somewhere to write after the last heading.
    fn normalize(&mut self) {
        let mut i = 0;
        while i + 1 < self.parts.len() {
            if self.parts[i].kind == Kind::Body && self.parts[i + 1].kind == Kind::Body {
                let next = self.parts.remove(i + 1);
                let first = &mut self.parts[i];
                let shift = plain_len(&first.edit) + 1;
                let caret = if self.at == next.id { Some(shift + caret_of(&next.edit)) } else { None };
                let keep = if self.at == first.id { Some(caret_of(&first.edit)) } else { None };
                let mut rich = rich_of(&first.edit);
                rich.append(&rich_of(&next.edit));
                first.edit = edit_of(Kind::Body, &rich);
                if let Some(c) = caret.or(keep) {
                    caret_to(&mut first.edit, c);
                    self.at = first.id;
                }
            } else {
                i += 1;
            }
        }
        let at = self.at;
        let last = self.parts.len().saturating_sub(1);
        let mut n = 0;
        self.parts.retain(|p| {
            let keep = p.kind != Kind::Body || !plain_text(&p.edit).is_empty() || p.id == at || n == last;
            n += 1;
            keep
        });
        if self.parts.last().is_none_or(|p| p.kind != Kind::Body) {
            self.push(Kind::Body, &Rich::plain(""));
        }
        if self.index(self.at).is_none() {
            self.at = self.parts.first().map_or(0, |p| p.id);
        }
    }

    /// A key in part `id`'s field, before the field has it: the ones that cross into the parts
    /// around it, and the ones that step over a pad or a task's checkbox. True when it was
    /// taken; the caret is then in `self.at`. `rows` says whether the caret is on the field's
    /// first and last rows as drawn (a long line wraps onto several), when that's known.
    pub fn key(&mut self, id: u64, ev: &KeyEvent, rows: Option<(bool, bool)>) -> bool {
        let Some(i) = self.index(id) else { return false };
        self.at = id;
        let plain = !ev.mods.cmd && !ev.mods.alt && !ev.mods.ctrl && !ev.mods.shift;
        let p = &self.parts[i];
        let (text, caret, collapsed) = (plain_text(&p.edit), caret_of(&p.edit), p.edit.anchor == p.edit.focus);
        let (first_row, last_row) = rows.unwrap_or((!text[..caret].contains('\n'), !text[caret..].contains('\n')));
        let kind = p.kind;
        let go = |e: &mut NotesEditor, to: usize, end: bool| {
            let part = &mut e.parts[to];
            let byte = if end { plain_len(&part.edit) } else { 0 };
            caret_to(&mut part.edit, byte);
            e.at = part.id;
            true
        };
        let crossing = match ev.key {
            Key::Up => plain && first_row && i > 0,
            Key::Down => plain && last_row && i + 1 < self.parts.len(),
            Key::Left => plain && collapsed && caret == 0 && i > 0,
            Key::Right => plain && collapsed && caret == text.len() && i + 1 < self.parts.len(),
            _ => false,
        };
        if !crossing && kind == Kind::Body && collapsed && self.step_over(i, ev) {
            return true;
        }
        match ev.key {
            Key::Up if plain && first_row && i > 0 => go(self, i - 1, true),
            Key::Down if plain && last_row && i + 1 < self.parts.len() => go(self, i + 1, false),
            Key::Left if plain && collapsed && caret == 0 && i > 0 => go(self, i - 1, true),
            Key::Right if plain && collapsed && caret == text.len() && i + 1 < self.parts.len() => {
                go(self, i + 1, false)
            }
            Key::Enter if kind == Kind::Body && plain && self.parts[i].edit.current_block() == Block::Paragraph => {
                // Rune's Return in a paragraph submits the field (a chat box's); in notes it starts
                // the next paragraph.
                let edit = &mut self.parts[i].edit;
                edit.insert("\n");
                edit.set_block(Block::Paragraph);
                pad::pad(edit);
                true
            }
            Key::Enter if matches!(kind, Kind::Task(_)) && !ev.mods.shift && text.trim().is_empty() => {
                // Return in an empty task ends the list: it's a line of text again.
                self.lower(id);
                true
            }
            Key::Enter if kind != Kind::Body && !ev.mods.shift => {
                // Return in a heading starts the text under it, and in a task the next task,
                // taking what was after the caret.
                let rich = rich_of(&self.parts[i].edit);
                let mut tail = rich.slice(caret, rich.text.len());
                tail.lines = vec![Line::Paragraph];
                let head = rich.slice(0, caret);
                self.parts[i].edit = edit_of(kind, &head);
                let next = if matches!(kind, Kind::Task(_)) { Kind::Task(false) } else { Kind::Body };
                let new = self.fresh();
                self.parts.insert(i + 1, EditPart { id: new, kind: next, edit: edit_of(next, &tail) });
                caret_to(&mut self.parts[i + 1].edit, 0);
                self.at = new;
                self.normalize();
                true
            }
            Key::Backspace if collapsed && caret == 0 && !ev.mods.cmd && !ev.mods.alt => {
                if kind != Kind::Body {
                    self.lower(id);
                    return true;
                }
                let Some(prev) = i.checked_sub(1) else { return false };
                // The first line of text under a heading: an empty one goes; one with words joins
                // the heading.
                let rich = rich_of(&self.parts[i].edit);
                let count = rich.line_count();
                let first = rich.lines_between(0, 1).unwrap_or_default();
                let rest = rich.lines_between(1, count);
                let joined_at = plain_len(&self.parts[prev].edit);
                if !first.text.is_empty() {
                    let mut heading = rich_of(&self.parts[prev].edit);
                    let mut line = first;
                    if self.parts[prev].kind == Kind::Fold {
                        line.marks.clear();
                    }
                    heading.extend_line(&line);
                    self.parts[prev].edit = edit_of(self.parts[prev].kind, &heading);
                }
                match rest {
                    Some(rest) => self.parts[i].edit = edit_of(Kind::Body, &rest),
                    None => self.parts[i].edit = edit_of(Kind::Body, &Rich::plain("")),
                }
                caret_to(&mut self.parts[prev].edit, joined_at);
                self.at = self.parts[prev].id;
                self.normalize();
                true
            }
            _ => false,
        }
    }
}

/// A body's lines as text, with each task line (led by its checkbox) a task of its own.
fn split_tasks(rich: &Rich) -> Vec<(Kind, Rich)> {
    let mut out: Vec<(Kind, Rich)> = Vec::new();
    for n in 0..rich.line_count() {
        let Some(line) = rich.lines_between(n, n + 1) else { continue };
        let task = line.marks.iter().find_map(|m| match m.mark {
            notes::Mark::Chip(Chip::Task(done)) if m.start == 0 => Some((done, m.end)),
            _ => None,
        });
        match task {
            Some((done, end)) => {
                let skip = end + line.text[end..].len() - line.text[end..].trim_start().len();
                out.push((Kind::Task(done), line.slice(skip, line.text.len())));
            }
            None => match out.last_mut() {
                Some((Kind::Body, before)) => before.append(&line),
                _ => out.push((Kind::Body, line)),
            },
        }
    }
    out
}

/// A task's words as a line of text led by its checkbox.
fn task_line(done: bool, words: &Rich) -> Rich {
    let label = Chip::Task(done).label();
    let shift = label.len();
    let mut marks = vec![Span { start: 0, end: shift, mark: notes::Mark::Chip(Chip::Task(done)) }];
    marks.extend(words.marks.iter().map(|m| Span { start: m.start + shift, end: m.end + shift, mark: m.mark.clone() }));
    Rich { text: format!("{label}{}", words.text), marks, lines: vec![Line::Paragraph] }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_task_has_a_checkbox_to_click_and_its_own_line() {
        let mut e = editor("Before\n\n- [ ] Ship it\n- [x] Tag it\n\nAfter\n");
        let kinds: Vec<Kind> = e.parts.iter().map(|p| p.kind).collect();
        assert_eq!(kinds, [Kind::Body, Kind::Task(false), Kind::Task(true), Kind::Body]);
        assert_eq!(e.parts[1].edit.text, "Ship it");
        let first = e.parts[1].id;
        e.check(first);
        assert_eq!(e.markdown(None), "Before\n\n- [x] Ship it\n- [x] Tag it\n\nAfter\n");
        // Return starts the next task; in an empty one it ends the list.
        let tag = at_line(&mut e, 2, 6);
        assert!(e.key(tag, &key(Key::Enter), None));
        assert_eq!(e.parts[3].kind, Kind::Task(false));
        let empty = e.parts[3].id;
        assert!(e.key(empty, &key(Key::Enter), None));
        assert_eq!(e.parts[3].kind, Kind::Body);
        assert_eq!(e.markdown(None), "Before\n\n- [x] Ship it\n- [x] Tag it\n\nAfter\n");
        // The button makes a line a task, and a task a line again.
        at_line(&mut e, 0, 3);
        e.toggle_task();
        assert_eq!(e.parts[0].kind, Kind::Task(false));
        e.toggle_task();
        assert_eq!(e.markdown(None), "Before\n\n- [x] Ship it\n- [x] Tag it\n\nAfter\n");
    }

    use super::*;

    fn key(key: Key) -> KeyEvent {
        KeyEvent { key, mods: rune::Mods::default() }
    }

    fn editor(md: &str) -> NotesEditor {
        NotesEditor::from_notes(&notes::parse(md))
    }

    fn at_line(e: &mut NotesEditor, part: usize, byte: usize) -> u64 {
        let p = &mut e.parts[part];
        caret_to(&mut p.edit, byte);
        e.at = p.id;
        p.id
    }

    #[test]
    fn a_line_becomes_a_heading_and_back_again() {
        let mut e = editor("one\n\ntwo\n\nthree\n");
        at_line(&mut e, 0, 5);
        e.toggle_heading();
        assert_eq!(e.markdown(None), "one\n\n### two\n\nthree\n");
        assert_eq!(e.parts.iter().map(|p| p.kind).collect::<Vec<_>>(), [Kind::Body, Kind::Heading(3), Kind::Body]);
        assert_eq!(e.parts[1].edit.focus, 1, "the caret stays in its place on the line");
        e.toggle_heading();
        assert_eq!(e.markdown(None), "one\n\ntwo\n\nthree\n");
        assert_eq!(e.parts.len(), 1, "the text joins up again");
        assert_eq!(e.parts[0].edit.focus, 5);
    }

    #[test]
    fn a_section_folds_what_comes_after_its_summary() {
        let mut e = editor("Highlights\n\nFull changelog\n\n- one\n");
        at_line(&mut e, 0, 11);
        e.toggle_fold();
        assert_eq!(
            e.markdown(None),
            "Highlights\n\n<details>\n<summary>Full changelog</summary>\n\n- one\n\n</details>\n"
        );
    }

    #[test]
    fn return_in_a_heading_starts_its_text_and_backspace_joins_it_back() {
        let mut e = editor("## What's Changed\n\n- one\n");
        let id = at_line(&mut e, 0, 6);
        assert!(e.key(id, &key(Key::Enter), None));
        assert_eq!(e.markdown(None), "## What's\n\n Changed\n\n- one\n");
        let body = e.at;
        assert!(e.key(body, &key(Key::Backspace), None));
        assert_eq!(e.markdown(None), "## What's Changed\n\n- one\n");
        assert_eq!(e.parts[0].edit.focus, 6, "the caret is where they joined");
    }

    #[test]
    fn arrows_cross_from_part_to_part() {
        let mut e = editor("## Title\n\nfirst\nsecond\n");
        let title = at_line(&mut e, 0, 3);
        assert!(e.key(title, &key(Key::Down), None));
        assert_eq!((e.at, e.parts[1].edit.focus), (e.parts[1].id, 0));
        let body = e.parts[1].id;
        assert!(!e.key(body, &key(Key::Down), None), "within the text, the field moves the caret");
        caret_to(&mut e.parts[1].edit, 2);
        assert!(e.key(body, &key(Key::Up), None));
        assert_eq!((e.at, e.parts[0].edit.focus), (title, 5));
    }

    #[test]
    fn a_draft_fills_empty_notes_and_follows_written_ones() {
        let draft = notes::draft("v1.0.0", "## What's Changed\n* One by @ana in x\n");
        let mut e = NotesEditor::default();
        e.take_draft(&draft);
        assert!(
            e.markdown(None).starts_with("**v1.0.0** · 1 change from 1 contributor\n\n<details>"),
            "{}",
            e.markdown(None)
        );
        let mut written = editor("My own words\n");
        written.take_draft(&draft);
        assert!(
            written.markdown(None).starts_with("My own words\n\n<details>\n<summary>Full changelog</summary>"),
            "{}",
            written.markdown(None)
        );
    }
}
