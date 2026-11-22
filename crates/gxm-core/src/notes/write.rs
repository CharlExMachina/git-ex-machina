//! Notes out as GitHub-flavoured markdown: what the tag carries, and what GitHub shows.

use super::{Chip, Line, Mark, Notes, Part, Rich, Span};

/// One markdown block, and whether it's a list item (list items sit on consecutive lines; every
/// other block has a blank line before it).
struct Block {
    text: String,
    item: bool,
    /// An empty line followed it: the list it was in ends there.
    ends_list: bool,
}

impl Block {
    fn new(text: String) -> Block {
        Block { text, item: false, ends_list: false }
    }
}

/// `notes` as markdown, ending in a newline (empty notes are empty). Attached files link to
/// their names; see [`markdown_with`].
pub fn markdown(notes: &Notes) -> String {
    markdown_with(notes, None)
}

/// [`markdown`], with attached files linked where the release will keep them: `assets` is the
/// release's download URL (`https://github.com/owner/name/releases/download/v1.0.0`).
pub fn markdown_with(notes: &Notes, assets: Option<&str>) -> String {
    let mut blocks: Vec<Block> = Vec::new();
    let mut in_fold = false;
    for part in &notes.parts {
        match part {
            Part::Heading(level, title) => {
                let hashes = "#".repeat(usize::from((*level).clamp(1, 3)));
                let title = inline(title.text.lines().next().unwrap_or_default(), &title.marks, assets);
                if !title.trim().is_empty() {
                    blocks.push(Block::new(format!("{hashes} {title}")));
                }
            }
            Part::Fold(summary) => {
                if in_fold {
                    blocks.push(Block::new("</details>".to_owned()));
                }
                let summary = escape_html(summary.trim());
                blocks.push(Block::new(format!("<details>\n<summary>{summary}</summary>")));
                in_fold = true;
            }
            Part::Body(body) => body_blocks(body, assets, &mut blocks),
        }
    }
    if in_fold {
        blocks.push(Block::new("</details>".to_owned()));
    }
    let mut out = String::new();
    let mut last: Option<&Block> = None;
    for b in &blocks {
        if let Some(prev) = last {
            let same_list = b.item && prev.item && !prev.ends_list;
            out.push_str(if same_list { "\n" } else { "\n\n" });
        }
        out.push_str(&b.text);
        last = Some(b);
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// A body's lines as blocks: paragraphs, list items (indented under their parents, numbered per
/// level), quotes and fenced code, each run of quote or code lines one block.
fn body_blocks(body: &Rich, assets: Option<&str>, blocks: &mut Vec<Block>) {
    let mut widths: Vec<usize> = Vec::new();
    let mut counts: Vec<u32> = Vec::new();
    let mut run: Option<(Line, Vec<String>)> = None;
    let flush = |run: &mut Option<(Line, Vec<String>)>, blocks: &mut Vec<Block>| {
        if let Some((kind, lines)) = run.take() {
            let text = match kind {
                Line::Code => {
                    let fence = if lines.iter().any(|l| l.contains("```")) { "````" } else { "```" };
                    format!("{fence}\n{}\n{fence}", lines.join("\n"))
                }
                _ => lines.iter().map(|l| format!("> {l}")).collect::<Vec<_>>().join("\n"),
            };
            blocks.push(Block::new(text));
        }
    };
    for (start, line, kind) in body.each_line() {
        let end = start + line.len();
        // A paragraph led by a task's checkbox is a task list's item.
        let tasked = body.marks.iter().any(|m| m.start == start && matches!(m.mark, Mark::Chip(Chip::Task(_))));
        let kind = if tasked && kind == Line::Paragraph { Line::Bullet(0) } else { kind };
        let marks: Vec<Span> = body
            .marks
            .iter()
            .filter(|m| m.start < end && m.end > start)
            .map(|m| Span { start: m.start.max(start) - start, end: m.end.min(end) - start, mark: m.mark.clone() })
            .collect();
        if run.as_ref().is_some_and(|(k, _)| *k != kind) {
            flush(&mut run, blocks);
        }
        match kind {
            Line::Code => run.get_or_insert_with(|| (Line::Code, Vec::new())).1.push(line.to_owned()),
            _ if line.trim().is_empty() => {
                // An empty line ends a list, so the next one starts afresh.
                widths.clear();
                counts.clear();
                if let Some(last) = blocks.last_mut() {
                    last.ends_list = true;
                }
            }
            Line::Quote => run.get_or_insert_with(|| (Line::Quote, Vec::new())).1.push(inline(line, &marks, assets)),
            Line::Bullet(level) | Line::Numbered(level) => {
                let level = usize::from(level).min(widths.len());
                widths.truncate(level);
                counts.truncate(level + 1);
                counts.resize(level + 1, 0);
                counts[level] += 1;
                let marker = match kind {
                    Line::Numbered(_) => format!("{}.", counts[level]),
                    _ => "-".to_owned(),
                };
                let indent = " ".repeat(widths.iter().sum());
                widths.push(marker.len() + 1);
                // A task's checkbox leads its item: `- [x] Done`.
                let task = marks.iter().find_map(|m| match &m.mark {
                    Mark::Chip(Chip::Task(done)) if m.start == 0 => Some((*done, m.end)),
                    _ => None,
                });
                let item = match task {
                    Some((done, end)) => {
                        let skip = end + line[end..].len() - line[end..].trim_start().len();
                        let rest: Vec<Span> = marks
                            .iter()
                            .filter(|m| m.end > skip)
                            .map(|m| Span { start: m.start.max(skip) - skip, end: m.end - skip, mark: m.mark.clone() })
                            .collect();
                        format!("[{}] {}", if done { "x" } else { " " }, inline(&line[skip..], &rest, assets))
                    }
                    None => inline(line, &marks, assets),
                };
                let text = format!("{indent}{marker} {}", item.trim_end());
                blocks.push(Block { text, item: true, ends_list: false });
            }
            Line::Paragraph => {
                widths.clear();
                counts.clear();
                blocks.push(Block::new(inline(line, &marks, assets).trim_end().to_owned()));
            }
        }
    }
    flush(&mut run, blocks);
}

/// One line's text with its marks as markdown delimiters, outermost mark first. A mark's edges
/// skip the spaces it covers (`**word **` isn't bold in markdown), and characters markdown
/// would read as formatting are escaped, except inside code and URLs.
fn inline(text: &str, marks: &[Span], assets: Option<&str>) -> String {
    let marks: Vec<Span> = marks
        .iter()
        .filter_map(|m| {
            let s = &text[m.start..m.end];
            let (start, end) = if matches!(m.mark, Mark::Code | Mark::Chip(_)) {
                (m.start, m.end)
            } else {
                (m.start + (s.len() - s.trim_start().len()), m.end - (s.len() - s.trim_end().len()))
            };
            (start < end).then(|| Span { start, end, mark: m.mark.clone() })
        })
        .collect();
    let mut cuts: Vec<usize> = marks.iter().flat_map(|m| [m.start, m.end]).chain([0, text.len()]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = String::new();
    let mut open: Vec<Mark> = Vec::new();
    // A code span holding backticks takes a longer run of them, with a space inside.
    let mut tick = "`";
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let mut want: Vec<Mark> = marks.iter().filter(|m| m.start <= a && m.end >= b).map(|m| m.mark.clone()).collect();
        want.sort_by_key(Mark::depth);
        want.dedup();
        let keep = open.iter().zip(&want).take_while(|(o, w)| o == w).count();
        while open.len() > keep {
            if let Some(m) = open.pop() {
                out.push_str(&if m == Mark::Code { close_code(tick) } else { close(&m) });
            }
        }
        for m in &want[keep..] {
            if *m == Mark::Code {
                let span = marks.iter().find(|s| s.mark == Mark::Code && s.start <= a && s.end >= b);
                let ticked = span.is_some_and(|s| text[s.start..s.end].contains('`'));
                tick = if ticked { "``" } else { "`" };
                out.push_str(&open_code(tick));
            } else {
                out.push_str(opener(m));
            }
            open.push(m.clone());
        }
        let piece = &text[a..b];
        let chip = marks.iter().find_map(|m| match &m.mark {
            Mark::Chip(c) if m.start <= a && m.end >= b => Some((m.start, c)),
            _ => None,
        });
        if let Some((start, chip)) = chip {
            // A chip writes itself once, whatever its label says.
            if start == a {
                out.push_str(&chip_markdown(chip, assets));
            }
        } else if open.contains(&Mark::Code) {
            out.push_str(piece);
        } else {
            out.push_str(&escape(piece, a == 0));
        }
    }
    while let Some(m) = open.pop() {
        out.push_str(&if m == Mark::Code { close_code(tick) } else { close(&m) });
    }
    out
}

/// What a chip writes: `@login`, `#12`, an image or a link to an attached file.
fn chip_markdown(chip: &Chip, assets: Option<&str>) -> String {
    match chip {
        Chip::Mention(login) => format!("@{login}"),
        Chip::Pull(n) => format!("#{n}"),
        Chip::Task(done) => format!("[{}]", if *done { "x" } else { " " }),
        Chip::File(name) => {
            let url = assets.map_or_else(|| name.clone(), |base| format!("{}/{name}", base.trim_end_matches('/')));
            if Chip::is_image(name) { format!("![{name}]({url})") } else { format!("[{name}]({url})") }
        }
    }
}

fn open_code(tick: &str) -> String {
    if tick.len() > 1 { format!("{tick} ") } else { tick.to_owned() }
}

fn close_code(tick: &str) -> String {
    if tick.len() > 1 { format!(" {tick}") } else { tick.to_owned() }
}

fn opener(m: &Mark) -> &'static str {
    match m {
        Mark::Bold => "**",
        Mark::Italic => "_",
        Mark::Strike => "~~",
        Mark::Code => "`",
        Mark::Link(_) => "[",
        Mark::Chip(_) => "",
    }
}

fn close(m: &Mark) -> String {
    match m {
        Mark::Link(url) => format!("]({url})"),
        other => opener(other).to_owned(),
    }
}

/// Plain text made safe for markdown. `at_start`: this is the start of the line, where `#`, `>`
/// and list markers would start a block.
fn escape(text: &str, at_start: bool) -> String {
    let mut out = String::new();
    let mut seen = false;
    let mut word_start = true;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // A URL stays as it is, so GitHub links it.
        if word_start && (text_at(&chars, i, "https://") || text_at(&chars, i, "http://")) {
            while i < chars.len() && !chars[i].is_whitespace() {
                out.push(chars[i]);
                i += 1;
            }
            continue;
        }
        let first = at_start && !seen && !c.is_whitespace();
        seen |= !c.is_whitespace();
        let intraword =
            |j: usize| j > 0 && j + 1 < chars.len() && chars[j - 1].is_alphanumeric() && chars[j + 1].is_alphanumeric();
        let special = match c {
            '\\' | '*' | '`' | '[' | ']' | '<' | '~' => true,
            '_' => !intraword(i),
            '#' | '>' | '+' | '-' => first,
            _ => false,
        };
        if special {
            out.push('\\');
        }
        out.push(c);
        // `1.` at a line's start would number it.
        if first && c.is_ascii_digit() {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                out.push(chars[j]);
                j += 1;
            }
            if j < chars.len() && (chars[j] == '.' || chars[j] == ')') {
                out.push('\\');
            }
            i = j;
            word_start = false;
            continue;
        }
        word_start = c.is_whitespace() || c == '(';
        i += 1;
    }
    out
}

fn text_at(chars: &[char], i: usize, s: &str) -> bool {
    s.chars().enumerate().all(|(k, c)| chars.get(i + k) == Some(&c))
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
