//! Files attached to release notes, pushed with the tag. They go up as a commit of their own
//! (no parent, just the files) under `refs/release-files/<tag>`, in the same push as the tag,
//! and the notes link each one by that commit: `github.com/owner/name/raw/<commit>/shot.png`.
//! GitHub keeps such a link as written (it routes only other sites' images through its image
//! proxy), so the viewer's browser loads it signed in as them: anyone who can see the repository,
//! private or public, sees the image. Nothing waits for the release workflow.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::env::Env;
use crate::git::{self, GitError};
use crate::remote::{Provider, Remote};

/// The ref a tag's files go up under.
pub fn files_ref(tag: &str) -> String {
    format!("refs/release-files/{tag}")
}

/// Where the files of `commit` are on GitHub: what the notes link them under.
pub fn files_base(remote: &Remote, commit: &str) -> Option<String> {
    (remote.provider == Provider::GitHub)
        .then(|| format!("https://{}/{}/{}/raw/{commit}", remote.host, remote.owner, remote.name))
}

/// Commits `files` (each one's name in the notes, and where it is) as a tree of their own. The
/// commit has no parent, a fixed author and a fixed date, so the same files make the same commit:
/// the notes can show its links before the tag is made.
pub fn files_commit(env: &Env, repo: &Path, files: &[(String, PathBuf)]) -> Result<String, GitError> {
    let mut tree = String::new();
    for (name, path) in files {
        let path = path.to_string_lossy();
        let blob = git::run(env, repo, &["hash-object", "-w", "--", &path])?;
        tree.push_str(&format!("100644 blob {}\t{name}\n", blob.trim()));
    }
    let tree = with_input(env, repo, &["mktree"], &tree)?;
    let mut commit = git::command(env, repo, &["commit-tree", tree.trim(), "-m", "Files in the release notes"]);
    let date = "@946684800 +0000";
    for (k, v) in [
        ("GIT_AUTHOR_NAME", "git-ex-machina"),
        ("GIT_AUTHOR_EMAIL", "files@git-ex-machina.invalid"),
        ("GIT_AUTHOR_DATE", date),
        ("GIT_COMMITTER_NAME", "git-ex-machina"),
        ("GIT_COMMITTER_EMAIL", "files@git-ex-machina.invalid"),
        ("GIT_COMMITTER_DATE", date),
    ] {
        commit.env(k, v);
    }
    let out = commit.output().map_err(|e| GitError::Spawn { message: e.to_string() })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        return Err(GitError::Failed { args: "commit-tree".to_owned(), code: out.status.code(), stderr });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Runs git with `input` on its stdin, and returns its stdout.
fn with_input(env: &Env, repo: &Path, args: &[&str], input: &str) -> Result<String, GitError> {
    let spawn = |e: std::io::Error| GitError::Spawn { message: e.to_string() };
    let mut child = git::command(env, repo, args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(spawn)?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).map_err(spawn)?;
    }
    let out = child.wait_with_output().map_err(spawn)?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        return Err(GitError::Failed { args: args.join(" "), code: out.status.code(), stderr });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_linked_by_their_commit_on_github() {
        let remote = Remote::parse("git@github.com:studio/beacon.git").unwrap();
        assert_eq!(files_base(&remote, "abc123").as_deref(), Some("https://github.com/studio/beacon/raw/abc123"));
        assert_eq!(files_base(&Remote::parse("git@gitlab.com:studio/beacon.git").unwrap(), "abc123"), None);
        assert_eq!(files_ref("v0.1.2"), "refs/release-files/v0.1.2");
    }

    #[test]
    fn the_same_files_make_the_same_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let env = Env::sandbox(&root.join("home"), "2026-11-11T15:00:00Z".parse().unwrap());
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git::run(&env, &repo, &["init", "--quiet"]).unwrap();
        std::fs::write(root.join("shot.png"), b"png").unwrap();
        let files = vec![("shot.png".to_owned(), root.join("shot.png"))];
        let first = files_commit(&env, &repo, &files).unwrap();
        assert_eq!(files_commit(&env, &repo, &files).unwrap(), first, "the links can be shown before the tag");
        let listed = git::run(&env, &repo, &["ls-tree", "--name-only", &first]).unwrap();
        assert_eq!(listed.trim(), "shot.png");
        assert_eq!(git::run(&env, &repo, &["show", &format!("{first}:shot.png")]).unwrap(), "png");
    }
}
