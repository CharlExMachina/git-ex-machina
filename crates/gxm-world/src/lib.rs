//! Hermetic worlds for journeys, CLI tests and headless renders.
//!
//! A [`World`] is a folder holding a sandboxed home (its own `.gitconfig`, `.ssh`, app config
//! and `~/code`, where the working copies live), local bare repositories standing in for GitHub, GitLab and Bitbucket, and working
//! copies built commit by commit with fixed authors and dates. The same world always produces
//! the same commit ids, so goldens stay stable.
//!
//! Remotes keep realistic URLs (`git@github.com:studio/atlas-web.git`). The world's
//! `.gitconfig` rewrites them with `insteadOf` to the local bare repositories, so fetch, pull
//! and push are real git operations that never touch the network.

use std::path::{Path, PathBuf};

use gxm_core::env::{Bundle, Env, ForgeAccess, Install};
use gxm_core::git::{self, GitError};
use gxm_core::workspace::Workspace;
use jiff::{SignedDuration, Timestamp};

pub mod fixtures;
pub mod hub;

/// "Now" in every world unless a test picks another time: Wednesday, Nov 11 2026, 15:00 UTC.
pub const NOW: &str = "2026-11-11T15:00:00Z";

/// The version of git-ex-machina every world plays, so an update a world publishes is newer
/// whatever the build's own version.
pub const VERSION: &str = "0.1.0";

/// What the world's app is installed as, in `~/Applications`: the Mac app on macOS, an AppImage
/// elsewhere.
pub const INSTALLED: &str = if cfg!(target_os = "macos") { "git-ex-machina.app" } else { "git-ex-machina.AppImage" };

/// Hosts whose SSH URLs the world serves locally.
pub const HOSTS: &[&str] = &["github.com", "gitlab.com", "bitbucket.org"];

/// A sandbox. Deleted on drop unless it was made with [`World::create_in`].
pub struct World {
    root: PathBuf,
    env: Env,
    hub: hub::Hub,
    _tmp: Option<tempfile::TempDir>,
}

/// A commit's author. Fixtures use the same few people so avatars and colours repeat.
#[derive(Clone, Copy, Debug)]
pub struct Author {
    pub name: &'static str,
    pub email: &'static str,
}

impl World {
    /// A fresh world in a temporary folder.
    pub fn new() -> World {
        let tmp =
            tempfile::Builder::new().prefix("gxm-world-").tempdir().unwrap_or_else(|e| fail("make a temp dir", e));
        // Canonical, so paths match what git reports (`/var` is `/private/var` on macOS).
        let root = tmp.path().canonicalize().unwrap_or_else(|e| fail("resolve the temp dir", e));
        let mut w = World::empty(root);
        w._tmp = Some(tmp);
        w
    }

    /// A world in `dir`, replacing whatever was there, kept after the process ends (for
    /// headless renders of the app binary).
    pub fn create_in(dir: &Path) -> World {
        if dir.exists() {
            std::fs::remove_dir_all(dir).unwrap_or_else(|e| fail("clear the world folder", e));
        }
        std::fs::create_dir_all(dir).unwrap_or_else(|e| fail("make the world folder", e));
        World::empty(dir.canonicalize().unwrap_or_else(|e| fail("resolve the world folder", e)))
    }

    fn empty(root: PathBuf) -> World {
        let home = root.join("home");
        for dir in [home.join(".ssh"), home.join("code"), root.join("remotes")] {
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| fail("make the world's folders", e));
        }
        let now: Timestamp = NOW.parse().unwrap_or_else(|e| fail("read NOW", e));
        // Every world has its own GitHub, empty until a fixture adds pull requests to it. Ana is
        // signed in to it as a sign-in leaves things: the token kept (in the sandbox, not a
        // Keychain) and her account beside it. The environment itself carries no token.
        let mut env = Env::sandbox(&home, now);
        let hub = hub::Hub::start(env.clone(), root.join("remotes"));
        env.forge = ForgeAccess { github_api: hub.url(), github_web: hub.url(), github_token: None };
        // Installed in `~/Applications` as this system's people install it (an AppImage, or the
        // Mac app), so it updates itself from the world's releases (there are none until a fixture
        // publishes one). The bundle itself is there only once a fixture installs it.
        let installed = home.join("Applications").join(INSTALLED);
        env.install = Install { version: VERSION.to_owned(), bundle: Some(Bundle::at(installed)) };
        let ana = gxm_core::forge::model::User { login: hub::ANA.login.to_owned(), name: hub::ANA.name.to_owned() };
        gxm_core::auth::remember(&env, &ana, hub::TOKEN).unwrap_or_else(|e| fail("sign Ana in", e));
        let mut cfg = String::from(
            // The person at the keyboard is Ana, who owns the demo workspace: what the app
            // commits in a world is hers.
            "[user]\n\tname = Ana Ruiz\n\temail = ana@example.com\n[init]\n\tdefaultBranch = main\n\
             [commit]\n\tgpgsign = false\n[tag]\n\tgpgsign = false\n[advice]\n\tdetachedHead = false\n\
             [protocol \"file\"]\n\tallow = always\n",
        );
        for host in HOSTS {
            let base = root.join("remotes").join(host);
            // HTTPS too, for someone who switches a remote to it.
            cfg.push_str(&format!(
                "[url \"file://{}/\"]\n\tinsteadOf = git@{host}:\n\tinsteadOf = https://{host}/\n",
                base.display()
            ));
        }
        std::fs::write(home.join(".gitconfig"), cfg).unwrap_or_else(|e| fail("write the world's .gitconfig", e));
        World { root, env, hub, _tmp: None }
    }

    pub fn env(&self) -> &Env {
        &self.env
    }

    /// Forgets Ana's sign-in, so the world starts signed out of GitHub.
    pub fn sign_out(&self) {
        gxm_core::auth::sign_out(&self.env).unwrap_or_else(|e| fail("sign Ana out", e));
    }

    /// What the world's app runs from, which an update replaces: `~/Applications/`[`INSTALLED`].
    pub fn installed(&self) -> PathBuf {
        self.env.home.join("Applications").join(INSTALLED)
    }

    /// The world's GitHub (see [`hub`]).
    pub fn hub(&self) -> &hub::Hub {
        &self.hub
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `~/code` in the world's home, where its working copies live.
    pub fn code(&self) -> PathBuf {
        self.env.home.join("code")
    }

    /// `now` minus `d`.
    pub fn ago(&self, d: SignedDuration) -> Timestamp {
        self.env.now().checked_sub(d).unwrap_or_else(|e| fail("subtract from now", e))
    }

    /// A new working copy at `~/code/<name>` whose `origin` is `git@<host>:<owner>/<name>.git`,
    /// backed by an empty local bare repository.
    pub fn repo(&self, host: &str, owner: &str, name: &str) -> RepoBuilder<'_> {
        let bare = self.remote(host, owner, name);
        std::fs::create_dir_all(&bare).unwrap_or_else(|e| fail("make a remote", e));
        self.git(&bare, &["init", "--bare", "--quiet"]);
        let path = self.code().join(name);
        std::fs::create_dir_all(&path).unwrap_or_else(|e| fail("make a working copy", e));
        self.git(&path, &["init", "--quiet"]);
        self.git(&path, &["remote", "add", "origin", &format!("git@{host}:{owner}/{name}.git")]);
        RepoBuilder { world: self, path }
    }

    /// A builder for a working copy the world already made, to keep changing it.
    pub fn open(&self, path: &Path) -> RepoBuilder<'_> {
        RepoBuilder { world: self, path: path.to_path_buf() }
    }

    /// The commit `branch` points at in the local remote standing in for `host`.
    pub fn remote_head(&self, host: &str, owner: &str, name: &str, branch: &str) -> String {
        self.git(&self.remote(host, owner, name), &["rev-parse", branch]).trim().to_owned()
    }

    /// The local bare repository standing in for `git@<host>:<owner>/<name>.git`.
    pub fn remote(&self, host: &str, owner: &str, name: &str) -> PathBuf {
        self.root.join("remotes").join(host).join(owner).join(format!("{name}.git"))
    }

    /// Lists a repository in the app's workspace.
    pub fn add_to_workspace(&self, repo: &Path, pinned: bool) {
        let mut ws = Workspace::load(&self.env).unwrap_or_else(|e| fail("load the workspace", e));
        ws.add(repo, pinned);
        ws.save(&self.env).unwrap_or_else(|e| fail("save the workspace", e));
    }

    /// Runs git in the world, failing the test with git's message if it fails.
    pub fn git(&self, dir: &Path, args: &[&str]) -> String {
        git::run(&self.env, dir, args).unwrap_or_else(|e| fail(&format!("run git {}", args.join(" ")), e))
    }

    fn git_as(&self, dir: &Path, who: Author, at: Timestamp, args: &[&str]) -> Result<String, GitError> {
        self.git_by(dir, who, who, at, args)
    }

    /// Git run with `author` and `committer` as the commit's two people, both at `at`.
    fn git_by(
        &self,
        dir: &Path,
        author: Author,
        committer: Author,
        at: Timestamp,
        args: &[&str],
    ) -> Result<String, GitError> {
        let date = format!("@{} +0000", at.as_second());
        let out = git::command(&self.env, dir, args)
            .env("GIT_AUTHOR_NAME", author.name)
            .env("GIT_AUTHOR_EMAIL", author.email)
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_NAME", committer.name)
            .env("GIT_COMMITTER_EMAIL", committer.email)
            .env("GIT_COMMITTER_DATE", &date)
            .output()
            .map_err(|e| GitError::Spawn { message: e.to_string() })?;
        if !out.status.success() {
            return Err(GitError::Failed {
                args: args.join(" "),
                code: out.status.code(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

impl Default for World {
    fn default() -> Self {
        World::new()
    }
}

/// Builds a working copy's history, commit by commit.
pub struct RepoBuilder<'w> {
    world: &'w World,
    path: PathBuf,
}

impl RepoBuilder<'_> {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends a line naming the commit to each file in `touch` (creating it), stages
    /// everything and commits. Returns the new commit's id.
    pub fn commit(&self, who: Author, at: Timestamp, message: &str, touch: &[&str]) -> String {
        for file in touch {
            self.append(file, &format!("{message}\n"));
        }
        self.git(&["add", "-A"]);
        self.git_as(who, at, &["commit", "--quiet", "--allow-empty", "-m", message]);
        self.head()
    }

    /// [`Self::commit`] written by `author` and committed by someone else, as when a
    /// maintainer applies a contributor's patch.
    pub fn commit_by(&self, author: Author, committer: Author, at: Timestamp, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.world
            .git_by(&self.path, author, committer, at, &["commit", "--quiet", "--allow-empty", "-m", message])
            .unwrap_or_else(|e| fail(&format!("commit as {} for {}", committer.name, author.name), e));
        self.head()
    }

    /// Merges `branch` into the checked-out branch with a merge commit.
    pub fn merge(&self, who: Author, at: Timestamp, branch: &str, message: &str) -> String {
        self.git_as(who, at, &["merge", "--quiet", "--no-ff", "-m", message, branch]);
        self.head()
    }

    /// Creates `branch` at HEAD and checks it out.
    pub fn branch(&self, branch: &str) {
        self.git(&["checkout", "--quiet", "-b", branch]);
    }

    pub fn checkout(&self, rev: &str) {
        self.git(&["checkout", "--quiet", rev]);
    }

    /// A lightweight tag at HEAD.
    pub fn tag(&self, name: &str) {
        self.git(&["tag", name]);
    }

    /// Pushes `branch` to `origin` and makes it the upstream.
    pub fn push(&self, branch: &str) {
        self.git(&["push", "--quiet", "-u", "origin", branch]);
    }

    /// Moves the checked-out branch back to `rev`, so it falls behind its upstream.
    pub fn reset_hard(&self, rev: &str) {
        self.git(&["reset", "--quiet", "--hard", rev]);
    }

    /// Writes a file without committing it.
    pub fn write(&self, file: &str, contents: &str) {
        let p = self.path.join(file);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).unwrap_or_else(|e| fail("make a folder", e));
        }
        std::fs::write(p, contents).unwrap_or_else(|e| fail("write a file", e));
    }

    /// Appends to a file without committing it.
    pub fn append(&self, file: &str, text: &str) {
        let p = self.path.join(file);
        let old = std::fs::read_to_string(&p).unwrap_or_default();
        self.write(file, &(old + text));
    }

    /// Feeds a `git fast-import` stream to the repository, then checks out what it made: a
    /// long history in milliseconds, where a commit at a time would take seconds.
    pub fn fast_import(&self, stream: &str, checkout: &str) {
        use std::io::Write as _;
        let mut child = git::command(&self.world.env, &self.path, &["fast-import", "--quiet"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap_or_else(|e| fail("start git fast-import", e));
        child
            .stdin
            .take()
            .unwrap_or_else(|| fail("open fast-import's input", "no stdin"))
            .write_all(stream.as_bytes())
            .unwrap_or_else(|e| fail("feed git fast-import", e));
        let status = child.wait().unwrap_or_else(|e| fail("wait for git fast-import", e));
        if !status.success() {
            fail("import a history", status);
        }
        self.git(&["checkout", "--quiet", "--force", checkout]);
    }

    /// Fetches from `origin` and dates the fetch `at`, so "fetched 2 minutes ago" reads the same
    /// on every run.
    pub fn fetched(&self, at: Timestamp) {
        self.git(&["fetch", "--quiet", "origin"]);
        let path = self.git(&["rev-parse", "--git-path", "FETCH_HEAD"]);
        let file = std::fs::File::options()
            .write(true)
            .open(self.path.join(path.trim()))
            .unwrap_or_else(|e| fail("open FETCH_HEAD", e));
        file.set_modified(at.into()).unwrap_or_else(|e| fail("date FETCH_HEAD", e));
    }

    /// Replaces `count` lines of `file` from line `start` (1-based) with `with`, uncommitted.
    pub fn replace_lines(&self, file: &str, start: usize, count: usize, with: &[String]) {
        let p = self.path.join(file);
        let text = std::fs::read_to_string(&p).unwrap_or_else(|e| fail("read a file to edit", e));
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let at = start - 1;
        if at + count > lines.len() {
            fail("edit a file", format!("{file} has {} lines, not {}", lines.len(), at + count));
        }
        lines.splice(at..at + count, with.iter().cloned());
        self.write(file, &(lines.join("\n") + "\n"));
    }

    pub fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    pub fn git(&self, args: &[&str]) -> String {
        self.world.git(&self.path, args)
    }

    fn git_as(&self, who: Author, at: Timestamp, args: &[&str]) -> String {
        self.world
            .git_as(&self.path, who, at, args)
            .unwrap_or_else(|e| fail(&format!("run git {} as {}", args.join(" "), who.name), e))
    }
}

/// Worlds are test scaffolding: a failure to build one is a broken test, reported loudly.
#[track_caller]
fn fail(what: &str, e: impl std::fmt::Display) -> ! {
    panic!("gxm-world couldn't {what}: {e}")
}

/// Hours, as a duration to go back by.
pub fn hours(n: i64) -> SignedDuration {
    SignedDuration::from_hours(n)
}

/// Days of 24 hours, as a duration to go back by.
pub fn days(n: i64) -> SignedDuration {
    SignedDuration::from_hours(n * 24)
}

/// Compares `actual` with the golden file at `path`. With `GXM_BLESS=1` (`cargo xtask bless`)
/// it writes the file instead, and the PR shows the diff.
#[track_caller]
pub fn golden(path: &Path, actual: &str) {
    #[expect(clippy::disallowed_methods, reason = "the bless switch is the test harness's, not the app's")]
    let bless = std::env::var("GXM_BLESS").is_ok_and(|v| v == "1");
    if bless {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap_or_else(|e| fail("make the goldens folder", e));
        }
        std::fs::write(path, actual).unwrap_or_else(|e| fail("write a golden", e));
        return;
    }
    let expected = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("no golden at {} yet: run `cargo xtask bless` and review it", path.display()));
    if expected != actual {
        panic!(
            "{} differs from its golden.\n--- golden\n{expected}\n--- actual\n{actual}\nIf the change is intended: cargo xtask bless, then show the diff in the PR.",
            path.display()
        );
    }
}
