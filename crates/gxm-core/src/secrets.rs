//! Tokens: in the macOS Keychain or the Linux Secret Service, or, in a sandbox, a file only
//! its owner can read (see [`Secrets`]).

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use crate::env::{Env, Secrets};

/// The Keychain service every git-ex-machina secret is filed under.
const SERVICE: &str = "git-ex-machina";

#[derive(Debug)]
pub struct SecretError(String);

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "couldn't reach the Keychain: {}", self.0)
    }
}

impl std::error::Error for SecretError {}

/// Keeps `secret` under `key` (`github.com`), replacing what was there.
pub fn save(env: &Env, key: &str, secret: &str) -> Result<(), SecretError> {
    env.off_ui_thread(&format_args!("saving the secret for {key}"));
    match &env.secrets {
        Secrets::Keychain => entry(key)?.set_password(secret).map_err(|e| SecretError(e.to_string())),
        Secrets::File(path) => {
            let mut all = read_file(path)?;
            all.insert(key.to_owned(), secret.to_owned());
            write_file(path, &all)
        }
    }
}

/// The secret under `key`, if there is one.
pub fn load(env: &Env, key: &str) -> Result<Option<String>, SecretError> {
    env.off_ui_thread(&format_args!("reading the secret for {key}"));
    match &env.secrets {
        Secrets::Keychain => match entry(key)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError(e.to_string())),
        },
        Secrets::File(path) => Ok(read_file(path)?.remove(key)),
    }
}

/// Forgets the secret under `key`. Forgetting one that isn't there is fine.
pub fn delete(env: &Env, key: &str) -> Result<(), SecretError> {
    env.off_ui_thread(&format_args!("deleting the secret for {key}"));
    match &env.secrets {
        Secrets::Keychain => match entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError(e.to_string())),
        },
        Secrets::File(path) => {
            let mut all = read_file(path)?;
            all.remove(key);
            write_file(path, &all)
        }
    }
}

fn entry(key: &str) -> Result<keyring::Entry, SecretError> {
    keyring::Entry::new(SERVICE, key).map_err(|e| SecretError(e.to_string()))
}

fn read_file(path: &Path) -> Result<BTreeMap<String, String>, SecretError> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| SecretError(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(SecretError(e.to_string())),
    }
}

fn write_file(path: &Path, all: &BTreeMap<String, String>) -> Result<(), SecretError> {
    let fail = |e: std::io::Error| SecretError(e.to_string());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(fail)?;
    }
    let text = serde_json::to_string_pretty(all).map_err(|e| SecretError(e.to_string()))?;
    std::fs::write(path, text).map_err(fail)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(fail)?;
    }
    Ok(())
}
