//! The world's "Generate release notes": GitHub's notes for a tag, written from the real history
//! in the world's remote. Each commit whose subject ends in `(#N)` is pull request N (squash
//! merged, so its title is the subject), by the person whose name the commit carries; someone
//! with no commit before the previous tag is a new contributor.

use serde_json::{Value, json};

use super::rest::{Answer, invalid, ok};
use super::{ANA, KENJI, LEA, State};

pub(super) fn generate(s: &State, owner: &str, name: &str, body: &Value) -> Answer {
    let bare = s.remotes.join("github.com").join(owner).join(format!("{name}.git"));
    let git = |args: &[&str]| gxm_core::git::run(&s.env, &bare, args);
    let (Some(tag), Some(target)) = (body["tag_name"].as_str(), body["target_commitish"].as_str()) else {
        return invalid("tag_name and target_commitish are required");
    };
    if git(&["rev-parse", "--verify", "--quiet", &format!("{target}^{{commit}}")]).is_err() {
        return (404, json!({ "message": "Not Found" }).to_string());
    }
    let previous = body["previous_tag_name"].as_str();
    let range = previous.map_or_else(|| target.to_owned(), |p| format!("{p}..{target}"));
    let log = git(&["log", "--reverse", "--format=%an%x1f%s", &range]).unwrap_or_default();
    let before = previous.and_then(|p| git(&["log", "--format=%an", p]).ok()).unwrap_or_default();
    let base = format!("https://github.com/{owner}/{name}");
    let mut changed = Vec::new();
    let mut new = Vec::new();
    for line in log.lines() {
        let Some((author, subject)) = line.split_once('\x1f') else { continue };
        let Some((title, number)) = subject.strip_suffix(')').and_then(|s| s.rsplit_once(" (#")) else { continue };
        let login = [ANA, KENJI, LEA].into_iter().find(|p| p.name == author).map_or(author, |p| p.login);
        changed.push(format!("* {title} by @{login} in {base}/pull/{number}"));
        let first = !before.lines().any(|a| a == author) && !new.iter().any(|(l, _)| *l == login);
        if first && previous.is_some() {
            new.push((login, number.to_owned()));
        }
    }
    let mut notes = String::from("## What's Changed\n");
    notes.push_str(&changed.join("\n"));
    if !new.is_empty() {
        notes.push_str("\n\n## New Contributors\n");
        let lines: Vec<String> =
            new.iter().map(|(l, n)| format!("* @{l} made their first contribution in {base}/pull/{n}")).collect();
        notes.push_str(&lines.join("\n"));
    }
    match previous {
        Some(p) => notes.push_str(&format!("\n\n**Full Changelog**: {base}/compare/{p}...{tag}")),
        None => notes.push_str(&format!("\n\n**Full Changelog**: {base}/commits/{tag}")),
    }
    ok(json!({ "name": tag, "body": notes }))
}
