//! git-ex-machina's releases on the fake GitHub: the REST API's latest release, and its AppImage
//! to download, as GitHub serves them to anyone for a public repository.

use sha2::{Digest, Sha256};

use super::State;
use super::server::Reply;

/// A published release.
#[derive(Clone, Debug)]
pub struct ReleaseData {
    pub version: String,
    pub notes: String,
    /// The AppImage's contents. A world's AppImage is a script that says it started.
    pub appimage: String,
}

pub(super) fn answer(s: &mut State, path: &str) -> Reply {
    let latest = format!("/repos/{}/releases/latest", gxm_core::update::REPO);
    if path == latest {
        return match s.releases.last() {
            Some(r) => (200, latest_json(&s.url, r).to_string()).into(),
            None => not_found(),
        };
    }
    let Some(rest) = path.strip_prefix("/releases/download/v") else { return not_found() };
    let Some((version, name)) = rest.split_once('/') else { return not_found() };
    let Some(release) = s.releases.iter().find(|r| r.version == version) else { return not_found() };
    if name != gxm_core::update::asset_name() {
        return not_found();
    }
    let mut body = release.appimage.clone();
    if s.damaged > 0 {
        s.damaged -= 1;
        // The same length, so only the digest can tell.
        body = body.chars().rev().collect();
    }
    Reply { status: 200, body, paced: true }
}

fn not_found() -> Reply {
    (404, r#"{"message":"Not Found"}"#.to_owned()).into()
}

/// The fields of GitHub's release JSON the app reads.
fn latest_json(url: &str, r: &ReleaseData) -> serde_json::Value {
    let name = gxm_core::update::asset_name();
    let digest: String = Sha256::digest(r.appimage.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    serde_json::json!({
        "tag_name": format!("v{}", r.version),
        "name": format!("git-ex-machina {}", r.version),
        "body": r.notes,
        "html_url": format!("https://github.com/{}/releases/tag/v{}", gxm_core::update::REPO, r.version),
        "draft": false,
        "prerelease": false,
        "assets": [{
            "name": name,
            "size": r.appimage.len(),
            "digest": format!("sha256:{digest}"),
            "browser_download_url": format!("{url}/releases/download/v{}/{name}", r.version),
        }],
    })
}
