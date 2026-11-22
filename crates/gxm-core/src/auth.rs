//! Signing in to GitHub with the OAuth device flow (the person approves a one-time code on
//! github.com, as `gh auth login` does), who is signed in, and signing out. The token is kept
//! in the Keychain ([`secrets`]); the account beside it, in `accounts.json`, so the window can
//! say who's signed in without asking the network.
//!
//! A token the process environment gives (`GH_TOKEN`, see [`crate::env::ForgeAccess`]) comes
//! first; a sign-in's token is used when there's none.

use std::fmt;
use std::time::Duration;

use serde::Deserialize;

use crate::env::Env;
use crate::forge::github::GitHub;
use crate::forge::model::User;
use crate::forge::{Forge, ForgeError, http};
use crate::secrets;

/// git-ex-machina's OAuth App. Public by design: the device flow needs no secret.
pub const CLIENT_ID: &str = "Ov23liYCFf3FCirGMPGF";
/// What the token may do: repositories (code, pull requests, reviews, checks; private ones too)
/// and reading which organisations the person is in.
pub const SCOPES: &str = "repo read:org";
/// [`SCOPES`] and deleting repositories, which GitHub keeps apart: asked for only when the
/// person deletes one.
pub const DELETE_SCOPES: &str = "repo read:org delete_repo";
/// The host the token is for, as the Keychain and `accounts.json` file it.
pub const HOST: &str = "github.com";

/// What GitHub hands out to start a sign-in.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    /// What the person types on github.com: `WDJB-MJHT`.
    pub user_code: String,
    /// Where they type it: `https://github.com/login/device`.
    pub verification_uri: String,
    /// Seconds until the code expires.
    pub expires_in: u64,
    /// Seconds to wait between asks.
    pub interval: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AuthError {
    /// The person declined on github.com.
    Declined,
    /// The code ran out before it was entered.
    Expired,
    Cancelled,
    /// GitHub no longer accepts the saved token (revoked, expired).
    BadToken,
    /// GitHub couldn't be reached, or said no.
    GitHub(ForgeError),
    Store(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::Declined => write!(f, "the sign-in was declined on GitHub"),
            AuthError::Expired => write!(f, "the code expired before it was entered"),
            AuthError::Cancelled => write!(f, "the sign-in was cancelled"),
            AuthError::BadToken => write!(f, "GitHub no longer accepts the saved sign-in"),
            AuthError::GitHub(e) => write!(f, "GitHub: {e}"),
            AuthError::Store(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for AuthError {}

/// Asks GitHub for a one-time code to sign in with.
pub fn start(env: &Env) -> Result<DeviceCode, AuthError> {
    start_with(env, SCOPES)
}

/// Asks GitHub for a one-time code for a token that may do what `scopes` grant.
pub fn start_with(env: &Env, scopes: &str) -> Result<DeviceCode, AuthError> {
    env.off_ui_thread(&"asking GitHub for a sign-in code");
    let url = format!("{}/login/device/code", env.forge.github_web);
    let answer = http::post_form(&url, &[("client_id", CLIENT_ID), ("scope", scopes)]).map_err(AuthError::GitHub)?;
    serde_json::from_value(answer.clone()).map_err(|_| refusal(&answer))
}

/// Waits for the person to enter `code` on github.com, asking every `interval` seconds as GitHub
/// asks, and returns the token. Stops when `cancelled` turns true.
pub fn wait(env: &Env, code: &DeviceCode, cancelled: &dyn Fn() -> bool) -> Result<String, AuthError> {
    env.off_ui_thread(&"waiting for the sign-in on GitHub");
    let url = format!("{}/login/oauth/access_token", env.forge.github_web);
    let mut interval = code.interval.max(1);
    let mut waited = 0;
    loop {
        // Sleep in short steps, so a cancel lands quickly.
        for _ in 0..interval * 10 {
            if cancelled() {
                return Err(AuthError::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        waited += interval;
        let answer = http::post_form(
            &url,
            &[
                ("client_id", CLIENT_ID),
                ("device_code", code.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ],
        )
        .map_err(AuthError::GitHub)?;
        if let Some(token) = answer["access_token"].as_str() {
            return Ok(token.to_owned());
        }
        match answer["error"].as_str() {
            Some("authorization_pending") if waited < code.expires_in => {}
            Some("slow_down") => interval += 5,
            Some("authorization_pending" | "expired_token") => return Err(AuthError::Expired),
            Some("access_denied") => return Err(AuthError::Declined),
            _ => return Err(refusal(&answer)),
        }
    }
}

/// GitHub's own words for a refusal: `error_description`, else `error`.
fn refusal(answer: &serde_json::Value) -> AuthError {
    let said = answer["error_description"]
        .as_str()
        .or_else(|| answer["error"].as_str())
        .unwrap_or("an answer we don't understand");
    AuthError::GitHub(ForgeError::Api(said.to_owned()))
}

/// Who `token` belongs to.
pub fn whoami(env: &Env, token: &str) -> Result<User, AuthError> {
    env.off_ui_thread(&"asking GitHub who's signed in");
    match GitHub::new(&env.forge.github_api, token.to_owned(), String::new(), String::new()).viewer() {
        Ok(user) => Ok(user),
        Err(ForgeError::Status(401)) => Err(AuthError::BadToken),
        Err(e) => Err(AuthError::GitHub(e)),
    }
}

/// Keeps a new sign-in: the token in the Keychain, the account in `accounts.json`.
pub fn remember(env: &Env, account: &User, token: &str) -> Result<(), AuthError> {
    secrets::save(env, HOST, token).map_err(|e| AuthError::Store(e.to_string()))?;
    write_account(env, Some(account))
}

/// Who's signed in, as last remembered; no network.
pub fn signed_in(env: &Env) -> Option<User> {
    env.off_ui_thread(&"reading accounts.json");
    let text = std::fs::read_to_string(env.config_dir.join("accounts.json")).ok()?;
    let mut all: std::collections::BTreeMap<String, User> = serde_json::from_str(&text).ok()?;
    all.remove(HOST)
}

/// The saved token, if there's a sign-in.
pub fn token(env: &Env) -> Result<Option<String>, AuthError> {
    secrets::load(env, HOST).map_err(|e| AuthError::Store(e.to_string()))
}

/// Who's signed in and their token, as last remembered; no network. None without a sign-in.
pub fn stored(env: &Env) -> Option<(User, String)> {
    let account = signed_in(env)?;
    token(env).ok().flatten().map(|token| (account, token))
}

/// Forgets the sign-in: the token and the account. GitHub keeps the authorisation until the
/// person revokes it in their settings, as with `gh`.
pub fn sign_out(env: &Env) -> Result<(), AuthError> {
    secrets::delete(env, HOST).map_err(|e| AuthError::Store(e.to_string()))?;
    write_account(env, None)
}

fn write_account(env: &Env, account: Option<&User>) -> Result<(), AuthError> {
    let path = env.config_dir.join("accounts.json");
    env.off_ui_thread(&format_args!("writing {}", path.display()));
    let store = |e: &dyn fmt::Display| AuthError::Store(format!("couldn't save {}: {e}", path.display()));
    let mut all: std::collections::BTreeMap<String, User> =
        std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    match account {
        Some(a) => all.insert(HOST.to_owned(), a.clone()),
        None => all.remove(HOST),
    };
    std::fs::create_dir_all(&env.config_dir).map_err(|e| store(&e))?;
    let text = serde_json::to_string_pretty(&all).map_err(|e| store(&e))?;
    std::fs::write(&path, text + "\n").map_err(|e| store(&e))
}
