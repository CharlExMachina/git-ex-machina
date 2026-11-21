//! The repositories a person keeps in git-ex-machina, and which are pinned.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::env::Env;
use crate::git;

/// Saved as `workspace.json` in [`Env::config_dir`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub repos: Vec<WorkspaceRepo>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRepo {
    pub path: PathBuf,
    #[serde(default)]
    pub pinned: bool,
}

/// Why the workspace file couldn't be read or written.
#[derive(Debug)]
pub enum WorkspaceError {
    Io { path: PathBuf, error: std::io::Error },
    Json { path: PathBuf, error: serde_json::Error },
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceError::Io { path, error } => write!(f, "{}: {error}", path.display()),
            WorkspaceError::Json { path, error } => write!(f, "{} isn't a valid workspace: {error}", path.display()),
        }
    }
}

impl std::error::Error for WorkspaceError {}

/// Why a folder couldn't be added to the workspace.
#[derive(Debug)]
pub enum AddError {
    /// Nothing is there.
    Missing(PathBuf),
    /// A file, not a folder.
    NotAFolder(PathBuf),
    /// A folder that isn't inside a git working copy (a plain folder, a bare repository).
    NotARepo(PathBuf),
    Workspace(WorkspaceError),
}

impl std::fmt::Display for AddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddError::Missing(path) => write!(f, "nothing is at {}", path.display()),
            AddError::NotAFolder(path) => write!(f, "{} is a file, not a folder", path.display()),
            AddError::NotARepo(path) => write!(f, "{} isn't a git repository", path.display()),
            AddError::Workspace(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for AddError {}

/// The top folder of the working copy `path` is in, as git names it: a folder inside a
/// repository means the repository.
pub fn working_copy(env: &Env, path: &Path) -> Result<PathBuf, AddError> {
    env.off_ui_thread(&format_args!("looking for a repository at {}", path.display()));
    if !path.exists() {
        return Err(AddError::Missing(path.to_path_buf()));
    }
    if !path.is_dir() {
        return Err(AddError::NotAFolder(path.to_path_buf()));
    }
    match git::run(env, path, &["rev-parse", "--show-toplevel"]) {
        Ok(top) if !top.trim().is_empty() => Ok(PathBuf::from(top.trim())),
        _ => Err(AddError::NotARepo(path.to_path_buf())),
    }
}

/// Saves the repository at `path` (or around it) in the workspace, unpinned, unless it's
/// already there. Returns its top folder.
pub fn add_repo(env: &Env, path: &Path) -> Result<PathBuf, AddError> {
    let top = working_copy(env, path)?;
    let mut ws = Workspace::load(env).map_err(AddError::Workspace)?;
    if !ws.repos.iter().any(|r| r.path == top) {
        ws.repos.push(WorkspaceRepo { path: top.clone(), pinned: false });
        ws.save(env).map_err(AddError::Workspace)?;
    }
    Ok(top)
}

impl Workspace {
    pub fn file(env: &Env) -> PathBuf {
        env.config_dir.join("workspace.json")
    }

    /// The saved workspace, or an empty one when none has been saved yet.
    pub fn load(env: &Env) -> Result<Workspace, WorkspaceError> {
        let path = Workspace::file(env);
        env.off_ui_thread(&format_args!("reading {}", path.display()));
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| WorkspaceError::Json { path, error }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Workspace::default()),
            Err(error) => Err(WorkspaceError::Io { path, error }),
        }
    }

    pub fn save(&self, env: &Env) -> Result<(), WorkspaceError> {
        let path = Workspace::file(env);
        env.off_ui_thread(&format_args!("writing {}", path.display()));
        let io = |error| WorkspaceError::Io { path: path.clone(), error };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let text =
            serde_json::to_string_pretty(self).map_err(|error| WorkspaceError::Json { path: path.clone(), error })?;
        std::fs::write(&path, text + "\n").map_err(io)
    }

    /// Adds a repository once; adding it again only updates its pin.
    pub fn add(&mut self, path: &Path, pinned: bool) {
        match self.repos.iter_mut().find(|r| r.path == path) {
            Some(r) => r.pinned = pinned,
            None => self.repos.push(WorkspaceRepo { path: path.to_path_buf(), pinned }),
        }
    }
}
