//! A brief for an agent (Claude Code, Codex, whichever the person runs): what a piece of work
//! is, where it lives and what's in its way, as Markdown to paste into the agent's prompt.

use jiff::Timestamp;

use super::conflicts::Conflict;
use super::rules::{self, ThreadState};
use super::worktree::Worktree;
use crate::forge::model::{CheckState, Pull};

/// The brief for pull request `pull`, read as `viewer`, checked out at `place` (a path as a
/// person reads it) when it is.
pub fn pull(pull: &Pull, viewer: &str, conflict: Option<&Conflict>, place: Option<&str>, now: Timestamp) -> String {
    let s = &pull.summary;
    let mut out = format!("# #{} {}\n\n{}\n\n", s.number, s.title, s.url);
    out.push_str(&format!("Branch `{}` into `{}`", s.head_ref, s.base_ref));
    match place {
        Some(p) => out.push_str(&format!(", checked out in `{p}`.\n")),
        None => out.push_str(", not checked out on this computer.\n"),
    }
    let why = rules::stuck(pull, viewer, conflict);
    if !why.is_empty() {
        out.push_str("\n## Why it's stuck\n\n");
        for r in &why {
            out.push_str(&format!("- {}\n", r.sentence(now)));
        }
    }
    let failing: Vec<_> = pull.checks.iter().filter(|c| c.state == CheckState::Failed).collect();
    if !failing.is_empty() {
        out.push_str("\n## Failing checks\n\n");
        for c in failing {
            match c.title.as_str() {
                "" => out.push_str(&format!("- {}\n", c.name)),
                title => out.push_str(&format!("- {}: {title}\n", c.name)),
            }
        }
    }
    let threads: Vec<_> =
        rules::unresolved(pull, viewer).into_iter().filter(|(_, st)| *st == ThreadState::NeedsYou).collect();
    if !threads.is_empty() {
        out.push_str("\n## Review comments to answer\n\n");
        for (t, _) in threads {
            let place = t.line.map_or_else(|| format!("{} (outdated)", t.path), |l| format!("{}:{l}", t.path));
            out.push_str(&format!("- `{place}`\n"));
            for c in &t.comments {
                out.push_str(&format!("  - {}: {}\n", c.author.name, c.body.trim().replace('\n', " ")));
            }
        }
    }
    out
}

/// The brief for a worktree with no pull request yet, at `place`, against `base`.
pub fn worktree(w: &Worktree, base: &str, place: &str) -> String {
    let title = w.subject.clone().or_else(|| w.branch.clone()).unwrap_or_else(|| w.name.clone());
    let mut out = format!("# {title}\n\n");
    match &w.branch {
        Some(b) => out.push_str(&format!("Branch `{b}`, checked out in `{place}`, with no pull request yet.\n")),
        None => out.push_str(&format!("A detached checkout in `{place}`.\n")),
    }
    out.push_str(&format!("\nIts state: {}.\n", w.state(base)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::model::{Check, Comment, Mergeable, PullState, PullSummary, Side, Thread, User};

    fn user(login: &str, name: &str) -> User {
        User { login: login.to_owned(), name: name.to_owned() }
    }

    #[test]
    fn a_brief_says_where_the_work_is_and_what_is_in_its_way() {
        let now: Timestamp = "2026-11-11T15:00:00Z".parse().unwrap_or_else(|e| panic!("{e}"));
        let then: Timestamp = "2026-10-23T15:00:00Z".parse().unwrap_or_else(|e| panic!("{e}"));
        let kenji = user("kenji-mori", "Kenji Mori");
        let p = Pull {
            summary: PullSummary {
                number: 118,
                title: "Load avatars in parallel".to_owned(),
                state: PullState::Open,
                author: user("ana-ruiz", "Ana Ruiz"),
                head_ref: "perf/avatar-loading".to_owned(),
                head_sha: "abc".to_owned(),
                base_ref: "main".to_owned(),
                merge_commit: None,
                url: "https://github.com/studio/atlas-web/pull/118".to_owned(),
            },
            id: "PR".to_owned(),
            body: String::new(),
            created: then,
            commits: 3,
            pushed: Some(then),
            mergeable: Mergeable::Conflicts,
            reviewers: Vec::new(),
            checks: vec![Check {
                name: "e2e / safari".to_owned(),
                state: CheckState::Failed,
                title: "1 test failed".to_owned(),
                seconds: Some(252),
                job: None,
                run: None,
            }],
            threads: vec![Thread {
                id: "T".to_owned(),
                path: "src/avatars/loader.rs".to_owned(),
                line: Some(12),
                side: Side::New,
                resolved: false,
                comments: vec![Comment { author: kenji, body: "Fall back when busy?".to_owned(), at: then }],
            }],
            conversation: Vec::new(),
        };
        let conflict =
            Conflict { base: "main".to_owned(), files: vec!["src/avatars/cache.rs".to_owned()], since: Some(129) };
        let brief = pull_brief(&p, &conflict, now);
        assert_eq!(
            brief,
            "# #118 Load avatars in parallel\n\nhttps://github.com/studio/atlas-web/pull/118\n\n\
             Branch `perf/avatar-loading` into `main`, checked out in `~/worktrees/atlas-web/avatar-loading`.\n\n\
             ## Why it's stuck\n\n\
             - Conflicts with main in src/avatars/cache.rs, since #129 landed\n\
             - 1 check is failing\n\
             - You haven't answered Kenji's comment in 19 days\n\
             - No one is asked to review it\n\n\
             ## Failing checks\n\n- e2e / safari: 1 test failed\n\n\
             ## Review comments to answer\n\n- `src/avatars/loader.rs:12`\n  - Kenji Mori: Fall back when busy?\n"
        );
    }

    fn pull_brief(p: &Pull, c: &Conflict, now: Timestamp) -> String {
        pull(p, "ana-ruiz", Some(c), Some("~/worktrees/atlas-web/avatar-loading"), now)
    }
}
