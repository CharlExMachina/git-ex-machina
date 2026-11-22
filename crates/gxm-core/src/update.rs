//! Updating the app in place. A release is a GitHub release tagged `vX.Y.Z`, carrying an AppImage
//! for each architecture (`git-ex-machina-x86_64.AppImage`) and its notes.
//!
//! When the app starts it asks GitHub for the latest release. A newer one downloads beside the
//! AppImage the app runs from, is checked against the SHA-256 GitHub keeps for it, and replaces
//! that file with one rename. The running app goes on from the old file, which the AppImage
//! runtime holds open, and the next start runs the new version; restarting starts it now. The
//! file keeps its name and place, so whatever starts it (a launcher entry, a shortcut) gets the
//! new version without being told.
//!
//! Only an AppImage updates itself ([`Install::appimage`](crate::env::Install)).

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::env::Env;
use crate::forge::{ForgeError, http};

/// Where releases are published.
pub const REPO: &str = "CharlExMachina/git-ex-machina";

/// A release newer than the running app, ready to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// `0.2.0`.
    pub version: String,
    /// What changed, as written for the release (Markdown).
    pub notes: String,
    /// The release's page on GitHub.
    pub page: String,
    pub asset: Asset,
}

/// The AppImage a release carries for this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub url: String,
    /// Bytes.
    pub size: u64,
    /// Hex, as GitHub reports the asset's digest.
    pub sha256: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UpdateError {
    /// The app isn't running from an AppImage, so there's no file to replace.
    NotAnAppImage,
    GitHub(ForgeError),
    /// The release has no AppImage for this machine.
    NoAsset(String),
    /// GitHub keeps no digest for the AppImage, so a download couldn't be checked.
    NoDigest,
    /// The download isn't the file GitHub has a digest for.
    Corrupt,
    /// The download couldn't be written, or put in place.
    Write(String),
    /// The new version couldn't be started.
    Start(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateError::NotAnAppImage => write!(f, "only the AppImage updates itself"),
            UpdateError::GitHub(e) => write!(f, "GitHub: {e}"),
            UpdateError::NoAsset(name) => write!(f, "the release has no {name}"),
            UpdateError::NoDigest => write!(f, "GitHub has no checksum for the download, so it can't be trusted"),
            UpdateError::Corrupt => write!(f, "the download was damaged on the way; try again"),
            UpdateError::Write(e) | UpdateError::Start(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for UpdateError {}

/// The AppImage a release carries for this machine's architecture.
pub fn asset_name() -> String {
    format!("git-ex-machina-{}.AppImage", std::env::consts::ARCH)
}

/// GitHub's latest release, as far as we read it.
#[derive(Deserialize)]
struct Latest {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    assets: Vec<LatestAsset>,
}

#[derive(Deserialize)]
struct LatestAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    /// `sha256:<hex>`.
    #[serde(default)]
    digest: Option<String>,
}

/// The latest release, when it's newer than the running app and the app can install it. None
/// when the app is up to date, when nothing is released yet, and when the app doesn't run from
/// an AppImage (it doesn't ask GitHub then).
pub fn check(env: &Env) -> Result<Option<Release>, UpdateError> {
    if env.install.appimage.is_none() {
        return Ok(None);
    }
    env.off_ui_thread(&"asking GitHub for the latest release");
    let url = format!("{}/repos/{REPO}/releases/latest", env.forge.github_api);
    let answer = match http::get_json(&url) {
        Ok(answer) => answer,
        Err(ForgeError::Status(404)) => return Ok(None),
        Err(e) => return Err(UpdateError::GitHub(e)),
    };
    let latest: Latest =
        serde_json::from_value(answer).map_err(|e| UpdateError::GitHub(ForgeError::Parse(e.to_string())))?;
    let (Some(newest), Some(running)) = (Version::parse(&latest.tag_name), Version::parse(&env.install.version)) else {
        return Ok(None);
    };
    if newest <= running {
        return Ok(None);
    }
    let name = asset_name();
    let asset = latest.assets.iter().find(|a| a.name == name).ok_or(UpdateError::NoAsset(name))?;
    let sha256 = asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).ok_or(UpdateError::NoDigest)?;
    Ok(Some(Release {
        version: newest.to_string(),
        notes: latest.body.unwrap_or_default().trim().to_owned(),
        page: latest.html_url,
        asset: Asset { url: asset.browser_download_url.clone(), size: asset.size, sha256: sha256.to_lowercase() },
    }))
}

/// Downloads `release` beside the running AppImage, checks it, and puts it in that AppImage's
/// place. `progress` hears the bytes so far and the total.
pub fn install(env: &Env, release: &Release, progress: &mut dyn FnMut(u64, u64)) -> Result<(), UpdateError> {
    env.off_ui_thread(&"downloading an update");
    let target = env.install.appimage.as_deref().ok_or(UpdateError::NotAnAppImage)?;
    // Beside it, so the rename that puts it in place never crosses file systems.
    let part = partial(target);
    let written = download_to(&part, release, progress);
    if written.is_err() {
        std::fs::remove_file(&part).ok();
    }
    written?;
    std::fs::rename(&part, target).map_err(|e| {
        std::fs::remove_file(&part).ok();
        UpdateError::Write(format!("couldn't replace {}: {e}", target.display()))
    })
}

/// `~/Applications/.git-ex-machina.AppImage.update` for `~/Applications/git-ex-machina.AppImage`.
fn partial(target: &Path) -> PathBuf {
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    target.with_file_name(format!(".{name}.update"))
}

fn download_to(part: &Path, release: &Release, progress: &mut dyn FnMut(u64, u64)) -> Result<(), UpdateError> {
    let cant = |e: std::io::Error| UpdateError::Write(format!("couldn't write {}: {e}", part.display()));
    let mut out = Hashed { file: std::fs::File::create(part).map_err(cant)?, hash: Sha256::new() };
    let size = release.asset.size;
    http::download(&release.asset.url, &mut out, &mut |done, total| progress(done, total.unwrap_or(size)))
        .map_err(UpdateError::GitHub)?;
    if hex(&out.hash.finalize()) != release.asset.sha256 {
        return Err(UpdateError::Corrupt);
    }
    out.file.sync_all().map_err(cant)?;
    executable(part).map_err(cant)
}

#[cfg(unix)]
fn executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn executable(_: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Starts the AppImage the app runs from, which is the new version once [`install`] has put it
/// there. The caller quits once it has started. (Rune's own relaunch would start the executable
/// inside the old AppImage's mount, which goes away with this process.)
pub fn restart(env: &Env) -> Result<(), UpdateError> {
    env.off_ui_thread(&"starting the new version");
    let appimage = env.install.appimage.as_deref().ok_or(UpdateError::NotAnAppImage)?;
    let mut cmd = std::process::Command::new(appimage);
    cmd.envs(env.child_vars())
        .current_dir(&env.home)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // Its own process group, so a terminal this one was started from doesn't take it down.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    cmd.spawn().map(drop).map_err(|e| UpdateError::Start(format!("couldn't start {}: {e}", appimage.display())))
}

/// Writes to a file and hashes what it wrote.
struct Hashed {
    file: std::fs::File,
    hash: Sha256,
}

impl Write for Hashed {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.file.write(buf)?;
        self.hash.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A release version, `major.minor.patch`. Pre-releases (`1.2.0-beta.1`) aren't offered, so they
/// don't parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u64, u64, u64);

impl Version {
    /// `v1.2.3` or `1.2.3`.
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        let mut parts = s.split('.').map(|p| p.parse::<u64>().ok());
        let v = Version(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(v)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_not_by_text() {
        let v = |s| Version::parse(s).unwrap_or_else(|| panic!("{s} parses"));
        assert_eq!(v("v0.1.0"), v("0.1.0"));
        assert!(v("0.10.0") > v("0.9.9"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert!(v("0.1.1") > v("0.1.0"));
        assert_eq!(v("v2.3.4").to_string(), "2.3.4");
        for bad in ["", "v", "1.2", "1.2.3.4", "1.2.x", "1.2.3-beta.1", "latest"] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_download_lands_beside_the_file_it_replaces() {
        let at = partial(Path::new("/home/ana/Applications/git-ex-machina.AppImage"));
        assert_eq!(at, Path::new("/home/ana/Applications/.git-ex-machina.AppImage.update"));
    }
}
