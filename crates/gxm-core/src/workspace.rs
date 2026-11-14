//! The repositories a person keeps in git-ex-machina, and which are pinned.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::env::Env;

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

impl Workspace {
    pub fn file(env: &Env) -> PathBuf {
        env.config_dir.join("workspace.json")
    }

    /// The saved workspace, or an empty one when none has been saved yet.
    pub fn load(env: &Env) -> Result<Workspace, WorkspaceError> {
        let path = Workspace::file(env);
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| WorkspaceError::Json { path, error }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Workspace::default()),
            Err(error) => Err(WorkspaceError::Io { path, error }),
        }
    }

    pub fn save(&self, env: &Env) -> Result<(), WorkspaceError> {
        let path = Workspace::file(env);
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
