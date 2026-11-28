//! One conflicted file: the text between its conflicts and the conflicts themselves, each with
//! its resolution once it has one. What's on disk is always [`File::render`]; reading the file
//! again after someone else changed it (an agent) is [`File::reconcile`].

use super::markers::{self, Hunk, Segment};
use super::scope;
use super::{By, Conflict, Resolution};

/// Lines of context each pane shows around a conflict.
pub const CONTEXT: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// From the worktree's root: `src/sidebar.rs`.
    pub path: String,
    /// `texts[i]` comes before `conflicts[i]`; the last one ends the file. Always one more
    /// than the conflicts.
    pub texts: Vec<String>,
    pub conflicts: Vec<Conflict>,
    /// The file as someone left it, when it could no longer be lined up with its conflicts (an
    /// agent rewrote the lines around them): it's written as it is, and its conflicts are
    /// resolved only once it holds no markers.
    pub verbatim: Option<String>,
}

impl File {
    /// `text`, as git left it with its conflicts.
    pub fn parse(path: &str, text: &str) -> File {
        let mut texts = vec![String::new()];
        let mut conflicts = Vec::new();
        for s in markers::parse(text) {
            match s {
                Segment::Text(t) => texts.last_mut().map(|l| l.push_str(&t)).unwrap_or_default(),
                Segment::Conflict(hunk) => {
                    conflicts.push(Conflict { hunk, scope: None, why: None, resolution: None });
                    texts.push(String::new());
                }
            }
        }
        let mut f = File { path: path.to_owned(), texts, conflicts, verbatim: None };
        for i in 0..f.conflicts.len() {
            let before = f.before(i);
            let lines: Vec<&str> = before.lines().collect();
            let hunk = &f.conflicts[i].hunk;
            let inside: Vec<&str> = hunk.ours.lines().chain(hunk.theirs.lines()).collect();
            let found = scope::enclosing(lines.iter().copied(), &inside);
            f.conflicts[i].scope = found;
        }
        f
    }

    /// The file as it is to be written: each conflict resolved, or still marked.
    pub fn render(&self) -> String {
        if let Some(v) = &self.verbatim {
            return v.clone();
        }
        let mut out = self.texts[0].clone();
        for (c, after) in self.conflicts.iter().zip(&self.texts[1..]) {
            out.push_str(c.text());
            out.push_str(after);
        }
        out
    }

    /// Everything before conflict `i`, as rendered.
    fn before(&self, i: usize) -> String {
        let mut out = self.texts[0].clone();
        for (c, after) in self.conflicts[..i].iter().zip(&self.texts[1..]) {
            out.push_str(c.text());
            out.push_str(after);
        }
        out
    }

    /// Everything after conflict `i`, as rendered.
    fn after(&self, i: usize) -> String {
        let mut out = self.texts[i + 1].clone();
        for (c, after) in self.conflicts[i + 1..].iter().zip(&self.texts[i + 2..]) {
            out.push_str(c.text());
            out.push_str(after);
        }
        out
    }

    /// The line conflict `i` starts on, 1-based, in the file as rendered.
    pub fn line_of(&self, i: usize) -> usize {
        markers::line_count(&self.before(i)) + 1
    }

    /// The lines just before conflict `i` and just after it, for the panes: up to
    /// [`CONTEXT`] each, as rendered.
    pub fn context(&self, i: usize) -> (Vec<String>, Vec<String>) {
        let before = self.before(i);
        let lines: Vec<&str> = before.lines().collect();
        let above = lines[lines.len().saturating_sub(CONTEXT)..].iter().map(|l| (*l).to_owned()).collect();
        let below = self.after(i).lines().take(CONTEXT).map(str::to_owned).collect();
        (above, below)
    }

    /// Conflicts not resolved yet.
    pub fn open(&self) -> usize {
        self.conflicts.iter().filter(|c| c.resolution.is_none()).count()
    }

    /// The file as `disk` now holds it, after someone else changed it: each conflict whose
    /// markers are gone is resolved by them with what stands in its place, each still marked
    /// stays open. The text between conflicts anchors each one; when that text changed too, the
    /// file is kept [`verbatim`](File::verbatim).
    pub fn reconcile(&self, disk: &str, by: By) -> File {
        if disk == self.render() {
            return self.clone();
        }
        match self.line_up(disk, by) {
            Some(f) => f,
            None => {
                let marked = markers::has_conflicts(disk);
                let mut f = self.clone();
                f.verbatim = Some(disk.to_owned());
                for c in &mut f.conflicts {
                    c.resolution = if marked { None } else { Some(Resolution { text: String::new(), by }) };
                }
                f
            }
        }
    }

    /// Finds each text between conflicts in `disk`, in order; what lies between two is the
    /// conflict between them.
    fn line_up(&self, disk: &str, by: By) -> Option<File> {
        if self.verbatim.is_some() {
            return None;
        }
        let first = &self.texts[0];
        let mut at = disk.strip_prefix(first.as_str()).map(|_| first.len())?;
        let mut f = self.clone();
        let last = self.texts.len() - 1;
        for i in 0..self.conflicts.len() {
            let next = &self.texts[i + 1];
            let end = if i + 1 == last {
                // The file's end: what follows the last conflict.
                disk.len().checked_sub(next.len()).filter(|e| *e >= at && disk[*e..] == **next)?
            } else if next.is_empty() {
                return None;
            } else {
                at + disk[at..].find(next.as_str())?
            };
            let span = &disk[at..end];
            let c = &mut f.conflicts[i];
            if span != c.text() {
                match markers::parse(span).as_slice() {
                    [Segment::Conflict(h)] => {
                        c.hunk = Hunk {
                            ours: h.ours.clone(),
                            base: h.base.clone(),
                            theirs: h.theirs.clone(),
                            raw: h.raw.clone(),
                        };
                        c.resolution = None;
                    }
                    _ if markers::has_conflicts(span) => return None,
                    _ => c.resolution = Some(Resolution { text: span.to_owned(), by }),
                }
            }
            at = end + next.len();
        }
        Some(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = "fn a() {\n    one();\n<<<<<<< HEAD\n    ours();\n=======\n    theirs();\n>>>>>>> main\n}\n\nfn b() {\n<<<<<<< HEAD\n    mine();\n=======\n    yours();\n>>>>>>> main\n}\n";

    fn resolve(f: &mut File, i: usize, text: &str) {
        f.conflicts[i].resolution = Some(Resolution { text: text.to_owned(), by: By::Hand });
    }

    #[test]
    fn a_file_renders_as_it_was_until_a_conflict_is_resolved() {
        let mut f = File::parse("src/a.rs", TWO);
        assert_eq!(f.conflicts.len(), 2);
        assert_eq!(f.render(), TWO);
        assert_eq!((f.line_of(0), f.line_of(1)), (3, 11));
        assert_eq!(f.conflicts[0].scope.as_deref(), Some("fn a"));
        assert_eq!(f.conflicts[1].scope.as_deref(), Some("fn b"));
        let (above, below) = f.context(1);
        assert_eq!(above, ["}", "", "fn b() {"]);
        assert_eq!(below, ["}"]);
        resolve(&mut f, 0, "    ours();\n    theirs();\n");
        assert_eq!(f.open(), 1);
        assert!(f.render().starts_with("fn a() {\n    one();\n    ours();\n    theirs();\n}\n"));
        assert_eq!(f.line_of(1), 8, "the next conflict moves up as the first one shrinks");
    }

    #[test]
    fn an_agents_edits_line_up_with_the_conflicts_they_resolved() {
        let f = File::parse("src/a.rs", TWO);
        // The agent resolved the second conflict only.
        let disk = TWO.replace("<<<<<<< HEAD\n    mine();\n=======\n    yours();\n>>>>>>> main\n", "    both();\n");
        let g = f.reconcile(&disk, By::Agent);
        assert_eq!(g.open(), 1);
        assert_eq!(g.conflicts[1].resolution, Some(Resolution { text: "    both();\n".to_owned(), by: By::Agent }));
        assert_eq!(g.render(), disk);
        // Then the first, with an extra line of its own.
        let disk2 = disk.replace(
            "<<<<<<< HEAD\n    ours();\n=======\n    theirs();\n>>>>>>> main\n",
            "    ours();\n    // and theirs\n",
        );
        let h = g.reconcile(&disk2, By::Agent);
        assert_eq!(h.open(), 0);
        assert_eq!(
            h.conflicts[0].resolution.as_ref().map(|r| r.text.as_str()),
            Some("    ours();\n    // and theirs\n")
        );
        assert_eq!(h.render(), disk2);
        assert_eq!(f.reconcile(TWO, By::Agent), f, "nothing changed");
    }

    #[test]
    fn rewritten_around_its_conflicts_the_file_is_kept_as_it_is() {
        let f = File::parse("src/a.rs", TWO);
        let rewritten = "fn a() { ours(); theirs(); }\nfn b() { mine(); }\n";
        let g = f.reconcile(rewritten, By::Agent);
        assert_eq!(g.verbatim.as_deref(), Some(rewritten));
        assert_eq!(g.open(), 0);
        assert_eq!(g.render(), rewritten);
        let still = format!("// moved\n{TWO}");
        let h = f.reconcile(&still, By::Agent);
        assert_eq!(h.open(), 2, "markers still there: still open");
        assert_eq!(h.render(), still);
    }
}
