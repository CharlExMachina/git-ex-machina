//! Cloning: what a person pastes (a provider's page, an SSH URL, `owner/name`), the URL that
//! becomes `origin`, and `git clone` with its progress.
//!
//! SSH is the default: an HTTPS URL on GitHub, GitLab or Bitbucket clones over SSH. SSH URLs
//! stay exactly as typed (an `ssh` config alias picks the key), and so do HTTPS URLs on other
//! hosts, where SSH may not be set up.

use std::fmt;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;

use crate::env::Env;
use crate::git::{self, GitError};
use crate::remote::{Protocol, Provider, Remote};

/// What to clone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// The URL git clones from, and `origin` from then on.
    pub url: String,
    pub remote: Remote,
}

/// What `typed` asks to clone, or None when it isn't a repository URL.
///
/// Takes a page of a repository as the browser shows it (`https://github.com/studio/site/tree/main`,
/// even without `https://`), any SSH URL, and `owner/name` for GitHub.
pub fn source(typed: &str) -> Option<Source> {
    let typed = typed.trim().trim_end_matches('/');
    let scp = typed.split_once(':').is_some_and(|(a, _)| !a.contains('/'));
    let url = if typed.contains("://") || scp {
        typed.to_owned()
    } else if typed.split('/').next().is_some_and(|host| host.contains('.')) {
        format!("https://{typed}")
    } else {
        match typed.split('/').collect::<Vec<_>>().as_slice() {
            [owner, name] if !owner.is_empty() && !name.is_empty() => {
                format!("git@github.com:{owner}/{}.git", name.trim_end_matches(".git"))
            }
            _ => return None,
        }
    };
    let remote = Remote::parse(&repository_page(&url))?;
    let url = match (remote.protocol, remote.provider) {
        (Protocol::Https, Provider::GitHub | Provider::GitLab | Provider::Bitbucket) => remote.ssh_url(),
        _ => url,
    };
    Some(Source { url, remote })
}

/// A provider's HTTPS page trimmed to its repository: GitHub and Bitbucket pages go
/// `owner/name/…`, GitLab's `group/…/name/-/…`.
fn repository_page(url: &str) -> String {
    let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) else {
        return url.to_owned();
    };
    let Some((host, path)) = rest.split_once('/') else { return url.to_owned() };
    let segments: Vec<&str> = path.split(['?', '#']).next().unwrap_or("").split('/').collect();
    let keep = match Provider::from_host(host) {
        Provider::GitHub | Provider::Bitbucket => 2.min(segments.len()),
        Provider::GitLab => segments.iter().position(|s| *s == "-").unwrap_or(segments.len()),
        Provider::Other => return url.to_owned(),
    };
    format!("https://{host}/{}", segments[..keep].join("/"))
}

/// Where cloning `source` into the folder `into` puts it.
pub fn destination(into: &Path, source: &Source) -> PathBuf {
    into.join(&source.remote.name)
}

/// Why a clone didn't happen.
#[derive(Debug)]
pub enum CloneError {
    /// Something is already there; cloning would mix two repositories.
    Exists(PathBuf),
    Cancelled,
    Io {
        what: &'static str,
        error: std::io::Error,
    },
    Git(GitError),
}

impl fmt::Display for CloneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CloneError::Exists(path) => write!(f, "{} already exists", path.display()),
            CloneError::Cancelled => write!(f, "the clone was cancelled"),
            CloneError::Io { what, error } => write!(f, "couldn't {what}: {error}"),
            CloneError::Git(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for CloneError {}

/// Clones `url` into `dest`, which must be missing or empty. `progress` hears each step git
/// reports (`Receiving objects 45%`); when `cancelled` turns true, git is stopped and the
/// partial clone removed.
pub fn run(
    env: &Env,
    url: &str,
    dest: &Path,
    progress: &mut dyn FnMut(&str),
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CloneError> {
    env.off_ui_thread(&format_args!("cloning {url}"));
    let occupied = dest.is_file() || std::fs::read_dir(dest).is_ok_and(|mut d| d.next().is_some());
    if occupied {
        return Err(CloneError::Exists(dest.to_path_buf()));
    }
    let parent = dest.parent().unwrap_or(dest);
    std::fs::create_dir_all(parent).map_err(|error| CloneError::Io { what: "make the folder", error })?;
    let dest_arg = dest.to_string_lossy();
    let mut child = git::command(env, parent, &["clone", "--progress", "--", url, &dest_arg])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| CloneError::Git(GitError::Spawn { message: e.to_string() }))?;
    // git reports progress on stderr, redrawing a line with `\r`; a thread reads it so this one
    // can keep checking for a cancel while git waits on the network.
    let (tx, rx) = mpsc::channel::<String>();
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut said = Vec::new();
        let mut line = Vec::new();
        let mut buf = [0u8; 4096];
        while let Some(n) = stderr.as_mut().and_then(|e| e.read(&mut buf).ok()).filter(|n| *n > 0) {
            for &b in &buf[..n] {
                said.push(b);
                if b == b'\r' || b == b'\n' {
                    tx.send(String::from_utf8_lossy(&line).into_owned()).ok();
                    line.clear();
                } else {
                    line.push(b);
                }
            }
        }
        String::from_utf8_lossy(&said).into_owned()
    });
    let status = loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(line) => {
                if let Some(p) = progress_line(&line) {
                    progress(&p);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break child.wait().map_err(|error| CloneError::Io { what: "wait for git", error })?;
            }
        }
        if cancelled() {
            child.kill().ok();
            child.wait().ok();
            reader.join().ok();
            std::fs::remove_dir_all(dest).ok();
            return Err(CloneError::Cancelled);
        }
    };
    let said = reader.join().unwrap_or_default();
    if !status.success() {
        return Err(CloneError::Git(GitError::Failed {
            args: format!("clone {url}"),
            code: status.code(),
            stderr: said,
        }));
    }
    Ok(())
}

/// One of git's progress lines as a person reads it: `Receiving objects:  45% (450/1000),
/// 1.20 MiB | 2.3 MiB/s` is `Receiving objects 45%`. None for any other line.
pub fn progress_line(line: &str) -> Option<String> {
    let line = line.trim();
    let line = line.strip_prefix("remote:").unwrap_or(line).trim();
    let (phase, rest) = line.split_once(':')?;
    let pct = rest.trim().split_once('%')?.0.trim();
    pct.parse::<u8>().ok()?;
    Some(format!("{phase} {pct}%"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(typed: &str) -> Option<String> {
        source(typed).map(|s| s.url)
    }

    #[test]
    fn what_people_paste_clones_over_ssh() {
        let ssh = Some("git@github.com:studio/site.git".to_owned());
        assert_eq!(url("https://github.com/studio/site"), ssh);
        assert_eq!(url("https://github.com/studio/site.git"), ssh);
        assert_eq!(url("https://github.com/studio/site/tree/main/src?tab=readme#top"), ssh, "a deeper page");
        assert_eq!(url("github.com/studio/site"), ssh, "the address bar without https://");
        assert_eq!(url("studio/site"), ssh, "owner/name is on GitHub");
        assert_eq!(url("git@github.com:studio/site.git"), ssh);
        assert_eq!(
            url("https://gitlab.com/group/sub/infra/-/tree/main"),
            Some("git@gitlab.com:group/sub/infra.git".to_owned())
        );
        assert_eq!(
            url("https://bitbucket.org/team/payments-api/src/main/"),
            Some("git@bitbucket.org:team/payments-api.git".to_owned())
        );
        // SSH stays exactly as typed: an alias picks the key, a port matters.
        assert_eq!(url("git@github.com-work:studio/site.git"), Some("git@github.com-work:studio/site.git".to_owned()));
        assert_eq!(url("ssh://git@host.example:2222/a/b.git"), Some("ssh://git@host.example:2222/a/b.git".to_owned()));
        // HTTPS elsewhere stays HTTPS: SSH may not be set up there.
        assert_eq!(url("https://git.example.com/a/b.git"), Some("https://git.example.com/a/b.git".to_owned()));
        assert_eq!(source("https://github.com/studio/site").map(|s| s.remote.name), Some("site".to_owned()));
        for nothing in ["", "site", "a/b/c", "not a url"] {
            assert_eq!(url(nothing), None, "{nothing:?}");
        }
    }

    #[test]
    fn progress_reads_as_a_phase_and_a_percentage() {
        assert_eq!(
            progress_line("Receiving objects:  45% (450/1000), 1.20 MiB | 2.30 MiB/s").as_deref(),
            Some("Receiving objects 45%")
        );
        assert_eq!(
            progress_line("remote: Counting objects: 100% (5/5), done.").as_deref(),
            Some("Counting objects 100%")
        );
        assert_eq!(progress_line("Cloning into '/home/ana/code/site'..."), None);
        assert_eq!(progress_line("warning: redirecting to https://github.com/x/y.git/"), None);
    }
}
