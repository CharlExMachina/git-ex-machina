//! GitHub's "Generate release notes" for a tag that isn't there yet: the pull requests merged
//! since the previous release, who wrote them, who's new, and the compare link, as markdown.

use serde_json::json;

use super::{ForgeError, http};
use crate::env::Env;
use crate::remote::{Provider, Remote};

/// GitHub's notes for `tag` on `target` (a commit GitHub has), counting from `previous`, or from
/// GitHub's own idea of the last release when there's none. The reason, when it can't, reads as
/// a sentence.
pub fn generate(env: &Env, remote: &Remote, tag: &str, target: &str, previous: Option<&str>) -> Result<String, String> {
    if remote.provider != Provider::GitHub {
        return Err(format!("Drafting notes works on GitHub; {} is coming next.", remote.provider.name()));
    }
    let Some(token) = env.forge.github_token.as_deref() else {
        return Err("Sign in to GitHub to draft the notes from its pull requests.".to_owned());
    };
    let api = if remote.host.starts_with("github.com") {
        env.forge.github_api.trim_end_matches('/').to_owned()
    } else {
        format!("https://{}/api/v3", remote.host)
    };
    let mut body = json!({ "tag_name": tag, "target_commitish": target });
    if let Some(previous) = previous {
        body["previous_tag_name"] = json!(previous);
    }
    let url = format!("{api}/repos/{}/{}/releases/generate-notes", remote.owner, remote.name);
    match http::rest("POST", &url, token, Some(&body)) {
        Ok(answer) => Ok(answer["body"].as_str().unwrap_or_default().to_owned()),
        Err(ForgeError::Status(404)) => {
            Err("GitHub doesn't have this commit yet: push it, then draft the notes.".to_owned())
        }
        Err(e) => Err(format!("GitHub couldn't draft the notes: {e}")),
    }
}
