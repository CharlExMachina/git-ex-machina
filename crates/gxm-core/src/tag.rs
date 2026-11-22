//! Tagging a commit, and pushing the tag: where the repository releases from tags, that push is
//! the release (`release.rs` says whether it does). What a new tag should be called comes from
//! the tags already there: the version after the newest.

use std::path::Path;

use crate::env::Env;
use crate::git::{self, GitError};
use crate::history::{RefKind, RefLabel};
use crate::release::{self, Trigger};

/// What tagging needs to know about a repository, read once with its history.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tagging {
    /// What pushing a tag starts, if the repository releases from tags.
    pub trigger: Option<Trigger>,
    /// Every tag, and the commit it's on.
    pub tags: Vec<(String, String)>,
    /// Where a pushed tag goes: `remote.pushDefault`, else `origin`, else the only remote.
    pub remote: Option<String>,
    /// The branch releases come from: `origin/HEAD`, else `main`, else `master`.
    pub main: String,
}

/// A tag to make: on `commit`, annotated with `message` (the name when it's empty), and pushed
/// when `push` says so, with the commit of its notes' files (`crate::attach`) when it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTag {
    pub name: String,
    pub commit: String,
    pub message: String,
    pub push: bool,
    pub files: Option<String>,
}

/// Reads what tagging needs: the release trigger, the tags, the remote and the main branch.
pub fn survey(env: &Env, repo: &Path) -> Tagging {
    env.off_ui_thread(&"reading the release workflows");
    let tags = git::run(
        env,
        repo,
        &["for-each-ref", "refs/tags", "--format=%(refname:strip=2)%1f%(objectname)%1f%(*objectname)"],
    )
    .unwrap_or_default();
    let tags = tags
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\x1f');
            let (name, object, peeled) = (f.next()?, f.next()?, f.next().unwrap_or_default());
            // An annotated tag's commit is the object it peels to.
            let commit = if peeled.is_empty() { object } else { peeled };
            Some((name.to_owned(), commit.to_owned()))
        })
        .collect();
    let remote = crate::ops::publish_remote(env, repo).ok();
    let main = crate::tidy::default_branch(env, repo);
    Tagging { trigger: release::trigger(repo), tags, remote, main }
}

impl Tagging {
    /// The commit a tag of this name is already on.
    pub fn existing(&self, name: &str) -> Option<&str> {
        self.tags.iter().find(|(n, _)| n == name).map(|(_, c)| c.as_str())
    }

    /// What to call the next tag: the version after the newest one (the newest one the release
    /// trigger runs for, where there is one). With no versions yet, a repository that releases
    /// from tags starts at 0.1.0; any other gets no suggestion.
    pub fn suggest(&self) -> Option<String> {
        let runs = |name: &str| self.trigger.as_ref().is_none_or(|t| t.runs_for(name));
        let newest =
            self.tags.iter().filter(|(n, _)| runs(n)).filter_map(|(n, _)| version(n)).max_by(|a, b| a.1.cmp(&b.1));
        if let Some((prefix, mut parts)) = newest {
            if let Some(last) = parts.last_mut() {
                *last += 1;
            }
            let parts: Vec<String> = parts.iter().map(u64::to_string).collect();
            return Some(format!("{prefix}{}", parts.join(".")));
        }
        let trigger = self.trigger.as_ref()?;
        let prefix = match trigger.patterns.first() {
            None => "v",
            Some(p) => {
                let literal = p.split(['*', '?', '[']).next().unwrap_or_default();
                if literal.ends_with(|c: char| c.is_ascii_digit()) || literal.len() == p.len() {
                    return None;
                }
                literal
            }
        };
        Some(format!("{prefix}0.1.0"))
    }

    /// Why `contained_in` (the refs that contain a commit) makes it an odd one to tag: it isn't
    /// on the main branch, or a push would be the first to send it.
    pub fn warnings(&self, contained_in: &[RefLabel], push: bool) -> Vec<String> {
        let mut said = Vec::new();
        let remote = self.remote.as_deref();
        let main = &self.main;
        let on_main = contained_in.iter().any(|r| match r.kind {
            RefKind::LocalBranch | RefKind::Head => r.name == *main,
            RefKind::RemoteBranch => remote.is_some_and(|rem| r.name == format!("{rem}/{main}")),
            RefKind::Tag => false,
        });
        if !on_main {
            said.push(format!("This commit isn't on {main}."));
        }
        if let (true, Some(remote)) = (push, remote) {
            let prefix = format!("{remote}/");
            let on_remote = contained_in.iter().any(|r| r.kind == RefKind::RemoteBranch && r.name.starts_with(&prefix));
            if !on_remote {
                said.push(format!("It isn't on {remote} yet: pushing the tag sends it too."));
            }
        }
        said
    }
}

/// A version tag split into its prefix and numbers: `v0.1.2` is `("v", [0, 1, 2])`. Pre-releases
/// (`v1.0.0-rc.1`) and names that aren't versions are `None`.
pub fn version(name: &str) -> Option<(&str, Vec<u64>)> {
    let at = name.find(|c: char| c.is_ascii_digit())?;
    let (prefix, rest) = name.split_at(at);
    let parts: Option<Vec<u64>> = rest.split('.').map(|p| p.parse().ok()).collect();
    Some((prefix, parts.filter(|p| (2..=4).contains(&p.len()))?))
}

/// The newest tag before `commit` that the release trigger runs for (any tag without one): what
/// GitHub's notes count from.
pub fn previous(env: &Env, repo: &Path, commit: &str, trigger: Option<&Trigger>) -> Option<String> {
    let mut args = vec!["describe".to_owned(), "--tags".to_owned(), "--abbrev=0".to_owned()];
    for p in trigger.map(|t| t.patterns.as_slice()).unwrap_or_default() {
        args.push(format!("--match={p}"));
    }
    // The commit's parent: a tag already on the commit is the one being made, not the last one.
    args.push(format!("{commit}^"));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    git::run(env, repo, &args).ok().map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

/// The full id of the commit `rev` names (`HEAD`, a branch, a short id).
pub fn resolve(env: &Env, repo: &Path, rev: &str) -> Result<String, GitError> {
    let out = git::run(env, repo, &["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")]);
    let missing = || GitError::Failed {
        args: "rev-parse".to_owned(),
        code: None,
        stderr: format!("fatal: no commit is called {rev}"),
    };
    Ok(out.map_err(|_| missing())?.trim().to_owned())
}

/// Makes the tag, and pushes it if asked: returns the remote it went to. A tag of that name on
/// that commit already is kept as it is, so a push that failed can be tried again.
pub fn create(env: &Env, repo: &Path, tag: &NewTag) -> Result<Option<String>, GitError> {
    let refuse = |why: String| GitError::Failed { args: "tag".to_owned(), code: None, stderr: format!("fatal: {why}") };
    let peeled = format!("refs/tags/{}^{{commit}}", tag.name);
    match git::run(env, repo, &["rev-parse", "--verify", "--quiet", &peeled]) {
        Ok(on) if on.trim() == tag.commit => {}
        Ok(on) => {
            let short = on.trim().get(..7).unwrap_or(on.trim()).to_owned();
            return Err(refuse(format!("{} already exists, on {short}", tag.name)));
        }
        Err(_) => {
            let message = if tag.message.trim().is_empty() { tag.name.as_str() } else { tag.message.trim() };
            // Verbatim: the message may be markdown, whose headings git would take for comments.
            let args = ["tag", "--annotate", "--cleanup=verbatim", "--message", message, &tag.name, &tag.commit];
            git::run(env, repo, &args)?;
        }
    }
    if !tag.push {
        return Ok(None);
    }
    let remote = crate::ops::publish_remote(env, repo)?;
    let mut refs = vec![format!("refs/tags/{}", tag.name)];
    if let Some(files) = &tag.files {
        // Kept here too, so the files stay in the repository the notes were written in.
        let files_ref = crate::attach::files_ref(&tag.name);
        git::run(env, repo, &["update-ref", &files_ref, files])?;
        refs.push(files_ref);
    }
    // One push: the tag never goes up without its files.
    let mut args = vec!["push", "--quiet", "--atomic", remote.as_str()];
    args.extend(refs.iter().map(String::as_str));
    git::run(env, repo, &args)?;
    Ok(Some(remote))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tagging(tags: &[&str], patterns: Option<&[&str]>) -> Tagging {
        Tagging {
            trigger: patterns.map(|p| Trigger {
                patterns: p.iter().map(|s| (*s).to_owned()).collect(),
                what: "the release workflow".to_owned(),
                file: ".github/workflows/release.yml".to_owned(),
                notes: release::NotesFrom::Unknown,
            }),
            tags: tags.iter().map(|t| ((*t).to_owned(), "c0ffee".to_owned())).collect(),
            remote: Some("origin".to_owned()),
            main: "main".to_owned(),
        }
    }

    #[test]
    fn the_next_tag_is_the_version_after_the_newest() {
        assert_eq!(tagging(&["v0.1.0", "v0.1.1", "v0.0.9"], Some(&["v*"])).suggest().as_deref(), Some("v0.1.2"));
        assert_eq!(tagging(&["v0.9.0", "v0.10.0"], None).suggest().as_deref(), Some("v0.10.1"), "by number, not text");
        assert_eq!(tagging(&["1.4", "1.3"], None).suggest().as_deref(), Some("1.5"));
        assert_eq!(tagging(&["v2.0.0-rc.1", "v1.9.0"], None).suggest().as_deref(), Some("v1.9.1"), "no pre-releases");
        let only_releases = tagging(&["v1.0.0", "docs-9.0.0"], Some(&["v*"]));
        assert_eq!(only_releases.suggest().as_deref(), Some("v1.0.1"), "only tags the trigger runs for");
    }

    #[test]
    fn a_first_release_starts_at_0_1_0_and_a_plain_repository_suggests_nothing() {
        assert_eq!(tagging(&[], Some(&["v*"])).suggest().as_deref(), Some("v0.1.0"));
        assert_eq!(tagging(&[], Some(&["release-*"])).suggest().as_deref(), Some("release-0.1.0"));
        assert_eq!(tagging(&[], Some(&[])).suggest().as_deref(), Some("v0.1.0"), "any tag releases");
        assert_eq!(tagging(&["draft"], None).suggest(), None);
    }

    #[test]
    fn a_commit_off_main_or_not_on_the_remote_is_warned_about() {
        let t = tagging(&[], None);
        let refs = |names: &[(RefKind, &str)]| -> Vec<RefLabel> {
            names.iter().map(|(kind, name)| RefLabel { kind: *kind, name: (*name).to_owned() }).collect()
        };
        assert!(
            t.warnings(&refs(&[(RefKind::LocalBranch, "main"), (RefKind::RemoteBranch, "origin/main")]), true)
                .is_empty()
        );
        assert_eq!(
            t.warnings(&refs(&[(RefKind::LocalBranch, "feature/x")]), true),
            ["This commit isn't on main.", "It isn't on origin yet: pushing the tag sends it too."]
        );
        assert_eq!(t.warnings(&refs(&[(RefKind::LocalBranch, "main")]), false), Vec::<String>::new(), "not pushing");
    }
}
