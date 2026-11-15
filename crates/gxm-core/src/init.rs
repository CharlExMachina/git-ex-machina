//! Making a folder a repository where it is: `git init` in place, a README and an ignore list
//! if the person wants them, and nothing committed, so they look over what's there first.

use std::fmt;
use std::path::Path;

use crate::env::Env;
use crate::git::{self, GitError};

/// A kind of project whose generated files a `.gitignore` keeps out, recognised by a file it
/// always has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Godot,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Godot => "Godot",
        }
    }

    /// The patterns, as the project's own tools write them (Godot 4's project manager).
    pub fn ignore(self) -> &'static str {
        match self {
            Kind::Godot => "# Godot 4+ specific ignores\n.godot/\n/android/\n",
        }
    }

    /// What it keeps out, short enough for a checkbox.
    pub fn ignores(self) -> &'static str {
        match self {
            Kind::Godot => ".godot/ and /android/",
        }
    }
}

/// What a folder already holds that a new repository builds on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    pub kind: Option<Kind>,
    pub has_readme: bool,
    pub has_ignore: bool,
}

/// Looks at the folder's top level, without git.
pub fn survey(env: &Env, folder: &Path) -> Folder {
    env.off_ui_thread(&format_args!("looking in {}", folder.display()));
    let kind = folder.join("project.godot").is_file().then_some(Kind::Godot);
    Folder { kind, has_readme: folder.join("README.md").exists(), has_ignore: folder.join(".gitignore").exists() }
}

/// What to write besides `.git`. A file that's already there is never overwritten.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub readme: Option<String>,
    pub ignore: Option<Kind>,
}

/// The README a repository named `name` starts with.
pub fn readme(name: &str) -> String {
    format!("# {}\n", name.trim())
}

#[derive(Debug)]
pub enum InitError {
    Io { what: String, error: std::io::Error },
    Git(GitError),
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InitError::Io { what, error } => write!(f, "couldn't {what}: {error}"),
            InitError::Git(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for InitError {}

/// Starts a repository in `folder` and writes what `plan` asks for. The branch is the person's
/// `init.defaultBranch`, else `main`. Nothing is staged or committed.
pub fn create(env: &Env, folder: &Path, plan: &Plan) -> Result<(), InitError> {
    let configured = git::run(env, folder, &["config", "--get", "init.defaultBranch"]).unwrap_or_default();
    let mut args = vec!["init", "--quiet"];
    if configured.trim().is_empty() {
        args.push("--initial-branch=main");
    }
    git::run(env, folder, &args).map_err(InitError::Git)?;
    let write = |file: &str, text: &str| {
        let path = folder.join(file);
        if path.exists() {
            return Ok(());
        }
        std::fs::write(&path, text).map_err(|error| InitError::Io { what: format!("write {file}"), error })
    };
    if let Some(text) = &plan.readme {
        write("README.md", text)?;
    }
    if let Some(kind) = plan.ignore {
        write(".gitignore", kind.ignore())?;
    }
    Ok(())
}
