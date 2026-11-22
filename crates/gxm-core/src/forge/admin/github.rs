//! GitHub's settings, through its REST API: the repository itself, its topics, who has access,
//! and its rule sets here; Actions, secrets and the rest in [`super::github_more`].

use serde_json::{Value, json};

use super::model::*;
use super::{Admin, Page, Reading, Write};
use crate::forge::{ForgeError, http};
use crate::remote::Provider;

pub struct GitHub {
    api: String,
    token: String,
    pub(super) owner: String,
    pub(super) name: String,
}

impl GitHub {
    pub fn new(api: &str, token: String, owner: &str, name: &str) -> GitHub {
        GitHub { api: api.trim_end_matches('/').to_owned(), token, owner: owner.to_owned(), name: name.to_owned() }
    }

    /// `method` on `path` (from the API root), with `body` as JSON.
    #[expect(clippy::needless_pass_by_value, reason = "callers build each body inside the call")]
    pub(super) fn call(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, ForgeError> {
        http::rest(method, &format!("{}{path}", self.api), &self.token, body.as_ref())
    }

    pub(super) fn get(&self, path: &str) -> Result<Value, ForgeError> {
        self.call("GET", path, None)
    }

    /// `/repos/owner/name`, then `rest`.
    pub(super) fn repo_path(&self, rest: &str) -> String {
        format!("/repos/{}/{}{rest}", self.owner, self.name)
    }

    fn repo(&self) -> Result<Repo, ForgeError> {
        let v = self.get(&self.repo_path(""))?;
        Ok(repo(&v))
    }

    fn people(&self) -> Result<People, ForgeError> {
        let collaborators = list(&self.get(&self.repo_path("/collaborators?affiliation=direct&per_page=100"))?)
            .iter()
            .map(|c| Collaborator {
                login: s(&c["login"]),
                name: c["name"].as_str().map(str::to_owned),
                role: role(c["role_name"].as_str().unwrap_or("read")),
            })
            .collect();
        let invitations = list(&self.get(&self.repo_path("/invitations?per_page=100"))?)
            .iter()
            .map(|i| Invitation {
                id: i["id"].as_u64().unwrap_or(0),
                login: s(&i["invitee"]["login"]),
                role: role(i["permissions"].as_str().unwrap_or("read")),
                sent: when(&i["created_at"]),
            })
            .collect();
        // A person's repository has no teams, and an organisation may not show its base role.
        let teams = match self.get(&self.repo_path("/teams?per_page=100")) {
            Ok(v) => list(&v)
                .iter()
                .map(|t| Team {
                    slug: s(&t["slug"]),
                    name: s(&t["name"]),
                    role: role(t["permission"].as_str().unwrap_or("pull")),
                })
                .collect(),
            Err(ForgeError::Status(403 | 404)) => Vec::new(),
            Err(e) => return Err(e),
        };
        let base = self
            .get(&format!("/orgs/{}", self.owner))
            .ok()
            .and_then(|o| o["default_repository_permission"].as_str().filter(|p| *p != "none").map(role));
        Ok(People { collaborators, invitations, teams, base })
    }

    fn rulesets(&self) -> Result<Vec<Ruleset>, ForgeError> {
        let mut out = Vec::new();
        for r in list(&self.get(&self.repo_path("/rulesets?includes_parents=false"))?) {
            let id = r["id"].as_u64().unwrap_or(0);
            out.push(ruleset(&self.get(&self.repo_path(&format!("/rulesets/{id}")))?));
        }
        Ok(out)
    }

    fn team_path(&self, slug: &str) -> String {
        format!("/orgs/{owner}/teams/{slug}/repos/{owner}/{name}", owner = self.owner, name = self.name)
    }
}

impl Admin for GitHub {
    fn provider(&self) -> Provider {
        Provider::GitHub
    }

    fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    fn read(&self, page: Page) -> Result<Reading, ForgeError> {
        Ok(match page {
            Page::Repo => Reading::Repo(Box::new(self.repo()?)),
            Page::People => Reading::People(self.people()?),
            Page::Rules => Reading::Rules(self.rulesets()?),
            other => self.read_more(other)?,
        })
    }

    fn write(&self, w: &Write) -> Result<(), ForgeError> {
        let permission = |r: Role| json!({ "permission": api_role(r) });
        match w {
            Write::Change(c) => self.call("PATCH", &self.repo_path(""), Some(patch(c))).map(drop),
            Write::Topics(names) => {
                self.call("PUT", &self.repo_path("/topics"), Some(json!({ "names": names }))).map(drop)
            }
            Write::Invite { login, role } | Write::SetRole { who: Grantee::Person(login), role } => {
                self.call("PUT", &self.repo_path(&format!("/collaborators/{login}")), Some(permission(*role))).map(drop)
            }
            Write::SetRole { who: Grantee::Team(slug), role } => {
                self.call("PUT", &self.team_path(slug), Some(permission(*role))).map(drop)
            }
            Write::Remove(Grantee::Person(login)) => {
                self.call("DELETE", &self.repo_path(&format!("/collaborators/{login}")), None).map(drop)
            }
            Write::Remove(Grantee::Team(slug)) => self.call("DELETE", &self.team_path(slug), None).map(drop),
            Write::CancelInvite(id) => {
                self.call("DELETE", &self.repo_path(&format!("/invitations/{id}")), None).map(drop)
            }
            Write::SaveRuleset(r) if r.id == 0 => {
                self.call("POST", &self.repo_path("/rulesets"), Some(ruleset_body(r))).map(drop)
            }
            Write::SaveRuleset(r) => {
                self.call("PUT", &self.repo_path(&format!("/rulesets/{}", r.id)), Some(ruleset_body(r))).map(drop)
            }
            Write::DeleteRuleset(id) => {
                self.call("DELETE", &self.repo_path(&format!("/rulesets/{id}")), None).map(drop)
            }
            Write::Transfer(owner) => {
                self.call("POST", &self.repo_path("/transfer"), Some(json!({ "new_owner": owner }))).map(drop)
            }
            Write::Delete => self.call("DELETE", &self.repo_path(""), None).map(drop),
            other => self.write_more(other),
        }
    }
}

pub(super) fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

pub(super) fn list(v: &Value) -> &[Value] {
    v.as_array().map_or(&[], Vec::as_slice)
}

pub(super) fn when(v: &Value) -> jiff::Timestamp {
    v.as_str().and_then(|t| t.parse().ok()).unwrap_or_default()
}

/// A role as GitHub writes it: `read` or `pull`, `write` or `push`, and the rest by name.
fn role(name: &str) -> Role {
    match name {
        "admin" => Role::Admin,
        "maintain" => Role::Maintain,
        "write" | "push" => Role::Write,
        "triage" => Role::Triage,
        _ => Role::Read,
    }
}

/// A role as GitHub's writes take it.
fn api_role(r: Role) -> &'static str {
    match r {
        Role::Read => "pull",
        Role::Triage => "triage",
        Role::Write => "push",
        Role::Maintain => "maintain",
        Role::Admin => "admin",
    }
}

fn repo(v: &Value) -> Repo {
    let b = |k: &str| v[k].as_bool().unwrap_or(false);
    let p = &v["permissions"];
    let mine = [("admin", Role::Admin), ("maintain", Role::Maintain), ("push", Role::Write), ("triage", Role::Triage)]
        .into_iter()
        .find_map(|(k, r)| p[k].as_bool().unwrap_or(false).then_some(r))
        .unwrap_or(Role::Read);
    let squash_message = match (v["squash_merge_commit_title"].as_str(), v["squash_merge_commit_message"].as_str()) {
        (Some("PR_TITLE"), Some("BLANK")) => Message::Title,
        (Some("PR_TITLE"), Some("PR_BODY")) => Message::TitleAndDescription,
        _ => Message::Default,
    };
    let merge_message = match (v["merge_commit_title"].as_str(), v["merge_commit_message"].as_str()) {
        (Some("PR_TITLE"), Some("BLANK")) => Message::Title,
        (Some("PR_TITLE"), Some("PR_BODY")) => Message::TitleAndDescription,
        _ => Message::Default,
    };
    Repo {
        owner: s(&v["owner"]["login"]),
        owner_is_org: v["owner"]["type"].as_str() == Some("Organization"),
        name: s(&v["name"]),
        description: s(&v["description"]),
        homepage: s(&v["homepage"]),
        topics: list(&v["topics"]).iter().map(s).collect(),
        visibility: match v["visibility"].as_str() {
            Some("public") => Visibility::Public,
            Some("internal") => Visibility::Internal,
            _ => Visibility::Private,
        },
        archived: b("archived"),
        template: b("is_template"),
        default_branch: s(&v["default_branch"]),
        issues: b("has_issues"),
        projects: b("has_projects"),
        wiki: b("has_wiki"),
        discussions: b("has_discussions"),
        merge_commit: b("allow_merge_commit"),
        merge_message,
        squash: b("allow_squash_merge"),
        squash_message,
        rebase: b("allow_rebase_merge"),
        auto_merge: b("allow_auto_merge"),
        update_branch: b("allow_update_branch"),
        delete_branch_on_merge: b("delete_branch_on_merge"),
        signoff: b("web_commit_signoff_required"),
        role: mine,
    }
}

/// The body of `PATCH /repos/{owner}/{repo}` that makes `change`.
pub(crate) fn patch(change: &Change) -> Value {
    let message = |m: Message, default_title: &str, default_body: &str| match m {
        Message::Default => (default_title.to_owned(), default_body.to_owned()),
        Message::Title => ("PR_TITLE".to_owned(), "BLANK".to_owned()),
        Message::TitleAndDescription => ("PR_TITLE".to_owned(), "PR_BODY".to_owned()),
    };
    match change {
        Change::Name(v) => json!({ "name": v }),
        Change::Description(v) => json!({ "description": v }),
        Change::Homepage(v) => json!({ "homepage": v }),
        Change::Visibility(v) => json!({ "visibility": v.word() }),
        Change::Archived(v) => json!({ "archived": v }),
        Change::Template(v) => json!({ "is_template": v }),
        Change::DefaultBranch(v) => json!({ "default_branch": v }),
        Change::Issues(v) => json!({ "has_issues": v }),
        Change::Projects(v) => json!({ "has_projects": v }),
        Change::Wiki(v) => json!({ "has_wiki": v }),
        Change::Discussions(v) => json!({ "has_discussions": v }),
        Change::MergeCommit(v) => json!({ "allow_merge_commit": v }),
        Change::MergeMessage(m) => {
            let (title, body) = message(*m, "MERGE_MESSAGE", "PR_TITLE");
            json!({ "merge_commit_title": title, "merge_commit_message": body })
        }
        Change::Squash(v) => json!({ "allow_squash_merge": v }),
        Change::SquashMessage(m) => {
            let (title, body) = message(*m, "COMMIT_OR_PR_TITLE", "COMMIT_MESSAGES");
            json!({ "squash_merge_commit_title": title, "squash_merge_commit_message": body })
        }
        Change::Rebase(v) => json!({ "allow_rebase_merge": v }),
        Change::AutoMerge(v) => json!({ "allow_auto_merge": v }),
        Change::UpdateBranch(v) => json!({ "allow_update_branch": v }),
        Change::DeleteBranchOnMerge(v) => json!({ "delete_branch_on_merge": v }),
        Change::Signoff(v) => json!({ "web_commit_signoff_required": v }),
    }
}

/// A ref pattern as people write it, from GitHub's: `refs/heads/main` → `main`.
fn short_ref(r: &str) -> String {
    r.strip_prefix("refs/heads/").or_else(|| r.strip_prefix("refs/tags/")).unwrap_or(r).to_owned()
}

/// A ref pattern as GitHub takes it: `main` → `refs/heads/main` (or `refs/tags/` for tags);
/// `~DEFAULT_BRANCH` and `~ALL` stay as they are.
fn full_ref(target: Target, r: &str) -> String {
    if r.starts_with('~') || r.starts_with("refs/") {
        return r.to_owned();
    }
    match target {
        Target::Branch => format!("refs/heads/{r}"),
        Target::Tag => format!("refs/tags/{r}"),
    }
}

/// The id GitHub gives the repository's admin role among a rule set's bypass actors.
const ADMIN_ROLE: u64 = 5;

pub(crate) fn ruleset(v: &Value) -> Ruleset {
    let mut rules = Rules::default();
    for r in list(&v["rules"]) {
        let p = &r["parameters"];
        match r["type"].as_str().unwrap_or_default() {
            "pull_request" => {
                rules.pull_request = Some(p["required_approving_review_count"].as_u64().unwrap_or(0) as u32);
            }
            "required_status_checks" => {
                rules.checks =
                    Some(list(&p["required_status_checks"]).iter().map(|c| s(&c["context"])).collect::<Vec<_>>());
            }
            "non_fast_forward" => rules.block_force_push = true,
            "deletion" => rules.block_deletion = true,
            "required_signatures" => rules.signed_commits = true,
            "required_linear_history" => rules.linear_history = true,
            _ => {}
        }
    }
    Ruleset {
        id: v["id"].as_u64().unwrap_or(0),
        name: s(&v["name"]),
        target: if v["target"].as_str() == Some("tag") { Target::Tag } else { Target::Branch },
        enforcement: match v["enforcement"].as_str() {
            Some("active") => Enforcement::Active,
            Some("evaluate") => Enforcement::Evaluate,
            _ => Enforcement::Disabled,
        },
        include: list(&v["conditions"]["ref_name"]["include"])
            .iter()
            .map(|r| short_ref(r.as_str().unwrap_or("")))
            .collect(),
        rules,
        admins_bypass: list(&v["bypass_actors"])
            .iter()
            .any(|a| a["actor_type"].as_str() == Some("RepositoryRole") && a["actor_id"].as_u64() == Some(ADMIN_ROLE)),
    }
}

pub(crate) fn ruleset_body(r: &Ruleset) -> Value {
    let mut rules = Vec::new();
    let x = &r.rules;
    if let Some(n) = x.pull_request {
        rules.push(json!({ "type": "pull_request", "parameters": {
            "required_approving_review_count": n,
            "dismiss_stale_reviews_on_push": false,
            "require_code_owner_review": false,
            "require_last_push_approval": false,
            "required_review_thread_resolution": false,
        }}));
    }
    if let Some(checks) = &x.checks {
        let checks: Vec<Value> = checks.iter().map(|c| json!({ "context": c })).collect();
        rules.push(json!({ "type": "required_status_checks", "parameters": {
            "required_status_checks": checks,
            "strict_required_status_checks_policy": false,
        }}));
    }
    for (on, kind) in [
        (x.block_force_push, "non_fast_forward"),
        (x.block_deletion, "deletion"),
        (x.signed_commits, "required_signatures"),
        (x.linear_history, "required_linear_history"),
    ] {
        if on {
            rules.push(json!({ "type": kind }));
        }
    }
    let bypass: Vec<Value> = if r.admins_bypass {
        vec![json!({ "actor_id": ADMIN_ROLE, "actor_type": "RepositoryRole", "bypass_mode": "always" })]
    } else {
        Vec::new()
    };
    json!({
        "name": r.name,
        "target": if r.target == Target::Tag { "tag" } else { "branch" },
        "enforcement": match r.enforcement {
            Enforcement::Active => "active",
            Enforcement::Evaluate => "evaluate",
            Enforcement::Disabled => "disabled",
        },
        "conditions": { "ref_name": {
            "include": r.include.iter().map(|i| full_ref(r.target, i)).collect::<Vec<_>>(),
            "exclude": [],
        }},
        "rules": rules,
        "bypass_actors": bypass,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_set_survives_the_trip_to_github_and_back() {
        let mut r = Ruleset::new_for_default_branch();
        r.id = 7;
        r.include.push("release/*".to_owned());
        r.rules.checks = Some(vec!["build".to_owned(), "test".to_owned()]);
        r.rules.signed_commits = true;
        r.enforcement = Enforcement::Evaluate;
        let mut body = ruleset_body(&r);
        assert_eq!(body["conditions"]["ref_name"]["include"], json!(["~DEFAULT_BRANCH", "refs/heads/release/*"]));
        body["id"] = json!(7);
        assert_eq!(ruleset(&body), r);
    }

    #[test]
    fn a_message_choice_sets_both_of_githubs_fields() {
        assert_eq!(
            patch(&Change::SquashMessage(Message::TitleAndDescription)),
            json!({ "squash_merge_commit_title": "PR_TITLE", "squash_merge_commit_message": "PR_BODY" })
        );
        assert_eq!(
            patch(&Change::MergeMessage(Message::Default)),
            json!({ "merge_commit_title": "MERGE_MESSAGE", "merge_commit_message": "PR_TITLE" })
        );
        assert_eq!(patch(&Change::Visibility(Visibility::Public)), json!({ "visibility": "public" }));
    }
}
