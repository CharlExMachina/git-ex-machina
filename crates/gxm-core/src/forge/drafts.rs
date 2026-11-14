//! A review in progress: comments, replies, the verdict, the summary and the files marked
//! viewed. Kept on this machine, so nothing reaches the provider until the review is submitted,
//! and nothing is lost if the app closes first.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::model::{NewComment, Reply, Side, Submission, Verdict};
use crate::env::Env;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDraft {
    pub verdict: Verdict,
    pub summary: String,
    pub comments: Vec<NewComment>,
    pub replies: Vec<Reply>,
    /// Files the person has marked viewed.
    pub viewed: Vec<String>,
}

impl ReviewDraft {
    /// Comments and replies waiting to be sent.
    pub fn pending(&self) -> usize {
        self.comments.len() + self.replies.len()
    }

    pub fn is_viewed(&self, path: &str) -> bool {
        self.viewed.iter().any(|p| p == path)
    }

    pub fn toggle_viewed(&mut self, path: &str) {
        if self.is_viewed(path) {
            self.viewed.retain(|p| p != path);
        } else {
            self.viewed.push(path.to_owned());
        }
    }

    pub fn comments_on(&self, path: &str, side: Side, line: u32) -> impl Iterator<Item = &NewComment> {
        self.comments.iter().filter(move |c| c.path == path && c.side == side && c.line == line)
    }

    pub fn replies_to(&self, thread: &str) -> impl Iterator<Item = &Reply> {
        self.replies.iter().filter(move |r| r.thread == thread)
    }

    /// The review to send.
    pub fn submission(&self) -> Submission {
        Submission {
            verdict: self.verdict,
            body: self.summary.trim().to_owned(),
            comments: self.comments.clone(),
            replies: self.replies.clone(),
        }
    }
}

/// `<config>/reviews/<host>/<owner>/<repo>/<number>.json`.
pub fn path(env: &Env, host: &str, owner: &str, repo: &str, number: u64) -> PathBuf {
    env.config_dir.join("reviews").join(host).join(owner).join(repo).join(format!("{number}.json"))
}

/// The saved draft, or a fresh one.
pub fn load(path: &std::path::Path) -> ReviewDraft {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Saves the draft, or removes the file once there's nothing in it worth keeping.
pub fn save(path: &std::path::Path, draft: &ReviewDraft) -> std::io::Result<()> {
    if *draft == ReviewDraft::default() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(draft).map_err(std::io::Error::other)?;
    std::fs::write(path, text + "\n")
}
