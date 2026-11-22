//! Releases of the world's repositories, as their release workflows publish them: a pushed tag
//! becomes a release on the second time anyone asks for it (the first finds the workflow still
//! running), its notes the tag's message (`gh release create --notes-from-tag`). Files go up as
//! its assets, the bytes kept, so a journey can check what arrived.

use serde_json::json;

use super::State;
use super::rest::{Answer, created, not_found, ok};

/// A release a workflow published.
#[derive(Clone, Debug)]
pub struct Published {
    pub owner: String,
    pub name: String,
    pub tag: String,
    pub id: u64,
    pub notes: String,
    pub assets: Vec<(String, Vec<u8>)>,
}

/// `GET /repos/owner/name/releases/tags/tag`.
pub(super) fn by_tag(s: &mut State, owner: &str, name: &str, tag: &str) -> Answer {
    if let Some(r) = s.published.iter().find(|r| r.owner == owner && r.name == name && r.tag == tag) {
        return ok(release_json(&s.url, r));
    }
    let bare = s.remotes.join("github.com").join(owner).join(format!("{name}.git"));
    let Ok(notes) = gxm_core::git::run(&s.env, &bare, &["tag", "-l", "--format=%(contents)", tag]) else {
        return not_found();
    };
    if notes.is_empty() && gxm_core::git::run(&s.env, &bare, &["rev-parse", "--verify", "--quiet", tag]).is_err() {
        return not_found();
    }
    let key = format!("{owner}/{name}@{tag}");
    let asked = s.release_asks.entry(key).or_insert(0);
    *asked += 1;
    if *asked < 2 {
        // The workflow is still building it.
        return not_found();
    }
    let id = u64::from(s.next_id);
    s.next_id += 1;
    let r = Published {
        owner: owner.to_owned(),
        name: name.to_owned(),
        tag: tag.to_owned(),
        id,
        notes,
        assets: Vec::new(),
    };
    let answer = ok(release_json(&s.url, &r));
    s.published.push(r);
    answer
}

/// `POST /uploads/repos/owner/name/releases/id/assets?name=file`, the file as the body.
pub(super) fn upload(s: &mut State, path: &str, bytes: &[u8]) -> Answer {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let ["uploads", "repos", owner, name, "releases", id, "assets"] = parts.as_slice() else { return not_found() };
    let Some(file) = query.split('&').find_map(|q| q.strip_prefix("name=")) else { return not_found() };
    let id: u64 = id.parse().unwrap_or(0);
    let Some(r) = s.published.iter_mut().find(|r| r.owner == *owner && r.name == *name && r.id == id) else {
        return not_found();
    };
    if r.assets.iter().any(|(n, _)| n == file) {
        return (422, json!({ "message": "Validation Failed", "errors": [{ "code": "already_exists" }] }).to_string());
    }
    r.assets.push((file.to_owned(), bytes.to_vec()));
    created(json!({ "name": file, "size": bytes.len() }))
}

fn release_json(url: &str, r: &Published) -> serde_json::Value {
    json!({
        "id": r.id,
        "tag_name": r.tag,
        "body": r.notes,
        "upload_url": format!("{url}/uploads/repos/{}/{}/releases/{}/assets{{?name,label}}", r.owner, r.name, r.id),
        "assets": r.assets.iter().map(|(n, b)| json!({ "name": n, "size": b.len() })).collect::<Vec<_>>(),
    })
}
