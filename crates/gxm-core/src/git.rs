//! Running the `git` executable.
//!
//! Network operations and writes go through the user's own `git` so their ssh config, credential
//! helpers and hooks behave exactly as on the command line. Reads go through it too for now; a
//! faster reader (gix) replaces them only when a perf budget asks for it.

use std::fmt;
use std::path::Path;
use std::process::Command;

use crate::env::{Clock, Env};

/// Variables from the developer's shell that would change what git does. Removed in isolated
/// environments.
const LEAKY_VARS: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_SSH",
    "GIT_ASKPASS",
    "SSH_ASKPASS",
    "SSH_AUTH_SOCK",
    "XDG_CONFIG_HOME",
    "GIT_AUTHOR_NAME",
    "GIT_AUTHOR_EMAIL",
    "GIT_AUTHOR_DATE",
    "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL",
    "GIT_COMMITTER_DATE",
];

/// A failed git invocation, with what it said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitError {
    /// `git` couldn't be started (not installed, not on `PATH`).
    Spawn { message: String },
    /// git ran and failed.
    Failed { args: String, code: Option<i32>, stderr: String },
    /// git's output wasn't what we asked for.
    Parse { what: &'static str, line: String },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::Spawn { message } => write!(f, "couldn't run git: {message}"),
            GitError::Failed { args, code, stderr } => {
                let code = code.map_or_else(|| "a signal".to_owned(), |c| c.to_string());
                write!(f, "`git {args}` exited with {code}: {}", stderr.trim())
            }
            GitError::Parse { what, line } => write!(f, "couldn't read {what} from git: {line:?}"),
        }
    }
}

impl std::error::Error for GitError {}

/// A command for `git -C repo args...` in `env`.
pub fn command(env: &Env, repo: &Path, args: &[&str]) -> Command {
    env.off_ui_thread(&format_args!("`git {}`", args.join(" ")));
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(args);
    if env.isolated {
        for var in LEAKY_VARS {
            cmd.env_remove(var);
        }
    }
    cmd.envs(env.git_vars.iter().map(|(k, v)| (k, v)));
    // On a fixed clock, what git records happens at that time too, so a commit made in a test
    // or a recording dates the same on every run.
    if let (true, Clock::Fixed(at)) = (env.isolated, env.clock) {
        let date = format!("@{} +0000", at.as_second());
        cmd.env("GIT_AUTHOR_DATE", &date).env("GIT_COMMITTER_DATE", &date);
    }
    cmd
}

/// Runs git and returns its stdout.
pub fn run(env: &Env, repo: &Path, args: &[&str]) -> Result<String, GitError> {
    let out = command(env, repo, args).output().map_err(|e| GitError::Spawn { message: e.to_string() })?;
    if !out.status.success() {
        return Err(GitError::Failed {
            args: args.join(" "),
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
