//! Release notes as a person writes them: headings, collapsed sections, and rich text between
//! them (bold, italic, strikethrough, code, links; bullet, numbered, quote and code lines). They
//! go out as GitHub-flavoured markdown ([`markdown`]), and GitHub's own drafts come in from it
//! ([`parse`]), so nobody has to write markdown to write a changelog.

mod read;
mod write;

#[cfg(test)]
mod tests;

pub use read::parse;
pub use write::markdown;

/// A whole set of notes, top to bottom.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notes {
    pub parts: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    /// A heading, level 1 to 3.
    Heading(u8, Rich),
    /// A collapsed section (`<details>`) with this summary, holding every part after it up to
    /// the next one or the end.
    Fold(String),
    /// Lines of text.
    Body(Rich),
}

/// Text with inline marks over byte ranges, and what each `\n`-separated line is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rich {
    pub text: String,
    pub marks: Vec<Span>,
    /// One per line of `text`.
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub mark: Mark,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Bold,
    Italic,
    Strike,
    Code,
    Link(String),
}

impl Mark {
    /// Outermost first, so a link holds bold that holds code, never the other way round.
    fn depth(&self) -> u8 {
        match self {
            Mark::Link(_) => 0,
            Mark::Bold => 1,
            Mark::Italic => 2,
            Mark::Strike => 3,
            Mark::Code => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Line {
    #[default]
    Paragraph,
    /// A bullet at nesting level 0 to 3.
    Bullet(u8),
    Numbered(u8),
    Quote,
    Code,
}

impl Rich {
    /// Plain text, every line a paragraph.
    pub fn plain(text: &str) -> Rich {
        Rich { text: text.to_owned(), marks: Vec::new(), lines: vec![Line::Paragraph; text.split('\n').count()] }
    }

    /// Each line's text and kind, with the byte where it starts.
    pub fn each_line(&self) -> impl Iterator<Item = (usize, &str, Line)> + '_ {
        let mut at = 0;
        self.text.split('\n').enumerate().map(move |(i, line)| {
            let start = at;
            at += line.len() + 1;
            (start, line, self.lines.get(i).copied().unwrap_or_default())
        })
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn line_count(&self) -> usize {
        self.text.split('\n').count()
    }

    /// Bytes `from..to` as text of their own: marks cut to them, and the lines they touch.
    pub fn slice(&self, from: usize, to: usize) -> Rich {
        let text = self.text[from..to].to_owned();
        let marks = self
            .marks
            .iter()
            .filter(|m| m.start < to && m.end > from)
            .map(|m| Span { start: m.start.max(from) - from, end: m.end.min(to) - from, mark: m.mark.clone() })
            .collect();
        let first = self.text[..from].matches('\n').count();
        let count = text.split('\n').count();
        let lines = (first..first + count).map(|i| self.lines.get(i).copied().unwrap_or_default()).collect();
        Rich { text, marks, lines }
    }

    /// Lines `from..to` as text of their own; `None` when there are none.
    pub fn lines_between(&self, from: usize, to: usize) -> Option<Rich> {
        let starts: Vec<usize> = self.each_line().map(|(s, _, _)| s).collect();
        if from >= to || from >= starts.len() {
            return None;
        }
        let end = starts.get(to).map_or(self.text.len(), |s| s - 1);
        Some(self.slice(starts[from], end))
    }

    /// `other`'s lines after this one's.
    pub fn append(&mut self, other: &Rich) {
        self.text.push('\n');
        let at = self.text.len();
        self.text.push_str(&other.text);
        self.marks.extend(other.marks.iter().map(|m| Span {
            start: m.start + at,
            end: m.end + at,
            mark: m.mark.clone(),
        }));
        self.lines.extend(&other.lines);
    }

    /// `other` joined onto the end of this text's last line.
    pub fn extend_line(&mut self, other: &Rich) {
        let at = self.text.len();
        self.text.push_str(&other.text);
        self.marks.extend(other.marks.iter().map(|m| Span {
            start: m.start + at,
            end: m.end + at,
            mark: m.mark.clone(),
        }));
    }
}

/// The first notes for `tag` from GitHub's generated ones (`generated`, markdown): a summary line
/// to keep or rewrite (`**v0.1.2** · 2 changes from 2 contributors`), an empty bullet for the
/// highlights, and GitHub's notes folded away under "Full changelog".
pub fn draft(tag: &str, generated: &str) -> Notes {
    let generated = parse(generated);
    let (changes, people) = count(&generated);
    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let summary = format!(
        "{tag} · {} from {}",
        plural(changes, "change", "changes"),
        plural(people, "contributor", "contributors")
    );
    let text = format!("{summary}\n");
    let lead = Rich {
        marks: vec![Span { start: 0, end: tag.len(), mark: Mark::Bold }],
        lines: vec![Line::Paragraph, Line::Bullet(0)],
        text,
    };
    let mut parts = vec![Part::Body(lead), Part::Fold("Full changelog".to_owned())];
    parts.extend(generated.parts);
    Notes { parts }
}

/// How many changes GitHub's notes list under "What's Changed", and from how many people (the
/// distinct `by @login`s).
fn count(notes: &Notes) -> (usize, usize) {
    let mut under = false;
    let mut changes = 0;
    let mut people: Vec<&str> = Vec::new();
    for part in &notes.parts {
        match part {
            Part::Heading(_, title) => under = title.text.trim() == "What's Changed",
            Part::Body(body) if under => {
                for (_, line, kind) in body.each_line() {
                    if !matches!(kind, Line::Bullet(_)) || line.trim().is_empty() {
                        continue;
                    }
                    changes += 1;
                    let by = line.split(" by @").nth(1).and_then(|r| r.split_whitespace().next());
                    if let Some(login) = by.filter(|l| !people.contains(l)) {
                        people.push(login);
                    }
                }
            }
            _ => {}
        }
    }
    (changes, people.len())
}
