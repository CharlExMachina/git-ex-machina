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
    /// Where tokens are kept.
    pub secrets: Secrets,
    /// Which version is running, and the file it runs from when it can replace itself.
    pub install: Install,
    /// Where the person's command-line tools are found (`PATH`): the agent harnesses they
    /// bring (`claude`, `codex`) are looked up here, never in the developer's own `PATH` when
    /// isolated.
    pub path: Vec<PathBuf>,
}

/// The running app as an installation: its version, and the bundle it was started from. Only an
/// AppImage or a Mac app updates itself (see `update`); anything else, a `cargo run` or a package
/// a package manager owns, leaves updating to whoever installed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Install {
    /// `0.1.0`.
    pub version: String,
    /// What the app runs from, which an update replaces.
    pub bundle: Option<Bundle>,
}

impl Install {
    /// This build's version, run from nothing it could replace.
    pub fn this_build() -> Install {
        Install { version: env!("CARGO_PKG_VERSION").to_owned(), bundle: None }
    }
}

/// What the app runs from, when an update can replace it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bundle {
    /// The AppImage file (the AppImage runtime's `APPIMAGE`).
    AppImage(PathBuf),
    /// The Mac app, `git-ex-machina.app`, wherever the person put it.
    MacApp(PathBuf),
}

impl Bundle {
    /// A path ending in `.app` is a Mac app; anything else is an AppImage.
    pub fn at(path: PathBuf) -> Bundle {
        if path.extension().is_some_and(|e| e == "app") { Bundle::MacApp(path) } else { Bundle::AppImage(path) }
    }

    pub fn path(&self) -> &Path {
        match self {
            Bundle::AppImage(p) | Bundle::MacApp(p) => p,
        }
    }

    /// The Mac app `exe` runs inside (`…/git-ex-machina.app/Contents/MacOS/git-ex-machina`), when
    /// an update could replace it. Not one opened from its disk image, nor one macOS moved aside
    /// to open it (App Translocation): both are read-only, and the person copies the app to
    /// Applications to keep it.
    pub fn mac_app_of(exe: &Path) -> Option<Bundle> {
        let app = exe.parent().filter(|d| d.ends_with("Contents/MacOS"))?.parent()?.parent()?;
        let read_only = app.starts_with("/Volumes") || app.components().any(|c| c.as_os_str() == "AppTranslocation");
        (app.extension().is_some_and(|e| e == "app") && !read_only).then(|| Bundle::MacApp(app.to_path_buf()))
    }
}

/// Where tokens are kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Secrets {
    /// The macOS Keychain, or the Secret Service on Linux.
    Keychain,
    /// A file only its owner can read: a sandbox's, so tests never touch the person's Keychain.
    File(PathBuf),
}

/// Where a provider's API is and the token that opens it. The token here is one the process
/// environment gave (`GH_TOKEN`); a sign-in's token, kept in the Keychain, is read off the UI
/// thread and put here when it lands (see `auth`).
#[derive(Clone, PartialEq, Eq)]
pub struct ForgeAccess {
    /// GitHub's API root: `https://api.github.com`, or a world's fake.
    pub github_api: String,
    /// Where GitHub's sign-in pages are: `https://github.com`, or a world's fake.
    pub github_web: String,
    pub github_token: Option<String>,
}

impl std::fmt::Debug for ForgeAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print a token.
        f.debug_struct("ForgeAccess")
            .field("github_api", &self.github_api)
            .field("github_web", &self.github_web)
            .field("github_token", &self.github_token.as_ref().map(|_| "…"))
            .finish()
    }
}

impl ForgeAccess {
    pub const GITHUB_API: &str = "https://api.github.com";
    pub const GITHUB_WEB: &str = "https://github.com";

    /// No provider reachable: no token, GitHub's real addresses.
    pub fn none() -> ForgeAccess {
        ForgeAccess {
            github_api: ForgeAccess::GITHUB_API.to_owned(),
            github_web: ForgeAccess::GITHUB_WEB.to_owned(),
            github_token: None,
        }
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
pub const GITHUB_WEB_VAR: &str = "GXM_GITHUB_WEB";
pub const VERSION_VAR: &str = "GXM_VERSION";
/// What a sandboxed run plays it runs from: an AppImage, or a Mac app (`….app`).
pub const BUNDLE_VAR: &str = "GXM_BUNDLE";
/// The tool search path a sandboxed run uses in place of `PATH`.
pub const PATH_VAR: &str = "GXM_PATH";

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
        let github_web = var(GITHUB_WEB_VAR).unwrap_or_else(|| ForgeAccess::GITHUB_WEB.to_owned());
        let forge = ForgeAccess { github_api, github_web, github_token };
        let secrets = if isolated { Secrets::File(config_dir.join("secrets.json")) } else { Secrets::Keychain };
        // A sandboxed run says which version it plays and what it would replace; a real one is
        // this build, from the AppImage the AppImage runtime names, or from the Mac app it's in.
        let install = if isolated {
            Install {
                version: var(VERSION_VAR).unwrap_or_else(|| Install::this_build().version),
                bundle: var(BUNDLE_VAR).map(|p| Bundle::at(PathBuf::from(p))),
            }
        } else {
            let appimage = var("APPIMAGE").map(|p| Bundle::AppImage(PathBuf::from(p)));
            let mac_app = || {
                let exe = std::env::current_exe().ok().filter(|_| cfg!(target_os = "macos"))?;
                Bundle::mac_app_of(&exe)
            };
            Install { bundle: appimage.or_else(mac_app), ..Install::this_build() }
        };
        // An isolated run finds only the tools its sandbox put on its path.
        let path = if isolated { var(PATH_VAR) } else { var("PATH") };
        let path = path.map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
        Env { home, config_dir, clock, tz, isolated, git_vars, forge, secrets, install, path, ui_thread: None }
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
            secrets: Secrets::File(home.join(".config/git-ex-machina/secrets.json")),
            install: Install::this_build(),
            path: vec![home.join(".local/bin")],
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

    /// A path as a person typed it: `~` is the home folder, and so is the start of a relative
    /// path, since a window has no working directory a person could know about.
    pub fn expand(&self, typed: &str) -> PathBuf {
        let typed = typed.trim();
        let rest = typed.strip_prefix('~').filter(|r| r.is_empty() || r.starts_with('/'));
        match rest {
            Some(rest) => self.home.join(rest.trim_start_matches('/')),
            None if Path::new(typed).is_absolute() => PathBuf::from(typed),
            None => self.home.join(typed),
        }
    }

    /// A path as a person reads it: inside the home folder, as `~/…`.
    pub fn tilde(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
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
        vars.push((GITHUB_WEB_VAR.into(), self.forge.github_web.clone().into()));
        if let Some(token) = &self.forge.github_token {
            vars.push((GITHUB_TOKEN_VAR.into(), token.clone().into()));
        }
        vars.push((VERSION_VAR.into(), self.install.version.clone().into()));
        if let Some(bundle) = &self.install.bundle {
            vars.push((BUNDLE_VAR.into(), bundle.path().into()));
        }
        if let Ok(path) = std::env::join_paths(&self.path) {
            vars.push((PATH_VAR.into(), path));
        }
        vars
    }

    /// The tool `name` (`claude`), as the first executable file of that name on [`Env::path`].
    pub fn which(&self, name: &str) -> Option<PathBuf> {
        self.off_ui_thread(&format_args!("looking for {name} on the path"));
        self.path.iter().map(|dir| dir.join(name)).find(|p| is_executable(p))
    }
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn default_config_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-ex-machina")
    } else {
        home.join(".config/git-ex-machina")
    }
}

/// Git with no system config, a global config inside the sandbox home, no prompts, stable
/// output (C locale, UTC), and no looking for a repository above the sandbox: a world built
/// inside a checkout must not find the checkout.
fn sandbox_git_vars(home: &Path) -> Vec<(OsString, OsString)> {
    vec![
        ("HOME".into(), home.into()),
        ("GIT_CEILING_DIRECTORIES".into(), home.parent().unwrap_or(home).into()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("GIT_CONFIG_GLOBAL".into(), home.join(".gitconfig").into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_SSH_COMMAND".into(), "ssh -F /dev/null -o BatchMode=yes".into()),
        ("LANG".into(), "C".into()),
        ("LC_ALL".into(), "C".into()),
        ("TZ".into(), "UTC".into()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_paths_start_at_home_and_read_back_with_a_tilde() {
        let env = Env::sandbox(Path::new("/home/ana"), Timestamp::UNIX_EPOCH);
        assert_eq!(env.expand("~"), Path::new("/home/ana"));
        assert_eq!(env.expand(" ~/code/site/ "), Path::new("/home/ana/code/site/"));
        assert_eq!(env.expand("code/site"), Path::new("/home/ana/code/site"), "relative to home");
        assert_eq!(env.expand("/srv/git/site"), Path::new("/srv/git/site"));
        assert_eq!(env.expand("~bob/x"), Path::new("/home/ana/~bob/x"), "another user's home isn't ours to guess");
        assert_eq!(env.tilde(Path::new("/home/ana/code/site")), "~/code/site");
        assert_eq!(env.tilde(Path::new("/home/ana")), "~");
        assert_eq!(env.tilde(Path::new("/home/anabel")), "/home/anabel");
        assert_eq!(env.tilde(Path::new("/srv/git")), "/srv/git");
    }

    #[test]
    fn a_mac_app_updates_itself_only_where_the_person_put_it() {
        let app = |exe: &str| Bundle::mac_app_of(Path::new(exe)).map(|b| b.path().to_path_buf());
        let exe = "/Applications/git-ex-machina.app/Contents/MacOS/git-ex-machina";
        assert_eq!(app(exe), Some(PathBuf::from("/Applications/git-ex-machina.app")));
        let renamed = "/Users/ana/Applications/gxm.app/Contents/MacOS/git-ex-machina";
        assert_eq!(app(renamed), Some(PathBuf::from("/Users/ana/Applications/gxm.app")), "renamed, in ~/Applications");
        let mounted = "/Volumes/git-ex-machina/git-ex-machina.app/Contents/MacOS/git-ex-machina";
        assert_eq!(app(mounted), None, "on its disk image");
        let moved = "/private/var/folders/x/T/AppTranslocation/0A1B/d/git-ex-machina.app/Contents/MacOS/git-ex-machina";
        assert_eq!(app(moved), None, "moved aside by Gatekeeper");
        assert_eq!(app("/Users/ana/code/gxm/target/debug/gxm-app"), None, "a cargo build");
        let mac = PathBuf::from("/a/git-ex-machina.app");
        assert_eq!(Bundle::at(mac.clone()), Bundle::MacApp(mac));
        let appimage = PathBuf::from("/a/git-ex-machina.AppImage");
        assert_eq!(Bundle::at(appimage.clone()), Bundle::AppImage(appimage));
    }
}
