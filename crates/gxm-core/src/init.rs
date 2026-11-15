//! Making a folder a repository where it is: `git init` in place, a README, an ignore list and
//! Git LFS if the person wants them, and nothing committed, so they look over what's there
//! first.

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
    /// The extensions of the binary files in it (`ogg`, `png`), sorted: what Git LFS would
    /// track. Read from the files themselves, so no list of formats needs keeping.
    pub binary: Vec<String>,
    /// `git lfs` is installed.
    pub lfs: bool,
}

/// Looks through the folder, without git.
pub fn survey(env: &Env, folder: &Path) -> Folder {
    env.off_ui_thread(&format_args!("looking in {}", folder.display()));
    Folder {
        kind: folder.join("project.godot").is_file().then_some(Kind::Godot),
        has_readme: folder.join("README.md").exists(),
        has_ignore: folder.join(".gitignore").exists(),
        binary: binary_extensions(folder),
        lfs: git::run(env, folder, &["lfs", "version"]).is_ok(),
    }
}

/// How many entries the binary scan looks at before it stops, so a huge folder answers quickly.
const SCAN_LIMIT: usize = 20_000;

/// Extensions of files that git would call binary (a NUL in the first 8000 bytes, git's own
/// test). Hidden folders (`.godot/`, `.git/`) hold generated files and are skipped.
fn binary_extensions(folder: &Path) -> Vec<String> {
    let mut found = std::collections::BTreeSet::new();
    let mut todo = vec![folder.to_path_buf()];
    let mut seen = 0;
    while let Some(dir) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            seen += 1;
            if seen > SCAN_LIMIT {
                return found.into_iter().collect();
            }
            let path = entry.path();
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            match entry.file_type() {
                Ok(t) if t.is_dir() && !hidden => todo.push(path),
                Ok(t) if t.is_file() => {
                    let Some(ext) = path.extension().map(|e| e.to_string_lossy().to_lowercase()) else { continue };
                    if !found.contains(&ext) && is_binary(&path) {
                        found.insert(ext);
                    }
                }
                _ => {}
            }
        }
    }
    found.into_iter().collect()
}

fn is_binary(path: &Path) -> bool {
    use std::io::Read as _;
    let mut head = [0u8; 8000];
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let n = file.read(&mut head).unwrap_or(0);
    head[..n].contains(&0)
}

/// What to write besides `.git`. A file that's already there is never overwritten.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub readme: Option<String>,
    pub ignore: Option<Kind>,
    /// Extensions Git LFS tracks (`png`), set up for this repository only.
    pub lfs: Vec<String>,
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
    if !plan.lfs.is_empty() {
        // `--local`: the hooks and filter go in this repository, never the person's global config.
        git::run(env, folder, &["lfs", "install", "--local"]).map_err(InitError::Git)?;
        let patterns: Vec<String> = plan.lfs.iter().map(|ext| format!("*.{ext}")).collect();
        let mut args = vec!["lfs", "track"];
        args.extend(patterns.iter().map(String::as_str));
        git::run(env, folder, &args).map_err(InitError::Git)?;
    }
    Ok(())
}
