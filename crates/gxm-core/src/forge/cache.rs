//! The last answer the provider gave, kept on disk, so a pull request opens at once and is
//! refreshed behind the scenes. A cache miss or an unreadable file is just a miss.

use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::RepoKey;
use crate::env::Env;

/// `<config>/cache/<host>/<owner>/<repo>/<name>.json`.
pub fn path(env: &Env, repo: &RepoKey, name: &str) -> PathBuf {
    env.config_dir.join("cache").join(&repo.host).join(&repo.owner).join(&repo.name).join(format!("{name}.json"))
}

pub fn load<T: DeserializeOwned>(env: &Env, repo: &RepoKey, name: &str) -> Option<T> {
    let text = std::fs::read_to_string(path(env, repo, name)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Writes the answer. A cache that can't be written only costs speed, so failures are ignored.
pub fn save<T: Serialize>(env: &Env, repo: &RepoKey, name: &str, value: &T) {
    let path = path(env, repo, name);
    let Ok(text) = serde_json::to_string(value) else { return };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(path, text).ok();
}
