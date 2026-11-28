//! Conflict markers, as git writes them into a file it couldn't merge:
//!
//! ```text
//! <<<<<<< HEAD
//! yours
//! ||||||| merged common ancestors      (only with merge.conflictStyle=diff3 or zdiff3)
//! the base
//! =======
//! theirs
//! >>>>>>> origin/main
//! ```
//!
//! [`parse`] splits a file into the text between conflicts and the conflicts themselves, keeping
//! every byte, so the file can be written back with any of them resolved.

/// One conflict: each side's lines, newline-terminated as in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub ours: String,
    /// The common ancestor's lines, when git wrote them (diff3 style).
    pub base: Option<String>,
    pub theirs: String,
    /// The block exactly as it stands in the file, markers and all.
    pub raw: String,
}

/// A stretch of a conflicted file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment {
    Text(String),
    Conflict(Hunk),
}

/// Whether `line` is the marker `mark` (seven of one character) followed by nothing or a space
/// and a label: `<<<<<<< HEAD`, but not `<<<<<<<<` or `<<<<<<<x`.
fn is_marker(line: &str, mark: char) -> bool {
    let line = line.trim_end_matches(['\n', '\r']);
    let n = line.chars().take_while(|c| *c == mark).count();
    n == 7 && line[7..].chars().next().is_none_or(|c| c == ' ' || c == '\t')
}

/// Splits `text` into its text and its conflicts, in order. A block that never ends (a stray
/// `<<<<<<<`) stays text. Text segments are never empty, and two never follow each other.
pub fn parse(text: &str) -> Vec<Segment> {
    enum In {
        Text,
        Ours,
        Base,
        Theirs,
    }
    let mut out: Vec<Segment> = Vec::new();
    let mut plain = String::new();
    let (mut ours, mut base, mut theirs, mut raw) = (String::new(), None::<String>, String::new(), String::new());
    let mut at = In::Text;
    for line in text.split_inclusive('\n') {
        match at {
            In::Text if is_marker(line, '<') => {
                raw = line.to_owned();
                at = In::Ours;
            }
            In::Text => plain.push_str(line),
            In::Ours | In::Base if is_marker(line, '=') => {
                raw.push_str(line);
                at = In::Theirs;
            }
            In::Ours if is_marker(line, '|') => {
                raw.push_str(line);
                base = Some(String::new());
                at = In::Base;
            }
            In::Ours => {
                raw.push_str(line);
                ours.push_str(line);
            }
            In::Base => {
                raw.push_str(line);
                if let Some(b) = base.as_mut() {
                    b.push_str(line);
                }
            }
            In::Theirs if is_marker(line, '>') => {
                raw.push_str(line);
                if !plain.is_empty() {
                    out.push(Segment::Text(std::mem::take(&mut plain)));
                }
                let hunk = Hunk {
                    ours: std::mem::take(&mut ours),
                    base: base.take(),
                    theirs: std::mem::take(&mut theirs),
                    raw: std::mem::take(&mut raw),
                };
                out.push(Segment::Conflict(hunk));
                at = In::Text;
            }
            In::Theirs => {
                raw.push_str(line);
                theirs.push_str(line);
            }
        }
    }
    // An unfinished block is text after all.
    plain.push_str(&raw);
    if !plain.is_empty() {
        out.push(Segment::Text(plain));
    }
    out
}

/// Whether `text` still holds a conflict.
pub fn has_conflicts(text: &str) -> bool {
    parse(text).iter().any(|s| matches!(s, Segment::Conflict(_)))
}

/// Whether any line of `text` is a conflict's opening or closing marker, finished block or not.
pub fn has_markers(text: &str) -> bool {
    text.split_inclusive('\n').any(|l| is_marker(l, '<') || is_marker(l, '>'))
}

/// Lines in `text`, counting a last line without its newline.
pub fn line_count(text: &str) -> usize {
    text.split_inclusive('\n').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conflicts(text: &str) -> Vec<Hunk> {
        parse(text)
            .into_iter()
            .filter_map(|s| match s {
                Segment::Conflict(h) => Some(h),
                Segment::Text(_) => None,
            })
            .collect()
    }

    fn joined(segments: &[Segment]) -> String {
        segments
            .iter()
            .map(|s| match s {
                Segment::Text(t) => t.as_str(),
                Segment::Conflict(h) => h.raw.as_str(),
            })
            .collect()
    }

    #[test]
    fn a_conflict_splits_into_both_sides_and_keeps_every_byte() {
        let text = "a\n<<<<<<< HEAD\nours 1\nours 2\n=======\ntheirs\n>>>>>>> origin/main\nb\n";
        let segments = parse(text);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0], Segment::Text("a\n".to_owned()));
        let [h] = conflicts(text).try_into().unwrap();
        assert_eq!((h.ours.as_str(), h.theirs.as_str(), h.base), ("ours 1\nours 2\n", "theirs\n", None));
        assert_eq!(segments[2], Segment::Text("b\n".to_owned()));
        assert_eq!(joined(&segments), text, "written back unchanged");
    }

    #[test]
    fn diff3_keeps_the_base_and_labels_with_paths_are_markers_too() {
        let text = "<<<<<<< HEAD:src/brass.json\nmine\n||||||| merged common ancestors\nold\n=======\n>>>>>>> origin/main:src/theme/brass.json\n";
        let [h] = conflicts(text).try_into().unwrap();
        assert_eq!(h.ours, "mine\n");
        assert_eq!(h.base.as_deref(), Some("old\n"));
        assert_eq!(h.theirs, "", "main deleted the lines");
        assert_eq!(joined(&parse(text)), text);
    }

    #[test]
    fn several_conflicts_in_order_and_lookalikes_stay_text() {
        let text =
            "<<<<<<< a\n1\n=======\n2\n>>>>>>> b\nmid\n========\n<<<<<<<< not one\n<<<<<<< c\n3\n=======\n4\n>>>>>>> d";
        let hunks = conflicts(text);
        assert_eq!(hunks.len(), 2);
        assert_eq!((hunks[1].ours.as_str(), hunks[1].theirs.as_str()), ("3\n", "4\n"), "the file ends on the marker");
        assert!(matches!(&parse(text)[1], Segment::Text(t) if t == "mid\n========\n<<<<<<<< not one\n"));
        assert_eq!(joined(&parse(text)), text);
    }

    #[test]
    fn an_unfinished_block_and_crlf_files() {
        let stray = "x\n<<<<<<< HEAD\ny\n";
        assert_eq!(parse(stray), vec![Segment::Text(stray.to_owned())]);
        assert!(!has_conflicts(stray));
        assert!(has_markers(stray), "but it still looks unfinished");
        assert!(!has_markers("=======\n"), "a lone rule is text");
        let crlf = "<<<<<<< HEAD\r\nours\r\n=======\r\ntheirs\r\n>>>>>>> main\r\n";
        let [h] = conflicts(crlf).try_into().unwrap();
        assert_eq!(h.ours, "ours\r\n");
        assert!(has_conflicts(crlf));
        assert_eq!(line_count("a\nb"), 2);
        assert_eq!(line_count(""), 0);
    }
}
