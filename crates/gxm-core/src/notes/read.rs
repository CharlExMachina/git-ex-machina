//! Markdown in as notes, for GitHub's generated drafts: headings, `<details>` sections, list
//! items (nested by indentation), quotes, fenced code and paragraphs, with bold, italic,
//! strikethrough, code and links inside them. What it doesn't know stays as text.

use super::{Line, Mark, Notes, Part, Rich, Span};

/// The body being read, line by line.
#[derive(Default)]
struct Body {
    rich: Rich,
    /// The paragraph line still taking wrapped lines.
    open_paragraph: bool,
}

impl Body {
    fn push(&mut self, kind: Line, markdown: &str) {
        let (text, marks) = inline(markdown);
        if !self.rich.lines.is_empty() {
            self.rich.text.push('\n');
        }
        let at = self.rich.text.len();
        self.rich.text.push_str(&text);
        self.rich.marks.extend(marks.into_iter().map(|m| Span { start: m.start + at, end: m.end + at, mark: m.mark }));
        self.rich.lines.push(kind);
        self.open_paragraph = kind == Line::Paragraph;
    }

    /// A line of fenced code, taken as written: no marks.
    fn code(&mut self, raw: &str) {
        if !self.rich.lines.is_empty() {
            self.rich.text.push('\n');
        }
        self.rich.text.push_str(raw);
        self.rich.lines.push(Line::Code);
        self.open_paragraph = false;
    }

    /// A wrapped line of the open paragraph: joined to it with a space.
    fn wrap(&mut self, markdown: &str) {
        let (text, marks) = inline(markdown);
        self.rich.text.push(' ');
        let at = self.rich.text.len();
        self.rich.text.push_str(&text);
        self.rich.marks.extend(marks.into_iter().map(|m| Span { start: m.start + at, end: m.end + at, mark: m.mark }));
    }

    fn take(&mut self, parts: &mut Vec<Part>) {
        let body = std::mem::take(self);
        if !body.rich.lines.is_empty() {
            parts.push(Part::Body(body.rich));
        }
    }
}

/// `markdown` as notes.
pub fn parse(markdown: &str) -> Notes {
    let mut parts = Vec::new();
    let mut body = Body::default();
    let mut fence: Option<String> = None;
    let mut indents: Vec<usize> = Vec::new();
    for raw in markdown.lines() {
        if let Some(f) = &fence {
            if raw.trim_start().starts_with(f.as_str()) {
                fence = None;
            } else {
                body.code(raw);
            }
            continue;
        }
        let line = raw.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            fence = Some(line.chars().take_while(|c| *c == '`' || *c == '~').collect());
            continue;
        }
        if line.is_empty() {
            body.open_paragraph = false;
            continue;
        }
        if line == "<details>" || line == "</details>" || line.starts_with("<details ") {
            body.take(&mut parts);
            continue;
        }
        if let Some(summary) = line.strip_prefix("<summary>").and_then(|s| s.strip_suffix("</summary>")) {
            body.take(&mut parts);
            parts.push(Part::Fold(unescape_html(summary.trim())));
            continue;
        }
        if let Some((level, title)) = heading(line) {
            body.take(&mut parts);
            let (text, marks) = inline(title);
            parts.push(Part::Heading(level, Rich { text, marks, lines: vec![Line::Paragraph] }));
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        if let Some((numbered, item)) = list_item(line) {
            while indents.last().is_some_and(|&i| i > indent) {
                indents.pop();
            }
            if indents.last() != Some(&indent) {
                indents.push(indent);
            }
            let level = u8::try_from(indents.len().saturating_sub(1)).unwrap_or(3).min(3);
            body.push(if numbered { Line::Numbered(level) } else { Line::Bullet(level) }, item);
            continue;
        }
        indents.clear();
        if let Some(quoted) = line.strip_prefix('>') {
            body.push(Line::Quote, quoted.trim_start());
        } else if body.open_paragraph {
            body.wrap(line);
        } else {
            body.push(Line::Paragraph, line);
        }
    }
    body.take(&mut parts);
    Notes { parts }
}

/// `## Title` as (2, "Title"); levels past 3 read as 3.
fn heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    let rest = &line[hashes..];
    let level = u8::try_from(hashes).ok().filter(|h| (1..=6).contains(h))?;
    (rest.is_empty() || rest.starts_with(' ')).then(|| (level.min(3), rest.trim().trim_end_matches('#').trim_end()))
}

/// `- item`, `* item`, `+ item` or `1. item` as (numbered, "item").
fn list_item(line: &str) -> Option<(bool, &str)> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return Some((false, rest));
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    let rest = line.get(digits..)?;
    let rest = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))?;
    (digits > 0).then_some((true, rest))
}

/// One line's markdown as text and marks.
pub(super) fn inline(md: &str) -> (String, Vec<Span>) {
    let mut text = String::new();
    let mut marks = Vec::new();
    read_inline(md, &mut text, &mut marks);
    (text, marks)
}

fn read_inline(md: &str, text: &mut String, marks: &mut Vec<Span>) {
    let mut i = 0;
    while i < md.len() {
        let rest = &md[i..];
        let c = rest.chars().next().unwrap_or_default();
        if c == '\\'
            && let Some(next) = rest[1..].chars().next().filter(|n| n.is_ascii_punctuation())
        {
            text.push(next);
            i += 1 + next.len_utf8();
            continue;
        }
        if c == '`' {
            let ticks = rest.chars().take_while(|c| *c == '`').count();
            let delim = &rest[..ticks];
            if let Some(end) = rest[ticks..].find(delim) {
                let code = &rest[ticks..ticks + end];
                let code = if code.starts_with(' ') && code.ends_with(' ') && code.len() > 1 {
                    &code[1..code.len() - 1]
                } else {
                    code
                };
                let start = text.len();
                text.push_str(code);
                marks.push(Span { start, end: text.len(), mark: Mark::Code });
                i += ticks + end + ticks;
                continue;
            }
        }
        if c == '['
            && let Some((label, url, used)) = link(rest)
        {
            let start = text.len();
            read_inline(label, text, marks);
            marks.push(Span { start, end: text.len(), mark: Mark::Link(url.to_owned()) });
            i += used;
            continue;
        }
        let delim = ["**", "__", "~~", "*", "_"].into_iter().find(|d| rest.starts_with(d));
        if let Some(d) = delim {
            let before = md[..i].chars().next_back();
            let after = rest[d.len()..].chars().next();
            let opens = after.is_some_and(|a| !a.is_whitespace())
                && !(d.starts_with('_') && before.is_some_and(char::is_alphanumeric));
            if opens && let Some(end) = closing(&rest[d.len()..], d) {
                let start = text.len();
                read_inline(&rest[d.len()..d.len() + end], text, marks);
                let mark = match d {
                    "**" | "__" => Mark::Bold,
                    "~~" => Mark::Strike,
                    _ => Mark::Italic,
                };
                if text.len() > start {
                    marks.push(Span { start, end: text.len(), mark });
                }
                i += d.len() + end + d.len();
                continue;
            }
        }
        text.push(c);
        i += c.len_utf8();
    }
}

/// Where `delim` closes in `s`: not after a space, and for `_` not inside a word.
fn closing(s: &str, delim: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(at) = s[from..].find(delim).map(|a| a + from) {
        let before = s[..at].chars().next_back();
        let after = s[at + delim.len()..].chars().next();
        // `**` inside `***`-style runs, or `*` that is half of a `**`, isn't this one.
        let doubled = delim.len() == 1 && (s[at + 1..].starts_with(delim) || (at > 0 && s[..at].ends_with(delim)));
        let ok = at > 0
            && before.is_some_and(|b| !b.is_whitespace())
            && !doubled
            && !(delim.starts_with('_') && after.is_some_and(char::is_alphanumeric));
        if ok {
            return Some(at);
        }
        from = at + delim.len();
    }
    None
}

/// `[label](url)` at the start of `s`: the label, the URL and the bytes it took.
fn link(s: &str) -> Option<(&str, &str, usize)> {
    let mut depth = 0;
    let mut close = None;
    for (i, c) in s.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let rest = s[close + 1..].strip_prefix('(')?;
    let end = rest.find(')')?;
    let url = rest[..end].split_whitespace().next().unwrap_or_default();
    Some((&s[1..close], url, close + 1 + 1 + end + 1))
}

fn unescape_html(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&")
}
