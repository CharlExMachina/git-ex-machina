//! `cargo xtask pr`: a PR body carries its proof (CLAUDE.md, "Pull requests"). The body is
//! checked here before the PR exists, by the same code CI runs on the PR, so a body that passes
//! on your machine passes on CI. `open` is the way to create or update a PR: it refuses a body
//! without proof and attaches every file the body references.

use std::path::{Path, PathBuf};
use std::process::Command;

use gxm_journeys::catalog;

use crate::{root, run};

/// The crates whose changes a person sees: a PR touching one needs proof.
const USER_FACING: &[&str] = &["crates/gxm-app/", "crates/gxm-journeys/", "crates/gxm-core/", "crates/gxm-cli/"];

/// Files a PR body can embed or link, which `gh --attach` uploads.
const MEDIA: &[&str] = &[".gif", ".mp4", ".png"];

pub fn pr(args: &[String]) -> Result<(), String> {
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        Some("check") => check_cmd(rest),
        Some("open") => open_cmd(rest),
        _ => Err("pr takes `check BODY.md [--base REF]` or `open BODY.md [--base BRANCH] [--title TITLE]`".into()),
    }
}

/// `pr check BODY.md [--base REF]`: what CI runs on every PR, with REF the base commit.
fn check_cmd(args: &[String]) -> Result<(), String> {
    let body_file = positional(args).ok_or("pr check takes a BODY.md")?;
    let base = flag(args, "--base").unwrap_or("origin/main");
    let body = read(Path::new(body_file))?;
    require_proof(&body, &changed_since(base)?)?;
    println!("pr: the body carries its proof");
    Ok(())
}

/// `pr open BODY.md [--base BRANCH] [--title TITLE]`: checks the body, then creates this
/// branch's PR (or updates it, if it has one) with every file the body references attached.
fn open_cmd(args: &[String]) -> Result<(), String> {
    let body_file = PathBuf::from(positional(args).ok_or("pr open takes a BODY.md")?);
    let base = flag(args, "--base").unwrap_or("main");
    let body = read(&body_file)?;
    run(git().args(["fetch", "--quiet", "origin", base]))?;
    require_proof(&body, &changed_since(&format!("origin/{base}"))?)?;

    let dir = body_file.parent().unwrap_or(Path::new("."));
    let attachments = attachments(&body);
    let absent: Vec<_> = attachments.iter().filter(|a| !dir.join(a).is_file()).collect();
    if !absent.is_empty() {
        let list = absent.iter().map(|a| format!("  - {a}")).collect::<Vec<_>>().join("\n");
        return Err(format!(
            "the body references files that aren't next to it in {}:\n{list}\n\
             Copy them from target/proof/<journey>/.",
            dir.display()
        ));
    }

    // gh runs beside the body, so the body's relative references and the attachments agree.
    // The body may live outside the checkout, so the repository and branch are named.
    let repo = stdout(
        Command::new("gh")
            .args(["repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner"])
            .current_dir(root()),
    )?;
    let branch = stdout(git().args(["branch", "--show-current"]))?;
    let open_pr = stdout(Command::new("gh").args(["pr", "list", "--repo", &repo, "--head", &branch]).args([
        "--state",
        "open",
        "--json",
        "number",
        "--jq",
        ".[0].number // empty",
    ]))?;
    let mut gh = Command::new("gh");
    gh.current_dir(dir);
    if open_pr.is_empty() {
        let title = match flag(args, "--title") {
            Some(t) => t.to_owned(),
            None => first_commit_subject(base)?,
        };
        gh.args(["pr", "create", "--repo", &repo, "--head", &branch, "--base", base, "--title", &title]);
    } else {
        gh.args(["pr", "edit", &open_pr, "--repo", &repo]);
    }
    gh.arg("--body-file").arg(body_file.file_name().ok_or("BODY.md must be a file")?);
    for a in &attachments {
        gh.arg("--attach").arg(a);
    }
    run(&mut gh)
}

fn require_proof(body: &str, changed: &[String]) -> Result<(), String> {
    let journeys: Vec<String> = catalog::all().into_iter().map(|j| j.name.to_owned()).collect();
    let missing = missing(body, changed, &journeys);
    if missing.is_empty() {
        return Ok(());
    }
    let list = missing.iter().map(|m| format!("  - {m}")).collect::<Vec<_>>().join("\n");
    Err(format!("this PR needs proof (CLAUDE.md, \"Pull requests\"). Missing:\n{list}"))
}

/// What `body` lacks as proof for a PR changing `changed`, when `journeys` are the journeys
/// there are. Empty when it has everything.
pub fn missing(body: &str, changed: &[String], journeys: &[String]) -> Vec<String> {
    if !changed.iter().any(|f| USER_FACING.iter().any(|c| f.starts_with(c))) {
        return Vec::new();
    }
    // The template's hints and examples live in comments; they aren't proof.
    let body = without_comments(body);
    let mut missing = Vec::new();
    if !body.lines().any(|l| l.trim_end().starts_with("## Proof")) {
        missing.push("a `## Proof` section".to_owned());
    }
    match field(&body, "Journeys") {
        None => missing.push("a `**Journeys:**` line naming the journeys that prove it".to_owned()),
        Some(value) => {
            let named = journeys_named(value);
            if named.is_empty() {
                missing.push(
                    "the journeys that prove it, first on the `**Journeys:**` line: \
                     `first_look (extended), push_and_pull (new)`"
                        .to_owned(),
                );
            }
            for name in named.iter().filter(|n| !journeys.contains(n)) {
                missing.push(format!("a journey named `{name}`: there's none (`cargo xtask journeys` lists them)"));
            }
        }
    }
    if !(MEDIA.iter().any(|m| body.contains(m)) || body.contains("user-attachments")) {
        missing.push("a video or screenshots".to_owned());
    }
    if !body.contains("cargo xtask verify") {
        missing.push("the commands run, `cargo xtask verify` among them".to_owned());
    }
    if changed.iter().any(|f| f.contains("/goldens/")) {
        let said = field(&body, "Goldens changed").map(str::trim).unwrap_or_default();
        if said.is_empty() || said.to_lowercase().starts_with("none") {
            missing.push("each changed golden and why it changed, on the `**Goldens changed:**` line".to_owned());
        }
    }
    missing
}

/// The journeys a `**Journeys:**` line starts with: names, each with an optional note in
/// parentheses, separated by commas. Prose may follow. Names may be set as code.
fn journeys_named(value: &str) -> Vec<String> {
    let plain: String = value.chars().filter(|c| !matches!(c, '`' | '*')).collect();
    let mut names = Vec::new();
    let mut rest = plain.trim_start();
    loop {
        let end =
            rest.find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')).unwrap_or(rest.len());
        if end == 0 {
            break;
        }
        names.push(rest[..end].to_owned());
        rest = rest[end..].trim_start();
        if let Some(note) = rest.strip_prefix('(') {
            rest = note.find(')').map_or("", |i| &note[i + 1..]).trim_start();
        }
        match rest.strip_prefix(',') {
            Some(next) => rest = next.trim_start(),
            None => break,
        }
    }
    names
}

/// What follows `**NAME:**` on its line.
fn field<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let label = format!("**{name}:**");
    body.lines().find_map(|l| l.trim_start().strip_prefix(label.as_str()))
}

fn without_comments(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = rest[start..].find("-->").map_or("", |end| &rest[start + end + 3..]);
    }
    out.push_str(rest);
    out
}

/// The local media files the body references, as `](path)`: the files `gh --attach` uploads,
/// rewriting each reference to its upload.
fn attachments(body: &str) -> Vec<String> {
    let body = without_comments(body);
    let mut out: Vec<String> = Vec::new();
    for (i, _) in body.match_indices("](") {
        let target = &body[i + 2..];
        let Some(end) = target.find(')') else { continue };
        let target = target[..end].trim();
        let local = !target.contains("://");
        if local && MEDIA.iter().any(|m| target.ends_with(m)) && !out.iter().any(|o| o == target) {
            out.push(target.to_owned());
        }
    }
    out
}

fn changed_since(base: &str) -> Result<Vec<String>, String> {
    let changed = stdout(git().args(["diff", "--name-only", &format!("{base}...HEAD")]))?;
    Ok(changed.lines().map(str::to_owned).collect())
}

fn first_commit_subject(base: &str) -> Result<String, String> {
    let range = format!("origin/{base}..HEAD");
    let subjects = stdout(git().args(["log", "--reverse", "--format=%s", &range]))?;
    subjects.lines().next().map(str::to_owned).ok_or_else(|| format!("no commits in {range}; pass --title"))
}

/// A command's trimmed output, failing with its error when it fails.
fn stdout(cmd: &mut Command) -> Result<String, String> {
    let shown = format!("{cmd:?}");
    let out = cmd.output().map_err(|e| format!("{shown}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{shown}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn git() -> Command {
    let mut c = Command::new("git");
    c.current_dir(root());
    c
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn positional(args: &[String]) -> Option<&str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a.starts_with("--") {
            it.next();
        } else {
            return Some(a);
        }
    }
    None
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| (*x).to_owned()).collect()
    }

    fn body(journeys: &str) -> String {
        format!(
            "## What and why\n\nIt does a thing.\n\n## Proof\n\n**Journeys:** {journeys}\n\n\
             ![get_started](./get_started.gif)\n\n```\ncargo xtask verify → all green\n```\n\n\
             **Goldens changed:** none.\n"
        )
    }

    fn missing_for(body: &str) -> Vec<String> {
        let journeys = strings(&["get_started", "sign_in", "review_pull_request", "merge_pull_request"]);
        missing(body, &strings(&["crates/gxm-app/src/ui.rs"]), &journeys)
    }

    #[test]
    fn names_set_as_code_count() {
        // The lines PRs #3, #4 and #5 were refused for: CI's old check wanted bare names.
        assert!(missing_for(&body("`get_started` (new): empty workspace → Add refuses `~/code/scratch`")).is_empty());
        assert!(missing_for(&body("`get_started` (extended). The flow:")).is_empty());
        assert!(missing_for(&body("**sign_in** (new). Each step and what it shows:")).is_empty());
    }

    #[test]
    fn a_list_of_journeys_reads_up_to_the_prose() {
        let line = "review_pull_request (new), merge_pull_request (new). The journey runner learns `scroll_to(key)`.";
        assert_eq!(journeys_named(line), strings(&["review_pull_request", "merge_pull_request"]));
        assert_eq!(
            journeys_named("first_look (extended: steps 12–18), `sign_in`"),
            strings(&["first_look", "sign_in"])
        );
        assert_eq!(journeys_named("get_started and its marks"), strings(&["get_started"]));
        assert!(journeys_named("Get started, new").is_empty());
    }

    #[test]
    fn journeys_must_exist() {
        assert_eq!(
            missing_for(&body("none")),
            strings(&["a journey named `none`: there's none (`cargo xtask journeys` lists them)"])
        );
        assert_eq!(missing_for(&body("get_startd (new)")).len(), 1);
        assert_eq!(missing_for(&body("")).len(), 1);
    }

    #[test]
    fn the_untouched_template_is_not_proof() {
        let template = include_str!("../../.github/pull_request_template.md");
        let missing = missing_for(template);
        assert_eq!(missing.len(), 2, "{missing:?}");
        assert!(missing[0].starts_with("the journeys that prove it"), "{missing:?}");
        assert_eq!(missing[1], "a video or screenshots");
    }

    #[test]
    fn goldens_that_change_need_a_reason() {
        let journeys = strings(&["get_started"]);
        let changed = strings(&["crates/gxm-app/src/ui.rs", "crates/gxm-app/tests/goldens/history.txt"]);
        let reason = body("get_started").replace("none.", "history.txt: the badge moved left");
        assert_eq!(missing(&body("get_started"), &changed, &journeys).len(), 1);
        assert!(missing(&reason, &changed, &journeys).is_empty());
    }

    #[test]
    fn only_user_facing_changes_need_proof() {
        assert!(missing("no proof at all", &strings(&["README.md", "xtask/src/main.rs"]), &[]).is_empty());
    }

    #[test]
    fn attachments_are_the_local_media_the_body_references() {
        let b = "![a](./a.gif) [a.mp4](./a.mp4) ![b](https://example.com/b.png) ![a](./a.gif) \
                 <!-- ![t](./t.gif) --> [doc](./notes.md)";
        assert_eq!(attachments(b), strings(&["./a.gif", "./a.mp4"]));
    }
}
