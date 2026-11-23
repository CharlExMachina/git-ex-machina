//! Releases come from tags. A tag `vX.Y.Z` names the version it releases: the release builds
//! write it into the workspace's `Cargo.toml` before they build (`version`), so nothing on main is
//! bumped by hand. The notes are the tag's message when it was pushed annotated with one, or else
//! the pull requests merged since the last release (`notes`).

use crate::{flag, git, root, stdout};

/// The version to build. With `--tag` (as the release workflow passes it) that's the tag's, written
/// into the workspace's `Cargo.toml` so every crate, and the app's updater, builds as it; without,
/// it's the workspace's.
pub fn version(args: &[String]) -> Result<String, String> {
    let Some(tag) = flag(args, "--tag") else { return Ok(env!("CARGO_PKG_VERSION").to_owned()) };
    let version = tag_version(tag)?;
    let path = root().join("Cargo.toml");
    let manifest = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::write(&path, with_version(&manifest, version)?).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(version.to_owned())
}

/// `notes vX.Y.Z`: prints the release's notes.
pub fn notes(args: &[String]) -> Result<(), String> {
    let tag = args.first().ok_or("usage: cargo xtask notes vX.Y.Z")?;
    tag_version(tag)?;
    let notes = match tag_message(tag)? {
        Some(message) => message,
        None => changes_before(tag)?,
    };
    println!("{notes}");
    Ok(())
}

/// `v1.2.3` → `1.2.3`. Pre-releases aren't released: the updater doesn't offer them.
fn tag_version(tag: &str) -> Result<&str, String> {
    let version = tag.strip_prefix('v').unwrap_or_default();
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
        Ok(version)
    } else {
        Err(format!("{tag:?} isn't a release tag: tag a release vX.Y.Z, as in v0.1.6"))
    }
}

/// `manifest` with its `[workspace.package]` version set to `version`.
fn with_version(manifest: &str, version: &str) -> Result<String, String> {
    let mut out = String::with_capacity(manifest.len());
    let mut section = "";
    let mut set = false;
    for line in manifest.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = trimmed;
        }
        let key = trimmed.split('=').next().unwrap_or_default().trim();
        if section == "[workspace.package]" && key == "version" && !set {
            let end = if line.ends_with('\n') { "\n" } else { "" };
            for part in ["version = \"", version, "\"", end] {
                out.push_str(part);
            }
            set = true;
        } else {
            out.push_str(line);
        }
    }
    if set { Ok(out) } else { Err("Cargo.toml has no [workspace.package] version".to_owned()) }
}

/// The message of `tag`, when it's an annotated tag with one.
fn tag_message(tag: &str) -> Result<Option<String>, String> {
    let reference = format!("refs/tags/{tag}");
    let kind = stdout(git().args(["cat-file", "-t", &reference])).ok();
    if kind.as_deref() != Some("tag") {
        return Ok(None);
    }
    Ok(message(&stdout(git().args(["cat-file", "tag", &reference]))?))
}

/// The message of a tag object as `git cat-file tag` prints it: what follows its headers, without
/// the signature a signed tag ends with.
fn message(object: &str) -> Option<String> {
    let (_headers, message) = object.split_once("\n\n")?;
    let message = message.split("-----BEGIN ").next().unwrap_or_default().trim();
    (!message.is_empty()).then(|| message.to_owned())
}

/// The changes since the release before `tag`, up to `tag` (or, before it's made, up to HEAD).
fn changes_before(tag: &str) -> Result<String, String> {
    let commit = format!("refs/tags/{tag}^{{commit}}");
    let head = if stdout(git().args(["rev-parse", "--verify", "--quiet", &commit])).is_ok() {
        commit
    } else {
        "HEAD".to_owned()
    };
    let previous =
        stdout(git().args(["describe", "--tags", "--abbrev=0", "--match", "v[0-9]*", &format!("{head}^")])).ok();
    let range = previous.map_or_else(|| head.clone(), |p| format!("{p}..{head}"));
    let log = stdout(git().args(["log", "--first-parent", "--format=%s%x1f%b%x1e", &range]))?;
    Ok(changelog(&log))
}

/// `git log --first-parent --format=%s%x1f%b%x1e` → one bullet per change, oldest first: a
/// merged pull request by its title and number, any other commit by its subject.
fn changelog(log: &str) -> String {
    let mut changes: Vec<String> = log
        .split('\x1e')
        .filter_map(|record| {
            let (subject, body) = record.trim().split_once('\x1f')?;
            Some(format!("- {}", change(subject.trim(), body)))
        })
        .collect();
    if changes.is_empty() {
        return "No changes since the last release.".to_owned();
    }
    changes.reverse();
    changes.join("\n")
}

/// GitHub's merge commit says `Merge pull request #29 from owner/branch` and puts the pull
/// request's title in its body.
fn change(subject: &str, body: &str) -> String {
    let pull = subject.strip_prefix("Merge pull request #").and_then(|rest| rest.split_whitespace().next());
    let title = body.lines().map(str::trim).find(|l| !l.is_empty());
    match (pull, title) {
        (Some(number), Some(title)) => format!("{title} (#{number})"),
        _ => subject.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_tag_names_three_numbers() {
        assert_eq!(tag_version("v0.1.6"), Ok("0.1.6"));
        assert_eq!(tag_version("v10.20.300"), Ok("10.20.300"));
        for not in ["0.1.6", "v0.1", "v0.1.6-beta.1", "v0.1.x", "v0..6", "release"] {
            assert!(tag_version(not).is_err(), "{not}");
        }
    }

    #[test]
    fn only_the_workspace_version_is_set() {
        let manifest = "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \"0.1.5\"\nedition = \"2024\"\n\n\
                        [workspace.dependencies]\nrune = { git = \"x\", version = \"0.0.1\" }\n";
        let set = with_version(manifest, "0.2.0").unwrap();
        assert_eq!(set, manifest.replace("version = \"0.1.5\"", "version = \"0.2.0\""));
        assert!(with_version("[package]\nversion = \"1.0.0\"\n", "0.2.0").is_err());
    }

    #[test]
    fn a_tags_message_is_its_notes_without_its_signature() {
        let object = "object 0a329e9\ntype commit\ntag v0.1.6\ntagger Someone <s@example.com> 0 +0000\n\n\
                      Faster history.\n\n## Fixed\n\n- **Drag** the window.\n-----BEGIN SSH SIGNATURE-----\nabc\n-----END SSH SIGNATURE-----\n";
        assert_eq!(message(object).as_deref(), Some("Faster history.\n\n## Fixed\n\n- **Drag** the window."));
        assert_eq!(message("object 0a329e9\ntype commit\n\n\n"), None);
    }

    #[test]
    fn the_changes_are_the_pull_requests_merged_oldest_first() {
        let log = "Merge pull request #31 from CharlExMachina/feat/mac-updates\x1fUpdate the Mac app from inside it\n\x1e\n\
                   Fix a typo in the README\x1f\x1e\n\
                   Merge pull request #29 from CharlExMachina/fix/drag-the-window\x1f\nMove and zoom the window from its top bars\n\nMore.\x1e\n";
        assert_eq!(
            changelog(log),
            "- Move and zoom the window from its top bars (#29)\n- Fix a typo in the README\n- Update the Mac app from inside it (#31)"
        );
        assert_eq!(changelog(""), "No changes since the last release.");
    }
}
