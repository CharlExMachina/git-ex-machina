use super::*;

/// What GitHub's "Generate release notes" writes.
const GENERATED: &str = "\
## What's Changed
* Print progress while uploading by @kenji-mori in https://github.com/studio/beacon/pull/5
* Fix the --json flag by @lea-brandt in https://github.com/studio/beacon/pull/6

## New Contributors
* @lea-brandt made their first contribution in https://github.com/studio/beacon/pull/6

**Full Changelog**: https://github.com/studio/beacon/compare/v0.1.1...v0.1.2";

fn body(text: &str, lines: &[Line], marks: &[(usize, usize, Mark)]) -> Part {
    let marks = marks.iter().map(|(s, e, m)| Span { start: *s, end: *e, mark: m.clone() }).collect();
    Part::Body(Rich { text: text.to_owned(), marks, lines: lines.to_vec() })
}

#[test]
fn github_drafts_read_as_headings_lists_and_bold() {
    let notes = parse(GENERATED);
    let [Part::Heading(2, changed), Part::Body(items), Part::Heading(2, new), Part::Body(rest)] =
        notes.parts.as_slice()
    else {
        panic!("{:#?}", notes.parts);
    };
    assert_eq!((changed.text.as_str(), new.text.as_str()), ("What's Changed", "New Contributors"));
    assert_eq!(items.lines, [Line::Bullet(0), Line::Bullet(0)]);
    assert!(items.text.starts_with("Print progress while uploading by @kenji-mori in https://"));
    assert_eq!(rest.lines, [Line::Bullet(0), Line::Paragraph]);
    let last = rest.text.lines().last().unwrap();
    assert_eq!(last, "Full Changelog: https://github.com/studio/beacon/compare/v0.1.1...v0.1.2");
    let bold = rest.marks.iter().find(|m| m.mark == Mark::Bold).unwrap();
    assert_eq!(&rest.text[bold.start..bold.end], "Full Changelog");
}

#[test]
fn a_draft_leads_with_a_summary_and_folds_github_s_notes_away() {
    let notes = draft("v0.1.2", GENERATED);
    let Part::Body(lead) = &notes.parts[0] else { panic!("{:#?}", notes.parts) };
    assert_eq!(lead.text, "v0.1.2 · 2 changes from 2 contributors\n");
    assert_eq!(lead.lines, [Line::Paragraph, Line::Bullet(0)], "an empty bullet for the highlights");
    assert_eq!(notes.parts[1], Part::Fold("Full changelog".to_owned()));
    assert_eq!(draft("v1.0.0", "## What's Changed\n* One by @ana in x\n").parts[0], {
        body("v1.0.0 · 1 change from 1 contributor\n", &[Line::Paragraph, Line::Bullet(0)], &[(0, 6, Mark::Bold)])
    });
}

#[test]
fn notes_write_as_the_markdown_github_shows() {
    let mut notes = draft("v0.1.2", GENERATED);
    // The highlight, and a heading after it, as the sheet writes them.
    notes.parts[0] = body(
        "v0.1.2 · 2 changes from 2 contributors\nFix the --json flag. It prints JSON again.",
        &[Line::Paragraph, Line::Bullet(0)],
        &[(0, 6, Mark::Bold), (39, 60, Mark::Bold)],
    );
    notes.parts.insert(1, Part::Heading(3, Rich::plain("Plus a steadier progress bar")));
    assert_eq!(
        markdown(&notes),
        "\
**v0.1.2** · 2 changes from 2 contributors

- **Fix the --json flag.** It prints JSON again.

### Plus a steadier progress bar

<details>
<summary>Full changelog</summary>

## What's Changed

- Print progress while uploading by @kenji-mori in https://github.com/studio/beacon/pull/5
- Fix the --json flag by @lea-brandt in https://github.com/studio/beacon/pull/6

## New Contributors

- @lea-brandt made their first contribution in https://github.com/studio/beacon/pull/6

**Full Changelog**: https://github.com/studio/beacon/compare/v0.1.1...v0.1.2

</details>
"
    );
}

#[test]
fn marks_nest_outermost_first_and_keep_spaces_outside() {
    let line = |text: &str, marks: &[(usize, usize, Mark)]| {
        markdown(&Notes { parts: vec![body(text, &[Line::Paragraph], marks)] })
    };
    assert_eq!(line("bold and italic", &[(0, 15, Mark::Bold), (5, 15, Mark::Italic)]), "**bold _and italic_**\n");
    assert_eq!(line("bold then", &[(0, 5, Mark::Bold)]), "**bold** then\n", "the space stays outside");
    let link = Mark::Link("https://example.com".to_owned());
    assert_eq!(line("see the docs", &[(4, 12, link), (8, 12, Mark::Code)]), "see [the `docs`](https://example.com)\n");
    assert_eq!(line("run `x`", &[(4, 7, Mark::Code)]), "run `` `x` ``\n", "backticks inside code");
    assert_eq!(line("~~gone~~", &[(0, 8, Mark::Strike)]), "~~\\~\\~gone\\~\\~~~\n");
}

#[test]
fn text_that_looks_like_markdown_is_escaped_but_urls_and_words_are_not() {
    let line = |text: &str| markdown(&Notes { parts: vec![Part::Body(Rich::plain(text))] });
    assert_eq!(line("# not a heading"), "\\# not a heading\n");
    assert_eq!(line("- not a list"), "\\- not a list\n");
    assert_eq!(line("1. not numbered"), "1\\. not numbered\n");
    assert_eq!(line("2 * 3 = [6]"), "2 \\* 3 = \\[6\\]\n");
    assert_eq!(line("watch_pull_request _x_"), "watch_pull_request \\_x\\_\n");
    assert_eq!(line("in https://x.dev/a_b*c"), "in https://x.dev/a_b*c\n");
}

#[test]
fn lists_nest_under_their_parents_and_number_per_level() {
    let notes = Notes {
        parts: vec![body(
            "one\ntwo\nunder two\nunder two again\ndeeper\nthree\n\nfresh",
            &[
                Line::Numbered(0),
                Line::Numbered(0),
                Line::Bullet(1),
                Line::Bullet(1),
                Line::Numbered(2),
                Line::Numbered(0),
                Line::Paragraph,
                Line::Numbered(0),
            ],
            &[],
        )],
    };
    assert_eq!(
        markdown(&notes),
        "1. one\n2. two\n   - under two\n   - under two again\n     1. deeper\n3. three\n\n1. fresh\n"
    );
    let back = parse(&markdown(&notes));
    let Part::Body(read) = &back.parts[0] else { panic!() };
    assert_eq!(
        read.lines,
        [
            Line::Numbered(0),
            Line::Numbered(0),
            Line::Bullet(1),
            Line::Bullet(1),
            Line::Numbered(2),
            Line::Numbered(0),
            Line::Numbered(0)
        ]
    );
}

#[test]
fn quotes_code_and_sections_round_trip() {
    let notes = Notes {
        parts: vec![
            body("Heads up: **breaking**\nlet x = 1;\nfn y() {}", &[Line::Quote, Line::Code, Line::Code], &[]),
            Part::Fold("Details & more".to_owned()),
            Part::Heading(2, Rich::plain("Inside")),
            body("text", &[Line::Paragraph], &[]),
        ],
    };
    let md = markdown(&notes);
    assert_eq!(
        md,
        "> Heads up: \\*\\*breaking\\*\\*\n\n```\nlet x = 1;\nfn y() {}\n```\n\n<details>\n<summary>Details &amp; more</summary>\n\n## Inside\n\ntext\n\n</details>\n"
    );
    assert_eq!(parse(&md), notes);
}

#[test]
fn inline_markdown_reads_back_as_marks() {
    let (text, marks) = read::inline("a **bold _both_** `co*de` [link](https://x.dev) ~~s~~ snake_case \\*lit\\*");
    assert_eq!(text, "a bold both co*de link s snake_case *lit*");
    let got: Vec<(&str, Mark)> = marks.iter().map(|m| (&text[m.start..m.end], m.mark.clone())).collect();
    assert_eq!(
        got,
        [
            ("both", Mark::Italic),
            ("bold both", Mark::Bold),
            ("co*de", Mark::Code),
            ("link", Mark::Link("https://x.dev".to_owned())),
            ("s", Mark::Strike),
        ]
    );
}

#[test]
fn rich_text_splits_and_joins_by_line_keeping_marks() {
    let Part::Body(r) =
        body("one\ntwo **x**\nthree", &[Line::Bullet(0), Line::Paragraph, Line::Quote], &[(4, 9, Mark::Bold)])
    else {
        unreachable!()
    };
    let middle = r.lines_between(1, 2).unwrap();
    assert_eq!((middle.text.as_str(), middle.lines.as_slice()), ("two **x**", &[Line::Paragraph][..]));
    assert_eq!(middle.marks, [Span { start: 0, end: 5, mark: Mark::Bold }]);
    let tail = r.lines_between(1, 3).unwrap();
    assert_eq!((tail.text.as_str(), tail.lines.as_slice()), ("two **x**\nthree", &[Line::Paragraph, Line::Quote][..]));
    assert_eq!(r.lines_between(3, 4), None);
    let mut head = r.lines_between(0, 1).unwrap();
    head.append(&tail);
    assert_eq!(head, r);
    let mut first = r.lines_between(0, 1).unwrap();
    first.extend_line(&middle);
    assert_eq!((first.text.as_str(), first.marks[0].start), ("onetwo **x**", 3));
}
