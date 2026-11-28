//! Bring your own harness: git-ex-machina isn't an agent, it drives the one the person already
//! uses (Claude Code, Codex) as T3 Code does, through the harness's own command-line tool.
//!
//! - [`available`] finds each known harness on the path and says whether it's usable: its
//!   version, whether it's signed in, and why not when it isn't.
//! - [`preferred`] is the one to use: the person's choice ([`prefer`]), else the first usable.
//! - [`start`] runs a prompt in a folder (a worktree) as a [`Session`], whose [`Event`]s stream
//!   in, the same for every harness, until it ends or the person [stops](Session::stop) it.
//!
//! Each harness is a [`Driver`]; adding one is a driver and a line in [`DRIVERS`]. Everything
//! here starts processes or reads files, so it runs off the UI thread.

mod claude;
mod codex;
mod event;
pub(crate) mod probe;
mod run;
pub mod session;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use event::{Change, Ending, Event, FileEdit, Tool};
pub use run::{Session, Wake};

use crate::env::Env;

/// A harness git-ex-machina knows how to drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Harness {
    ClaudeCode,
    Codex,
}

impl Harness {
    pub const ALL: [Harness; 2] = [Harness::ClaudeCode, Harness::Codex];

    /// As a person reads it: `Claude Code`.
    pub fn name(self) -> &'static str {
        self.driver().name()
    }

    /// As settings and the CLI name it: `claude-code`, `codex`.
    pub fn id(self) -> &'static str {
        match self {
            Harness::ClaudeCode => "claude-code",
            Harness::Codex => "codex",
        }
    }

    pub fn from_id(id: &str) -> Option<Harness> {
        Harness::ALL.into_iter().find(|h| h.id() == id || h.driver().binary() == id)
    }

    fn driver(self) -> &'static dyn Driver {
        match self {
            Harness::ClaudeCode => &claude::ClaudeCode,
            Harness::Codex => &codex::Codex,
        }
    }
}

/// Every driver, in the order settings lists them.
pub const DRIVERS: [&dyn Driver; 2] = [&claude::ClaudeCode, &codex::Codex];

/// How much an agent may do without asking, as T3 Code's runtime modes have it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RuntimeMode {
    /// Reads and plans freely; anything that needs approval (a command, an edit outside what the
    /// harness allows by itself) is declined, since no one is there to approve it.
    #[default]
    ApprovalRequired,
    /// Edits and runs commands in the session's folder without asking.
    FullAccess,
}

/// A prompt to run in a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Start {
    /// Where the agent works: a worktree.
    pub dir: PathBuf,
    pub prompt: String,
    pub mode: RuntimeMode,
}

/// One harness, as found on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarnessStatus {
    pub harness: Harness,
    /// Where its command is, when it's on the path.
    pub path: Option<PathBuf>,
    /// Its version, as its `--version` prints it: `2.1.3`.
    pub version: Option<String>,
    pub auth: Auth,
    /// Why it can't be used, when it can't, with what to do about it.
    pub problem: Option<String>,
}

/// Whether a harness is signed in to its provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Auth {
    /// Signed in, as whom when the harness says (an email, "API key").
    SignedIn(Option<String>),
    SignedOut,
    /// It couldn't be asked (not installed, or it didn't answer).
    Unknown,
}

impl HarnessStatus {
    /// Installed and signed in: it can run a prompt.
    pub fn usable(&self) -> bool {
        self.path.is_some() && self.problem.is_none()
    }

    pub fn installed(&self) -> bool {
        self.path.is_some()
    }
}

/// What a driver knows about its harness.
pub trait Driver: Send + Sync {
    fn harness(&self) -> Harness;
    /// `Claude Code`.
    fn name(&self) -> &'static str;
    /// The command on the path: `claude`.
    fn binary(&self) -> &'static str;
    /// How to install it, for a person who hasn't.
    fn install_hint(&self) -> &'static str;
    /// Asks the harness at `program` for its version and whether it's signed in.
    fn probe(&self, env: &Env, program: &Path) -> HarnessStatus;
    /// The arguments that start a session for `start`, and the protocol that reads it.
    fn launch(&self, start: &Start) -> (Vec<String>, Box<dyn session::Protocol>);
}

/// Every known harness and whether it can be used here, in [`DRIVERS`] order.
pub fn available(env: &Env) -> Vec<HarnessStatus> {
    DRIVERS.iter().map(|d| status(env, *d)).collect()
}

/// The status of one harness.
pub fn status(env: &Env, d: &dyn Driver) -> HarnessStatus {
    match env.which(d.binary()) {
        Some(program) => d.probe(env, &program),
        None => HarnessStatus {
            harness: d.harness(),
            path: None,
            version: None,
            auth: Auth::Unknown,
            problem: Some(format!("{} (`{}`) was not found on PATH. {}", d.name(), d.binary(), d.install_hint())),
        },
    }
}

/// The harness to use: the one the person chose, while it's usable, else the first usable one.
pub fn preferred(env: &Env) -> Option<Harness> {
    pick(choice(env), &available(env))
}

/// [`preferred`] over statuses already in hand.
pub fn pick(chosen: Option<Harness>, statuses: &[HarnessStatus]) -> Option<Harness> {
    let usable = |h: Harness| statuses.iter().any(|s| s.harness == h && s.usable());
    chosen.filter(|h| usable(*h)).or_else(|| statuses.iter().find(|s| s.usable()).map(|s| s.harness))
}

/// What's saved in `agents.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    preferred: Option<Harness>,
}

fn saved_at(env: &Env) -> PathBuf {
    env.config_dir.join("agents.json")
}

/// The harness the person chose in settings, if they did.
pub fn choice(env: &Env) -> Option<Harness> {
    let path = saved_at(env);
    env.off_ui_thread(&format_args!("reading {}", path.display()));
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Saved>(&text).ok()?.preferred
}

/// Saves `harness` as the one to use, in the app's config.
pub fn prefer(env: &Env, harness: Harness) -> Result<(), String> {
    let path = saved_at(env);
    env.off_ui_thread(&format_args!("writing {}", path.display()));
    let say = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    std::fs::create_dir_all(&env.config_dir).map_err(|e| say(&e))?;
    let text = serde_json::to_string_pretty(&Saved { preferred: Some(harness) }).map_err(|e| say(&e))?;
    std::fs::write(&path, text + "\n").map_err(|e| say(&e))
}

/// Starts `harness` on `start`. Its events arrive on the returned [`Session`]; `wake` runs after
/// each one lands (a window's waker, or nothing).
pub fn start(env: &Env, harness: Harness, start: &Start, wake: Wake) -> Result<Session, String> {
    let d = harness.driver();
    let program = env
        .which(d.binary())
        .ok_or_else(|| format!("{} (`{}`) was not found on PATH. {}", d.name(), d.binary(), d.install_hint()))?;
    if !start.dir.is_dir() {
        return Err(format!("{} isn't a folder", start.dir.display()));
    }
    let (args, protocol) = d.launch(start);
    run::spawn(env, harness, start, run::Launch { program, args, protocol }, wake)
}
