//! Everything the outside world gives the app: where home is, what time it is, and which
//! environment git runs in. Read once, at startup, and passed down.
//!
//! Tests and headless renders build an [`Env`] that points at a sandbox (see `gxm-world`), so
//! nothing ever reads or writes the real `~/.ssh`, `~/.gitconfig` or the wall clock.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use jiff::tz::TimeZone;

/// The process's view of the world.
#[derive(Clone, Debug)]
pub struct Env {
    /// The user's home directory (`~`).
    pub home: PathBuf,
    /// Where git-ex-machina keeps its own settings (the workspace list, pins).
    pub config_dir: PathBuf,
    /// Where "now" comes from, for relative dates: fixed in tests and headless renders, the
    /// wall clock otherwise. Read it with [`Env::now`].
    pub clock: Clock,
    /// The zone dates are shown in.
    pub tz: TimeZone,
    /// When true, git runs without the git and ssh variables of the shell that started us,
    /// plus [`Env::git_vars`], so the developer's own settings can't leak into a test.
    pub isolated: bool,
    /// Variables every git invocation gets.
    pub git_vars: Vec<(OsString, OsString)>,
    /// The thread that draws a window, set by [`Env::claim_ui_thread`]. Git and file reads on
    /// it would stall frames, so debug builds panic there (see [`Env::off_ui_thread`]).
    ui_thread: Option<std::thread::ThreadId>,
    /// How to reach the providers' APIs (pull requests, reviews, checks).
    pub forge: ForgeAccess,
}

/// Where a provider's API is and the token that opens it. Keychain storage and a sign-in
/// wizard come later; for now the token comes from the environment, read here and nowhere else.
#[derive(Clone, PartialEq, Eq)]
pub struct ForgeAccess {
    /// GitHub's API root: `https://api.github.com`, or a world's fake.
    pub github_api: String,
    pub github_token: Option<String>,
}

impl std::fmt::Debug for ForgeAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print a token.
        f.debug_struct("ForgeAccess")
            .field("github_api", &self.github_api)
            .field("github_token", &self.github_token.as_ref().map(|_| "…"))
            .finish()
    }
}

impl ForgeAccess {
    pub const GITHUB_API: &str = "https://api.github.com";

    /// No provider reachable: no token, GitHub's real address.
    pub fn none() -> ForgeAccess {
        ForgeAccess { github_api: ForgeAccess::GITHUB_API.to_owned(), github_token: None }
    }
}

/// The source of "now".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    /// Always this instant, so renders and goldens don't drift.
    Fixed(Timestamp),
    /// The wall clock, read each time, so a window left open keeps its dates current.
    System,
}

/// Overrides a headless render or a journey passes through the process environment.
pub const NOW_VAR: &str = "GXM_NOW";
pub const TZ_VAR: &str = "GXM_TZ";
pub const ISOLATED_VAR: &str = "GXM_ISOLATED";
pub const CONFIG_DIR_VAR: &str = "GXM_CONFIG_DIR";
pub const GITHUB_API_VAR: &str = "GXM_GITHUB_API";
pub const GITHUB_TOKEN_VAR: &str = "GXM_GITHUB_TOKEN";

impl Env {
    /// The real environment, with the `GXM_*` overrides a sandboxed run sets.
    #[expect(clippy::disallowed_methods, reason = "this is the one place the process environment is read")]
    pub fn from_process() -> Env {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let home = var("HOME").map(PathBuf::from).or_else(std::env::home_dir).unwrap_or_else(|| PathBuf::from("/"));
        let clock = var(NOW_VAR).and_then(|v| v.parse().ok()).map_or(Clock::System, Clock::Fixed);
        let tz = var(TZ_VAR).and_then(|v| TimeZone::get(&v).ok()).unwrap_or_else(TimeZone::system);
        let isolated = var(ISOLATED_VAR).is_some_and(|v| v == "1");
        let config_dir = var(CONFIG_DIR_VAR).map(PathBuf::from).unwrap_or_else(|| default_config_dir(&home));
        let git_vars = if isolated { sandbox_git_vars(&home) } else { Vec::new() };
        // An isolated run sees only the token its sandbox gives it; otherwise the token the
        // GitHub CLI and Actions use works too.
        let github_token = var(GITHUB_TOKEN_VAR)
            .or_else(|| if isolated { None } else { var("GH_TOKEN").or_else(|| var("GITHUB_TOKEN")) });
        let github_api = var(GITHUB_API_VAR).unwrap_or_else(|| ForgeAccess::GITHUB_API.to_owned());
        let forge = ForgeAccess { github_api, github_token };
        Env { home, config_dir, clock, tz, isolated, git_vars, forge, ui_thread: None }
    }

    /// A sandboxed environment rooted at `home`, at a fixed time, in UTC.
    pub fn sandbox(home: &Path, now: Timestamp) -> Env {
        Env {
            home: home.to_path_buf(),
            config_dir: home.join(".config/git-ex-machina"),
            clock: Clock::Fixed(now),
            tz: TimeZone::UTC,
            isolated: true,
            git_vars: sandbox_git_vars(home),
            forge: ForgeAccess::none(),
            ui_thread: None,
        }
    }

    /// Marks the calling thread as the one that draws the window. From then on, debug builds
    /// panic when this environment runs git or reads a file there.
    pub fn claim_ui_thread(&mut self) {
        self.ui_thread = Some(std::thread::current().id());
    }

    /// Fails loudly (in debug builds) when `what` would block the UI thread: slow work belongs
    /// on a background job, or the window drops frames on a big repository.
    pub fn off_ui_thread(&self, what: &dyn std::fmt::Display) {
        if cfg!(debug_assertions) && self.ui_thread == Some(std::thread::current().id()) {
            panic!("{what} on the UI thread; run it on a background job (see gxm-app's load.rs)");
        }
    }

    /// What time it is.
    #[expect(clippy::disallowed_methods, reason = "the one place the wall clock is read")]
    pub fn now(&self) -> Timestamp {
        match self.clock {
            Clock::Fixed(t) => t,
            Clock::System => Timestamp::now(),
        }
    }

    /// The variables a child process needs to see this same sandbox (for headless renders of
    /// the app binary and for the CLI). Empty for a real, non-isolated environment.
    pub fn child_vars(&self) -> Vec<(OsString, OsString)> {
        if !self.isolated {
            return Vec::new();
        }
        let mut vars = self.git_vars.clone();
        if let Clock::Fixed(t) = self.clock {
            vars.push((NOW_VAR.into(), t.to_string().into()));
        }
        vars.push((TZ_VAR.into(), self.tz.iana_name().unwrap_or("UTC").into()));
        vars.push((ISOLATED_VAR.into(), "1".into()));
        vars.push((CONFIG_DIR_VAR.into(), self.config_dir.clone().into()));
        vars.push((GITHUB_API_VAR.into(), self.forge.github_api.clone().into()));
        if let Some(token) = &self.forge.github_token {
            vars.push((GITHUB_TOKEN_VAR.into(), token.clone().into()));
        }
        vars
    }
}

fn default_config_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-ex-machina")
    } else {
        home.join(".config/git-ex-machina")
    }
}

/// Git with no system config, a global config inside the sandbox home, no prompts, and stable
/// output (C locale, UTC).
fn sandbox_git_vars(home: &Path) -> Vec<(OsString, OsString)> {
    vec![
        ("HOME".into(), home.into()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("GIT_CONFIG_GLOBAL".into(), home.join(".gitconfig").into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_SSH_COMMAND".into(), "ssh -F /dev/null -o BatchMode=yes".into()),
        ("LANG".into(), "C".into()),
        ("LC_ALL".into(), "C".into()),
        ("TZ".into(), "UTC".into()),
    ]
}
