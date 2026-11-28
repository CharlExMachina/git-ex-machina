//! What the coding agent is asked: the merge under way, each conflict still open (where it is,
//! both sides, and why the base changed it), and what to do and not do. The person reviews what
//! it wrote and commits the merge themselves.

use std::fmt::Write as _;

use super::{Merge, Target, Why};

/// A fence for `text` that `text` can't close: one backtick more than its longest run.
fn fence(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    "`".repeat(longest.max(2) + 1)
}

fn block(out: &mut String, title: &str, text: &str) {
    let f = fence(text);
    let body =
        if text.is_empty() { "(nothing: this side deleted these lines)\n".to_owned() } else { super::ended(text) };
    write!(out, "{title}:\n\n{f}\n{body}{f}\n\n").unwrap_or_default();
}

/// Why the base changed the lines, as one sentence.
pub fn why_line(why: &Why) -> String {
    match &why.pull {
        Some(p) => {
            let reason = p.reason.as_ref().map(|r| format!(": {r}")).unwrap_or_default();
            format!("#{} {} (by {}){reason}", p.number, p.title, p.author)
        }
        None => format!(
            "commit {} {} (by {}, no pull request)",
            &why.commit[..why.commit.len().min(7)],
            why.subject,
            why.author
        ),
    }
}

/// The prompt for the conflicts `merge` has open.
pub fn build(target: &Target, merge: &Merge) -> String {
    let base = target.base_ref();
    let open: Vec<_> = merge
        .files
        .iter()
        .flat_map(|f| f.conflicts.iter().enumerate().map(move |(i, c)| (f, i, c)))
        .filter(|(_, _, c)| c.resolution.is_none())
        .collect();
    let files = {
        let mut paths: Vec<&str> = open.iter().map(|(f, _, _)| f.path.as_str()).collect();
        paths.dedup();
        paths
    };
    let mut out = String::new();
    writeln!(
        out,
        "You're resolving merge conflicts in a git worktree, for a person who will review your work before it's committed.\n"
    ).unwrap_or_default();
    writeln!(
        out,
        "## The merge\n\n`{base}` (`{}`, just fetched) is being merged into `{}`, the branch of pull request #{} \"{}\", in this folder. Git stopped on the conflicts below; the merge is not committed.\n",
        target.base, target.branch, target.number, target.title
    ).unwrap_or_default();
    let done = merge.total() - merge.open();
    if done > 0 {
        writeln!(
            out,
            "The person already resolved {done} other conflict{}; leave those lines as they are.\n",
            if done == 1 { "" } else { "s" }
        )
        .unwrap_or_default();
    }
    writeln!(out, "## The conflicts ({} in {})\n", open.len(), files.join(", ")).unwrap_or_default();
    for (n, (f, i, c)) in open.iter().enumerate() {
        let scope = c.scope.as_ref().map(|s| format!(", in `{s}`")).unwrap_or_default();
        writeln!(out, "### {}. {}:{}{scope}\n", n + 1, f.path, f.line_of(*i)).unwrap_or_default();
        block(&mut out, &format!("This branch (`{}`)", target.branch), &c.hunk.ours);
        block(&mut out, &format!("`{}`", target.base), &c.hunk.theirs);
        if let Some(why) = &c.why {
            writeln!(out, "Why {} changed it: {}\n", target.base, why_line(why)).unwrap_or_default();
        }
    }
    writeln!(
        out,
        "## What to do\n\n\
         1. Resolve every conflict marker (`<<<<<<<`, `=======`, `>>>>>>>`) in {}, keeping the intent of both sides: what this branch set out to do, and what `{}` changed meanwhile. Take one side only when the other is obsolete.\n\
         2. If the project has an obvious build or test command that runs quickly (`cargo check`, `npm test`, `go build`, …), run it and fix what the merge broke.\n\
         3. `git add` each file you resolved.\n\
         4. Don't commit, don't push, and don't run `git merge --abort`, `git reset` or `git checkout`: the person reviews your changes in git-ex-machina and commits the merge there.\n\
         5. Finish with a short summary: for each conflict, what you kept and why.",
        files.join(", "),
        target.base
    ).unwrap_or_default();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{By, File, Resolution, WhyPull};

    #[test]
    fn the_prompt_names_each_open_conflict_both_sides_and_why() {
        let target = Target {
            repo: "/r".into(),
            worktree: "/w".into(),
            worktree_name: "avatar-loading".to_owned(),
            branch: "perf/avatar-loading".to_owned(),
            base: "main".to_owned(),
            remote: "origin".to_owned(),
            number: 118,
            title: "Load avatars in parallel".to_owned(),
        };
        let text = "fn load() {\n<<<<<<< HEAD\n    pool.load_all(); // ```\n=======\n    cache.evict();\n>>>>>>> origin/main\n}\n";
        let mut file = File::parse("src/cache.rs", text);
        file.conflicts[0].why = Some(Why {
            commit: "76245e5aaaa".to_owned(),
            subject: "Fix avatar cache eviction".to_owned(),
            author: "Lea Brandt".to_owned(),
            at: jiff::Timestamp::UNIX_EPOCH,
            pull: Some(WhyPull {
                number: 129,
                title: "Fix avatar cache eviction".to_owned(),
                author: "Lea Brandt".to_owned(),
                reason: Some("Released as v0.4.0.".to_owned()),
                merged: None,
            }),
        });
        let mut done = File::parse("src/other.rs", "<<<<<<< HEAD\na\n=======\nb\n>>>>>>> x\n");
        done.conflicts[0].resolution = Some(Resolution { text: "a\n".to_owned(), by: By::Hand });
        let merge = Merge {
            head: "h".to_owned(),
            base_commit: "b".to_owned(),
            files: vec![done, file],
            regenerated: Vec::new(),
            lockfiles: Vec::new(),
            elsewhere: Vec::new(),
        };
        let p = build(&target, &merge);
        for want in [
            "`origin/main` (`main`, just fetched) is being merged into `perf/avatar-loading`, the branch of pull request #118 \"Load avatars in parallel\"",
            "The person already resolved 1 other conflict; leave those lines as they are.",
            "## The conflicts (1 in src/cache.rs)",
            "### 1. src/cache.rs:2, in `fn load`",
            "````\n    pool.load_all(); // ```\n````",
            "Why main changed it: #129 Fix avatar cache eviction (by Lea Brandt): Released as v0.4.0.",
            "3. `git add` each file you resolved.",
            "Don't commit, don't push",
        ] {
            assert!(p.contains(want), "missing {want:?} in:\n{p}");
        }
        assert!(!p.contains("src/other.rs"), "resolved files aren't asked about");
    }
}
