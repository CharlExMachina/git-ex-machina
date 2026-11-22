//! Files attached to a release, uploaded once it exists. The release notes link each file where
//! the release will keep it (`…/releases/download/v1.0.0/shot.png`), but the release is made by
//! the repository's workflow after the tag is pushed, so the files wait in a queue on disk
//! (copied, so the originals can move) until GitHub has the release, then go up as its assets.
//! The queue outlives the app: what's left is tried again at the next start.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{ForgeError, http};
use crate::env::Env;
use crate::remote::{Provider, Remote};

/// A release's files still to upload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub host: String,
    pub owner: String,
    pub name: String,
    pub tag: String,
    /// Each file's asset name and the copy waiting to go up.
    pub files: Vec<(String, PathBuf)>,
}

/// Where a release keeps its files: what the notes link attached files under.
pub fn download_base(remote: &Remote, tag: &str) -> Option<String> {
    (remote.provider == Provider::GitHub)
        .then(|| format!("https://{}/{}/{}/releases/download/{tag}", remote.host, remote.owner, remote.name))
}

fn store(env: &Env) -> PathBuf {
    env.config_dir.join("attachments")
}

fn list_file(env: &Env) -> PathBuf {
    store(env).join("pending.json")
}

/// What's waiting to go up.
pub fn pending(env: &Env) -> Vec<Pending> {
    env.off_ui_thread(&"reading the attachments waiting for their releases");
    std::fs::read_to_string(list_file(env)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(env: &Env, list: &[Pending]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(store(env)).map_err(|e| e.to_string())?;
    std::fs::write(list_file(env), text).map_err(|e| e.to_string())
}

/// Queues `files` (asset name, where it is) for `tag`'s release on `remote`, copying each one.
pub fn queue(env: &Env, remote: &Remote, tag: &str, files: &[(String, PathBuf)]) -> Result<(), String> {
    let dir = store(env).join(&remote.owner).join(&remote.name).join(tag);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut copies = Vec::new();
    for (name, from) in files {
        let to = dir.join(name);
        std::fs::copy(from, &to).map_err(|e| format!("couldn't read {}: {e}", env.tilde(from)))?;
        copies.push((name.clone(), to));
    }
    let mut list = pending(env);
    list.retain(|p| !(p.owner == remote.owner && p.name == remote.name && p.tag == tag));
    list.push(Pending {
        host: remote.host.clone(),
        owner: remote.owner.clone(),
        name: remote.name.clone(),
        tag: tag.to_owned(),
        files: copies,
    });
    save(env, &list)
}

/// Uploads every queued release's files once GitHub has the release, waiting for it (asking
/// every 15 seconds, for up to three hours). Says what it attached.
pub fn attach_all(env: &Env) -> Result<String, String> {
    let mut said = Vec::new();
    for p in pending(env) {
        let n = attach(env, &p)?;
        let mut list = pending(env);
        list.retain(|q| q != &p);
        save(env, &list)?;
        if let Some(dir) = p.files.first().and_then(|(_, f)| f.parent()) {
            // The copies are up: they can go.
            std::fs::remove_dir_all(dir).ok();
        }
        said.push(match (n, p.files.first()) {
            (1, Some((name, _))) => format!("Attached {name} to {}", p.tag),
            _ => format!("Attached {n} files to {}", p.tag),
        });
    }
    Ok(said.join(" · "))
}

fn attach(env: &Env, p: &Pending) -> Result<usize, String> {
    let token = env.forge.github_token.as_deref().ok_or("Sign in to GitHub to attach the files")?;
    let api = if p.host.starts_with("github.com") {
        env.forge.github_api.trim_end_matches('/').to_owned()
    } else {
        format!("https://{}/api/v3", p.host)
    };
    let url = format!("{api}/repos/{}/{}/releases/tags/{}", p.owner, p.name, p.tag);
    // A world's GitHub answers at once; the real one's workflow takes minutes.
    let (every, tries) = if env.isolated { (Duration::from_millis(200), 150) } else { (Duration::from_secs(15), 720) };
    let mut release = None;
    for _ in 0..tries {
        match http::rest("GET", &url, token, None) {
            Ok(r) => {
                release = Some(r);
                break;
            }
            Err(ForgeError::Status(404)) => std::thread::sleep(every),
            Err(e) => return Err(format!("GitHub couldn't say whether {} is published: {e}", p.tag)),
        }
    }
    let release =
        release.ok_or_else(|| format!("{} still isn't published: its files go up at the next start", p.tag))?;
    let upload = release["upload_url"].as_str().unwrap_or_default();
    let upload = upload.split('{').next().unwrap_or(upload);
    let there: Vec<&str> =
        release["assets"].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect()).unwrap_or_default();
    let mut sent = 0;
    for (name, file) in &p.files {
        if there.contains(&name.as_str()) {
            sent += 1;
            continue;
        }
        let bytes = std::fs::read(file).map_err(|e| format!("{name}: {e}"))?;
        http::upload(&format!("{upload}?name={name}"), token, content_type(file), &bytes)
            .map_err(|e| format!("GitHub didn't take {name}: {e}"))?;
        sent += 1;
    }
    Ok(sent)
}

fn content_type(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("txt" | "md" | "log") => "text/plain",
        Some("json") => "application/json",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_github_release_keeps_its_files_under_its_tag() {
        let remote = Remote::parse("git@github.com:studio/beacon.git").unwrap();
        assert_eq!(
            download_base(&remote, "v0.1.2").as_deref(),
            Some("https://github.com/studio/beacon/releases/download/v0.1.2")
        );
        let gitlab = Remote::parse("git@gitlab.com:studio/beacon.git").unwrap();
        assert_eq!(download_base(&gitlab, "v0.1.2"), None);
    }
}
