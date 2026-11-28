//! `gxm work [PATH]`: the repository's open work as the app's Work view lists it, as text:
//! pull requests that need the viewer's review, the viewer's own, and worktrees with no pull
//! request yet, each section oldest first, with why each is stuck; then every checkout.

use std::io::Write;
use std::path::PathBuf;

use gxm_core::env::Env;
use gxm_core::forge::{self, Forge};
use gxm_core::repo::RepoSummary;
use gxm_core::work::rules::{self, age, review_state};
use gxm_core::work::{Entry, Item, Next, Section, Tone, Work};

pub fn work(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    let mut env = env.clone();
    if env.forge.github_token.is_none() {
        env.forge.github_token = gxm_core::auth::token(&env).map_err(|e| e.to_string())?;
    }
    let env = &env;
    let path = args.first().map_or_else(|| PathBuf::from("."), PathBuf::from);
    let summary = RepoSummary::read(env, &path).map_err(|e| e.to_string())?;
    let forge = summary.origin.as_ref().and_then(|o| forge::connect(env, o).ok());
    let work = Work::read(env, &summary.path, forge.as_deref().map(|f| f as &dyn Forge))?;
    let text = render(env, &summary.name, &work);
    out.write_all(text.as_bytes()).map_err(|e| e.to_string())
}

fn render(env: &Env, name: &str, work: &Work) -> String {
    let mut s = format!(
        "{name} · {} to review · {} yours · {} with no PR yet\n",
        work.count(Section::Review),
        work.count(Section::Yours),
        match work.count(Section::Unproposed) {
            1 => "1 worktree".to_owned(),
            n => format!("{n} worktrees"),
        }
    );
    if work.open.is_none() {
        s.push_str("(not connected to the provider: worktrees only)\n");
    }
    for section in Section::ALL {
        let rows: Vec<&Entry> = work.entries.iter().filter(|e| e.section == section).collect();
        if rows.is_empty() {
            continue;
        }
        s.push_str(&format!("\n{}\n", section.title().to_uppercase()));
        for e in rows {
            s.push_str(&entry(env, work, e));
        }
    }
    s.push_str("\nCHECKOUTS\n");
    for w in &work.local.worktrees {
        let pull = w
            .branch
            .as_deref()
            .and_then(|b| work.open.as_ref()?.pulls.iter().find(|p| p.summary.head_ref == b).map(|p| p.summary.number));
        let branch = w.branch.clone().unwrap_or_else(|| "detached".to_owned());
        let branch = pull.map_or(branch.clone(), |n| format!("{branch} · #{n}"));
        let conflicts = pull.and_then(|n| work.conflict(n)).is_some();
        s.push_str(&format!(
            "  {} {:<20} {:<40} {}  {}\n",
            mark(w.tone(conflicts)),
            if w.main { "main checkout" } else { &w.name },
            branch,
            w.state(&work.local.base),
            env.tilde(&w.path),
        ));
    }
    s
}

fn entry(env: &Env, work: &Work, e: &Entry) -> String {
    let now = env.now();
    let base = &work.local.base;
    let age = age(e.since, now);
    match &e.item {
        Item::Pull(n) => {
            let Some(p) = work.pull(*n) else { return String::new() };
            let viewer = work.open.as_ref().map_or("", |o| o.viewer.login.as_str());
            let worktree = work.local.on_branch(&p.summary.head_ref);
            let conflict = work.conflict(*n);
            let tile = worktree
                .map_or_else(|| "no worktree".to_owned(), |w| format!("worktree {} ({})", w.name, w.state(base)));
            let mut meta = vec![];
            if e.section == Section::Review {
                meta.push(p.summary.author.name.clone());
            }
            meta.push(p.summary.head_ref.clone());
            if let Some((a, r)) = work.open.as_ref().and_then(|o| o.stats.get(n)) {
                meta.push(format!("+{a} −{r}"));
            }
            if e.section == Section::Yours {
                meta.push(review_state(p).0);
            }
            meta.push(tile);
            let action = match e.section {
                Section::Review => "Review".to_owned(),
                _ => match rules::next(p) {
                    Next::ResolveConflicts => "Resolve conflicts".to_owned(),
                    Next::Merge => "Merge".to_owned(),
                    Next::Wait(why) => format!("[{why}]"),
                },
            };
            let mut s = format!("  {age:>4}  #{n} {}\n        {} · → {action}\n", p.summary.title, meta.join(" · "));
            for r in rules::stuck(p, viewer, conflict) {
                s.push_str(&format!("        {} {}\n", mark(r.tone()), r.sentence(now)));
            }
            s
        }
        Item::Worktree(path) => {
            let Some(w) = work.local.worktrees.iter().find(|w| &w.path == path) else { return String::new() };
            let title = w.subject.clone().unwrap_or_else(|| w.branch.clone().unwrap_or_default());
            let mut meta = vec![w.branch.clone().unwrap_or_default()];
            if w.unpushed > 0 {
                meta.push(format!("{} unpushed", w.unpushed));
            }
            meta.push(format!("worktree {} ({})", w.name, w.state(base)));
            format!("  {age:>4}  {title}\n        {} · {} · → Open PR\n", meta.join(" · "), env.tilde(&w.path))
        }
    }
}

/// A tone as a character: red ✕, amber !, green ✓, quiet ·.
fn mark(tone: Tone) -> char {
    match tone {
        Tone::Bad => '✕',
        Tone::Warn => '!',
        Tone::Good => '✓',
        Tone::Quiet => '·',
    }
}
