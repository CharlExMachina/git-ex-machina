//! The world's GitHub REST API for repository settings: the paths the app's settings client
//! calls, answered in GitHub's shapes from [`super::admin::Settings`]. Writes need the viewer to
//! be an admin of the repository, as on GitHub; deleting one needs a token granted
//! `delete_repo` too.

use serde_json::{Value, json};

use super::State;
use super::admin::{InvitationData, Settings};

/// A REST answer: status and body (empty for 204).
pub(super) type Answer = (u16, String);

pub(super) fn ok(v: impl std::fmt::Display) -> Answer {
    (200, v.to_string())
}

pub(super) fn created(v: impl std::fmt::Display) -> Answer {
    (201, v.to_string())
}

pub(super) fn done() -> Answer {
    (204, String::new())
}

pub(super) fn not_found() -> Answer {
    (404, json!({ "message": "Not Found" }).to_string())
}

pub(super) fn invalid(message: &str) -> Answer {
    (422, json!({ "message": "Validation Failed", "errors": [{ "message": message }] }).to_string())
}

fn not_admin() -> Answer {
    (403, json!({ "message": "Must have admin rights to Repository." }).to_string())
}

/// Answers `method path` with `body`, for a request made with `token`.
pub(super) fn answer(s: &mut State, method: &str, path: &str, body: &str, can_delete: bool) -> Answer {
    let path = path.split('?').next().unwrap_or(path);
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let body: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    match parts.as_slice() {
        ["users", login] => ok(json!({ "login": login, "id": super::user_id(login) })),
        ["orgs", org] => match s.settings.iter().find(|((o, _), st)| o == org && st.owner_is_org) {
            Some((_, st)) => ok(
                json!({ "login": org, "default_repository_permission": st.base_role.clone().unwrap_or("none".to_owned()) }),
            ),
            None => not_found(),
        },
        ["orgs", _org, "teams", slug, "repos", owner, name] => {
            let viewer = s.viewer.login;
            let Some(st) = s.settings_mut(owner, name) else { return not_found() };
            if st.role_of(viewer) != "admin" {
                return not_admin();
            }
            let Some(team) = st.teams.iter_mut().position(|t| t.0 == *slug) else { return not_found() };
            match method {
                "PUT" => {
                    st.teams[team].2 = body["permission"].as_str().unwrap_or("pull").to_owned();
                    done()
                }
                "DELETE" => {
                    st.teams.remove(team);
                    done()
                }
                _ => not_found(),
            }
        }
        ["repos", owner, name, "releases", "generate-notes"] if method == "POST" => {
            super::notes::generate(s, owner, name, &body)
        }
        ["repos", owner, name, rest @ ..] => {
            // A renamed repository answers to its old name too, by its new one.
            let name = s.current_name(owner, name);
            let owner = (*owner).to_owned();
            repo(s, method, &owner, &name, rest, &body, can_delete)
        }
        _ => not_found(),
    }
}

fn repo(s: &mut State, method: &str, owner: &str, name: &str, rest: &[&str], body: &Value, can_delete: bool) -> Answer {
    let viewer = s.viewer.login;
    let now = s.now;
    let next_id = s.next_id;
    let Some(st) = s.settings_mut(owner, name) else { return not_found() };
    if st.deleted {
        return not_found();
    }
    let admin = st.role_of(viewer) == "admin";
    if method != "GET" && !admin {
        return not_admin();
    }
    let mut bump = false;
    let answer = match (method, rest) {
        ("GET", []) => ok(st.repo_json(owner, name, viewer)),
        ("PATCH", []) => {
            if let Some(new) = body["name"].as_str().filter(|n| *n != name) {
                return rename(s, owner, name, new);
            }
            st.patch(body);
            ok(st.repo_json(owner, name, viewer))
        }
        ("DELETE", []) if !can_delete => not_admin(),
        ("DELETE", []) => {
            st.deleted = true;
            done()
        }
        ("POST", ["transfer"]) => match body["new_owner"].as_str() {
            Some(to) if !to.is_empty() => {
                st.transferred_to = Some(to.to_owned());
                (202, json!({ "full_name": format!("{to}/{name}") }).to_string())
            }
            _ => invalid("new_owner is missing"),
        },
        ("GET", ["topics"]) => ok(json!({ "names": st.topics })),
        ("PUT", ["topics"]) => {
            st.topics = body["names"].as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect()).unwrap_or_default();
            ok(json!({ "names": st.topics }))
        }
        ("GET", ["collaborators"]) => ok(json!(
            st.collaborators.iter().map(|(l, r)| json!({ "login": l, "role_name": r })).collect::<Vec<_>>()
        )),
        ("PUT", ["collaborators", login]) => {
            let role = match body["permission"].as_str().unwrap_or("push") {
                "pull" | "read" => "read",
                "push" | "write" => "write",
                other => other,
            }
            .to_owned();
            if let Some(c) = st.collaborators.iter_mut().find(|(l, _)| l == login) {
                c.1 = role;
                done()
            } else if login.contains(' ') || login.is_empty() {
                invalid("That isn't a GitHub username")
            } else {
                bump = true;
                st.invitations.push(InvitationData { id: next_id.into(), login: (*login).to_owned(), permission: role, created: now });
                created(json!({ "id": next_id }))
            }
        }
        ("DELETE", ["collaborators", login]) => {
            st.collaborators.retain(|(l, _)| l != login);
            done()
        }
        ("GET", ["invitations"]) => ok(json!(
            st.invitations
                .iter()
                .map(|i| json!({ "id": i.id, "invitee": { "login": i.login }, "permissions": i.permission, "created_at": i.created.to_string() }))
                .collect::<Vec<_>>()
        )),
        ("DELETE", ["invitations", id]) => {
            st.invitations.retain(|i| i.id.to_string() != *id);
            done()
        }
        ("GET", ["teams"]) if st.owner_is_org => ok(json!(
            st.teams.iter().map(|(slug, name, p)| json!({ "slug": slug, "name": name, "permission": p })).collect::<Vec<_>>()
        )),
        ("GET", ["rulesets"]) => ok(json!(
            st.rulesets.iter().map(|r| json!({ "id": r["id"], "name": r["name"], "target": r["target"], "enforcement": r["enforcement"] })).collect::<Vec<_>>()
        )),
        ("POST", ["rulesets"]) => {
            if body["name"].as_str().is_none_or(str::is_empty) {
                invalid("A rule set needs a name")
            } else {
                bump = true;
                let mut r = body.clone();
                r["id"] = json!(next_id);
                st.rulesets.push(r.clone());
                created(r)
            }
        }
        (_, ["rulesets", id]) => {
            let Some(i) = st.rulesets.iter().position(|r| r["id"].as_u64().is_some_and(|n| n.to_string() == *id)) else { return not_found() };
            match method {
                "GET" => ok(st.rulesets[i].clone()),
                "PUT" => {
                    let mut r = body.clone();
                    r["id"] = st.rulesets[i]["id"].clone();
                    st.rulesets[i] = r.clone();
                    ok(r)
                }
                "DELETE" => {
                    st.rulesets.remove(i);
                    done()
                }
                _ => not_found(),
            }
        }
        _ => {
            let site = format!("https://{owner}.github.io/{name}/");
            super::rest_more::answer(st, &site, method, rest, body, now, next_id, &mut bump)
        }
    };
    if bump {
        s.next_id += 1;
    }
    answer
}

/// `PATCH` with a new name: the repository answers by it from now on, and its old name
/// redirects (git's URLs too: the bare repository keeps a link under the old name).
fn rename(s: &mut State, owner: &str, old: &str, new: &str) -> Answer {
    if s.settings.iter().any(|((o, n), _)| o == owner && n == new) {
        return invalid("name already exists on this account");
    }
    let base = s.remotes.join("github.com").join(owner);
    let (from, to) = (base.join(format!("{old}.git")), base.join(format!("{new}.git")));
    if std::fs::rename(&from, &to).is_err() {
        return invalid("the repository couldn't be renamed");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&to, &from).ok();
    if let Some(entry) = s.settings.iter_mut().find(|((o, n), _)| o == owner && n == old) {
        entry.0.1 = new.to_owned();
    }
    for (o, n, _) in &mut s.repos {
        if o == owner && n == old {
            *n = new.to_owned();
        }
    }
    s.renamed.push(((owner.to_owned(), old.to_owned()), new.to_owned()));
    let viewer = s.viewer.login;
    match s.settings_mut(owner, new) {
        Some(st) => ok(st.repo_json(owner, new, viewer)),
        None => not_found(),
    }
}

impl State {
    /// The name `owner/name` goes by now, after any renames.
    fn current_name(&self, owner: &str, name: &str) -> String {
        self.renamed.iter().find(|((o, n), _)| o == owner && n == name).map_or(name.to_owned(), |(_, new)| new.clone())
    }

    /// `owner/name`'s settings, following a rename, made as GitHub would for a repository the
    /// world has but no fixture set up. None for a repository the world doesn't have.
    pub(super) fn settings_mut(&mut self, owner: &str, name: &str) -> Option<&mut Settings> {
        let name = self.current_name(owner, name);
        if !self.settings.iter().any(|((o, n), _)| o == owner && *n == name) {
            if !self.remotes.join("github.com").join(owner).join(format!("{name}.git")).exists() {
                return None;
            }
            let viewer = self.viewer.login;
            self.settings.push(((owner.to_owned(), name.clone()), Settings::new(viewer)));
        }
        self.settings.iter_mut().find(|((o, n), _)| o == owner && *n == name).map(|(_, st)| st)
    }
}
