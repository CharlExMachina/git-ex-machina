//! Updating the app in place. A release is a GitHub release tagged `vX.Y.Z`, carrying a build for
//! each system and architecture (`git-ex-machina-x86_64.AppImage`,
//! `git-ex-machina-aarch64.dmg`) and its notes.
//!
//! When the app starts it asks GitHub for the latest release. A newer one downloads beside what
//! the app runs from, is checked against the SHA-256 GitHub keeps for it, and takes its place
//! (see [`appimage`] and [`macapp`] for how). The running app goes on from what it already
//! loaded, and the next start runs the new version; restarting starts it now. The bundle keeps
//! its name and place, so whatever starts it (a launcher entry, the Dock) gets the new version
//! without being told.
//!
//! Only an AppImage or a Mac app updates itself ([`Install::bundle`](crate::env::Install)).

mod appimage;
mod macapp;

use std::fmt;
use std::io::Write;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::env::{Bundle, Env};
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

/// The build a release carries for this machine.
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
    /// The app doesn't run from an AppImage or a Mac app, so there's nothing to replace.
    NotInstalled,
    GitHub(ForgeError),
    /// The release has no build for this machine.
    NoAsset(String),
    /// GitHub keeps no digest for the build, so a download couldn't be checked.
    NoDigest,
    /// The download isn't the file GitHub has a digest for.
    Corrupt,
    /// The new version isn't signed as the running one is.
    Untrusted(String),
    /// The download couldn't be written, opened or put in place.
    Write(String),
    /// The new version couldn't be started.
    Start(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateError::NotInstalled => write!(f, "only an AppImage or the Mac app updates itself"),
            UpdateError::GitHub(e) => write!(f, "GitHub: {e}"),
            UpdateError::NoAsset(name) => write!(f, "the release has no {name}"),
            UpdateError::NoDigest => write!(f, "GitHub has no checksum for the download, so it can't be trusted"),
            UpdateError::Corrupt => write!(f, "the download was damaged on the way; try again"),
            UpdateError::Untrusted(e) | UpdateError::Write(e) | UpdateError::Start(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for UpdateError {}

/// The build a release carries for `bundle` on this machine's architecture: its AppImage, or
/// the Mac app's disk image.
pub fn asset_name(bundle: &Bundle) -> String {
    let arch = std::env::consts::ARCH;
    match bundle {
        Bundle::AppImage(_) => format!("git-ex-machina-{arch}.AppImage"),
        Bundle::MacApp(_) => format!("git-ex-machina-{arch}.dmg"),
    }
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
/// something it can replace (it doesn't ask GitHub then).
pub fn check(env: &Env) -> Result<Option<Release>, UpdateError> {
    let Some(bundle) = &env.install.bundle else {
        return Ok(None);
    };
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
    let name = asset_name(bundle);
    let asset = latest.assets.iter().find(|a| a.name == name).ok_or(UpdateError::NoAsset(name))?;
    let sha256 = asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).ok_or(UpdateError::NoDigest)?;
    Ok(Some(Release {
        version: newest.to_string(),
        notes: latest.body.unwrap_or_default().trim().to_owned(),
        page: latest.html_url,
        asset: Asset { url: asset.browser_download_url.clone(), size: asset.size, sha256: sha256.to_lowercase() },
    }))
}

/// Downloads `release`, checks it, and puts it in place of what the app runs from. `progress`
/// hears the bytes so far and the total.
pub fn install(env: &Env, release: &Release, progress: &mut dyn FnMut(u64, u64)) -> Result<(), UpdateError> {
    env.off_ui_thread(&"downloading an update");
    match &env.install.bundle {
        Some(Bundle::AppImage(file)) => appimage::install(file, release, progress),
        Some(Bundle::MacApp(app)) => macapp::install(app, release, progress),
        None => Err(UpdateError::NotInstalled),
    }
}

/// Starts what the app runs from, which is the new version once [`install`] has put it there.
/// The caller quits once it has started.
pub fn restart(env: &Env) -> Result<(), UpdateError> {
    env.off_ui_thread(&"starting the new version");
    match &env.install.bundle {
        Some(Bundle::AppImage(file)) => appimage::restart(env, file),
        Some(Bundle::MacApp(app)) => macapp::restart(app),
        None => Err(UpdateError::NotInstalled),
    }
}

/// Downloads `release` to `to`, and keeps it only if it's the file GitHub has a digest for.
fn download(to: &Path, release: &Release, progress: &mut dyn FnMut(u64, u64)) -> Result<(), UpdateError> {
    let cant = |e: std::io::Error| UpdateError::Write(format!("couldn't write {}: {e}", to.display()));
    let written = (|| {
        let mut out = Hashed { file: std::fs::File::create(to).map_err(cant)?, hash: Sha256::new() };
        let size = release.asset.size;
        http::download(&release.asset.url, &mut out, &mut |done, total| progress(done, total.unwrap_or(size)))
            .map_err(UpdateError::GitHub)?;
        if hex(&out.hash.finalize()) != release.asset.sha256 {
            return Err(UpdateError::Corrupt);
        }
        out.file.sync_all().map_err(cant)
    })();
    if written.is_err() {
        std::fs::remove_file(to).ok();
    }
    written
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
}
