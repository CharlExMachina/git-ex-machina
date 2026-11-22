//! A repository's settings on this computer: what lives in its `.git/config` (how it fetches,
//! pulls and pushes, who commits and how commits are signed) and what the app keeps there for
//! itself, under `gxm.`.
//!
//! Each reads as git would use it, wherever it was set (this repository, or the person's own
//! `~/.gitconfig`), and each change is written to this repository alone.

use std::path::Path;

use crate::env::Env;
use crate::git::{self, GitError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalSettings {
    /// `fetch.prune`: a fetch forgets `origin/…` branches the remote no longer has.
    pub fetch_prune: bool,
    pub pull: PullMode,
    /// `push.autoSetupRemote`: a branch's first push sets up where it pushes.
    pub auto_setup_remote: bool,
    /// `push.followTags`: tags go with the commits they point at.
    pub follow_tags: bool,
    /// `gxm.deleteMergedBranches`: when a pull request merges from the app, its local branch goes
    /// too, if nothing on it is missing.
    pub delete_merged: bool,
    pub identity: Identity,
}

/// What `git pull` does with commits that aren't pushed yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullMode {
    /// Joins them with a merge commit.
    Merge,
    /// Moves them on top of what's new.
    Rebase,
    /// Refuses unless no merge is needed.
    FastForward,
}

impl PullMode {
    pub const ALL: [PullMode; 3] = [PullMode::Rebase, PullMode::Merge, PullMode::FastForward];

    pub fn word(self) -> &'static str {
        match self {
            PullMode::Merge => "Merge them",
            PullMode::Rebase => "Go on top",
            PullMode::FastForward => "Refuse",
        }
    }
}

/// Who commits, and how commits are signed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub email: String,
    /// Set in this repository, rather than taken from `~/.gitconfig`.
    pub own_name: bool,
    pub own_email: bool,
    pub signing: Signing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signing {
    Off,
    /// With this SSH key (a path, or the public key itself).
    Ssh(String),
    /// With this GPG key id.
    Gpg(String),
}

/// One change, written to this repository's config.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalChange {
    FetchPrune(bool),
    Pull(PullMode),
    AutoSetupRemote(bool),
    FollowTags(bool),
    DeleteMerged(bool),
    /// A name for this repository's commits; None goes back to `~/.gitconfig`'s.
    Name(Option<String>),
    Email(Option<String>),
    Signing(Signing),
}

/// The settings as git uses them in `repo`.
pub fn read(env: &Env, repo: &Path) -> Result<LocalSettings, GitError> {
    let get = |key: &str| git::run(env, repo, &["config", "--get", key]).ok().map(|v| v.trim().to_owned());
    let local = |key: &str| git::run(env, repo, &["config", "--local", "--get", key]).is_ok();
    let flag = |key: &str| get(key).is_some_and(|v| matches!(v.as_str(), "true" | "yes" | "on" | "1"));
    let pull = if flag("pull.rebase") {
        PullMode::Rebase
    } else if get("pull.ff").as_deref() == Some("only") {
        PullMode::FastForward
    } else {
        PullMode::Merge
    };
    let key = get("user.signingkey").unwrap_or_default();
    let signing = match (flag("commit.gpgsign"), get("gpg.format").as_deref()) {
        (false, _) => Signing::Off,
        (true, Some("ssh")) => Signing::Ssh(key),
        (true, _) => Signing::Gpg(key),
    };
    Ok(LocalSettings {
        fetch_prune: flag("fetch.prune"),
        pull,
        auto_setup_remote: flag("push.autoSetupRemote"),
        follow_tags: flag("push.followTags"),
        delete_merged: flag("gxm.deleteMergedBranches"),
        identity: Identity {
            name: get("user.name").unwrap_or_default(),
            email: get("user.email").unwrap_or_default(),
            own_name: local("user.name"),
            own_email: local("user.email"),
            signing,
        },
    })
}

/// Writes `change` to `repo`'s own config.
pub fn write(env: &Env, repo: &Path, change: &LocalChange) -> Result<(), GitError> {
    let set = |key: &str, value: &str| git::run(env, repo, &["config", "--local", key, value]).map(drop);
    // Unsetting what isn't set is fine: the setting is gone either way.
    let unset = |key: &str| {
        git::run(env, repo, &["config", "--local", "--unset-all", key]).ok();
        Ok(())
    };
    let flag = |key: &str, on: bool| set(key, if on { "true" } else { "false" });
    match change {
        LocalChange::FetchPrune(on) => flag("fetch.prune", *on),
        LocalChange::AutoSetupRemote(on) => flag("push.autoSetupRemote", *on),
        LocalChange::FollowTags(on) => flag("push.followTags", *on),
        LocalChange::DeleteMerged(on) => flag("gxm.deleteMergedBranches", *on),
        LocalChange::Pull(PullMode::Rebase) => {
            flag("pull.rebase", true)?;
            unset("pull.ff")
        }
        LocalChange::Pull(PullMode::Merge) => {
            flag("pull.rebase", false)?;
            unset("pull.ff")
        }
        LocalChange::Pull(PullMode::FastForward) => {
            flag("pull.rebase", false)?;
            set("pull.ff", "only")
        }
        LocalChange::Name(Some(v)) => set("user.name", v),
        LocalChange::Name(None) => unset("user.name"),
        LocalChange::Email(Some(v)) => set("user.email", v),
        LocalChange::Email(None) => unset("user.email"),
        LocalChange::Signing(Signing::Off) => flag("commit.gpgsign", false),
        LocalChange::Signing(Signing::Ssh(key)) => {
            set("gpg.format", "ssh")?;
            set("user.signingkey", key)?;
            flag("commit.gpgsign", true)
        }
        LocalChange::Signing(Signing::Gpg(key)) => {
            set("gpg.format", "openpgp")?;
            set("user.signingkey", key)?;
            flag("commit.gpgsign", true)
        }
    }
}

/// A remote, as this repository's config has it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteEntry {
    pub name: String,
    pub url: String,
    /// Pushing to it is turned off (its push URL is [`NO_PUSH`]).
    pub fetch_only: bool,
}

/// The push URL that turns pushing to a remote off, as people conventionally write it.
pub const NO_PUSH: &str = "no_push";

/// The repository's remotes, `origin` first.
pub fn remotes(env: &Env, repo: &Path) -> Result<Vec<RemoteEntry>, GitError> {
    let out =
        git::run(env, repo, &["config", "--local", "--get-regexp", r"^remote\..*\.(url|pushurl)$"]).unwrap_or_default();
    let mut list: Vec<RemoteEntry> = Vec::new();
    for line in out.lines() {
        let Some((key, value)) = line.split_once(' ') else { continue };
        let Some(rest) = key.strip_prefix("remote.") else { continue };
        let (name, field) = match rest.rsplit_once('.') {
            Some(parts) => parts,
            None => continue,
        };
        let i = match list.iter().position(|r| r.name == name) {
            Some(i) => i,
            None => {
                list.push(RemoteEntry { name: name.to_owned(), url: String::new(), fetch_only: false });
                list.len() - 1
            }
        };
        match field {
            "url" => list[i].url = value.to_owned(),
            _ => list[i].fetch_only = value == NO_PUSH,
        }
    }
    list.sort_by_key(|r| (r.name != "origin", r.name.clone()));
    Ok(list)
}

/// One change to the repository's remotes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteChange {
    Add { name: String, url: String },
    Remove(String),
    SetUrl { name: String, url: String },
    FetchOnly { name: String, on: bool },
}

pub fn change_remote(env: &Env, repo: &Path, change: &RemoteChange) -> Result<(), GitError> {
    match change {
        RemoteChange::Add { name, url } => git::run(env, repo, &["remote", "add", name, url]).map(drop),
        RemoteChange::Remove(name) => git::run(env, repo, &["remote", "remove", name]).map(drop),
        RemoteChange::SetUrl { name, url } => git::run(env, repo, &["remote", "set-url", name, url]).map(drop),
        RemoteChange::FetchOnly { name, on: true } => {
            git::run(env, repo, &["config", "--local", &format!("remote.{name}.pushurl"), NO_PUSH]).map(drop)
        }
        RemoteChange::FetchOnly { name, on: false } => {
            git::run(env, repo, &["config", "--local", "--unset-all", &format!("remote.{name}.pushurl")]).ok();
            Ok(())
        }
    }
}

/// Large files: the patterns `.gitattributes` hands to Git LFS, and whether `git-lfs` is here
/// to store them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lfs {
    pub installed: bool,
    pub patterns: Vec<String>,
}

const LFS_ATTRS: &str = "filter=lfs diff=lfs merge=lfs -text";

pub fn lfs(env: &Env, repo: &Path) -> Lfs {
    let attrs = std::fs::read_to_string(repo.join(".gitattributes")).unwrap_or_default();
    let patterns = attrs
        .lines()
        .filter(|l| l.split_whitespace().any(|a| a == "filter=lfs"))
        .filter_map(|l| l.split_whitespace().next().map(str::to_owned))
        .collect();
    Lfs { installed: git::run(env, repo, &["lfs", "version"]).is_ok(), patterns }
}

/// Hands files matching `pattern` to Git LFS, as `git lfs track` does: a line in
/// `.gitattributes`, which the person commits.
pub fn track(repo: &Path, pattern: &str) -> std::io::Result<()> {
    let path = repo.join(".gitattributes");
    let mut attrs = std::fs::read_to_string(&path).unwrap_or_default();
    if attrs.lines().any(|l| l.split_whitespace().next() == Some(pattern)) {
        return Ok(());
    }
    if !attrs.is_empty() && !attrs.ends_with('\n') {
        attrs.push('\n');
    }
    attrs.push_str(&format!("{pattern} {LFS_ATTRS}\n"));
    std::fs::write(path, attrs)
}

/// Stops handing `pattern` to Git LFS.
pub fn untrack(repo: &Path, pattern: &str) -> std::io::Result<()> {
    let path = repo.join(".gitattributes");
    let attrs = std::fs::read_to_string(&path).unwrap_or_default();
    let kept: String = attrs
        .lines()
        .filter(|l| !(l.split_whitespace().next() == Some(pattern) && l.contains("filter=lfs")))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(path, kept)
}
