//! A repository's settings on the world's GitHub: everything its settings pages hold, as the
//! REST API ([`super::rest`]) reads and changes it, kept where journeys can check it.

use jiff::Timestamp;
use serde_json::{Value, json};

/// One repository's settings. Fields use GitHub's own words and values (`"push"`, `"legacy"`).
#[derive(Clone, Debug)]
pub struct Settings {
    pub owner_is_org: bool,
    pub description: String,
    pub homepage: String,
    pub topics: Vec<String>,
    /// `private` or `public`.
    pub visibility: String,
    pub archived: bool,
    pub is_template: bool,
    pub default_branch: String,
    pub has_issues: bool,
    pub has_projects: bool,
    pub has_wiki: bool,
    pub has_discussions: bool,
    pub allow_merge_commit: bool,
    pub allow_squash_merge: bool,
    pub allow_rebase_merge: bool,
    pub allow_auto_merge: bool,
    pub allow_update_branch: bool,
    pub delete_branch_on_merge: bool,
    pub web_commit_signoff_required: bool,
    pub squash_merge_commit_title: String,
    pub squash_merge_commit_message: String,
    pub merge_commit_title: String,
    pub merge_commit_message: String,
    /// Login and role name (`admin`, `maintain`, `write`, `triage`, `read`).
    pub collaborators: Vec<(String, String)>,
    pub invitations: Vec<InvitationData>,
    /// Slug, name and permission (`pull`, `triage`, `push`, `maintain`, `admin`).
    pub teams: Vec<(String, String, String)>,
    /// The owning organisation's base role, if it has one.
    pub base_role: Option<String>,
    /// Rule sets as GitHub returns them, each with its `id`.
    pub rulesets: Vec<Value>,
    pub actions_enabled: bool,
    /// `all`, `local_only` or `selected`.
    pub allowed_actions: String,
    /// `read` or `write`.
    pub workflow_permissions: String,
    pub approve_reviews: bool,
    /// Scope (`actions`, `dependabot`, `codespaces`) and its secrets, their values unsealed.
    pub secrets: Vec<(String, Vec<SecretData>)>,
    pub variables: Vec<VariableData>,
    pub environments: Vec<EnvironmentData>,
    pub hooks: Vec<HookData>,
    pub pages: Option<PagesData>,
    pub dependabot_alerts: bool,
    pub dependabot_updates: bool,
    pub secret_scanning: bool,
    pub push_protection: bool,
    pub deploy_keys: Vec<DeployKeyData>,
    pub autolinks: Vec<AutolinkData>,
    /// Who it was transferred to.
    pub transferred_to: Option<String>,
    pub deleted: bool,
}

#[derive(Clone, Debug)]
pub struct InvitationData {
    pub id: u64,
    pub login: String,
    /// `read`, `triage`, `write`, `maintain` or `admin`.
    pub permission: String,
    pub created: Timestamp,
}

#[derive(Clone, Debug)]
pub struct SecretData {
    pub name: String,
    /// What the app sealed, unsealed with the hub's key: proof it was sealed for this repository.
    pub value: String,
    pub updated: Timestamp,
}

#[derive(Clone, Debug)]
pub struct VariableData {
    pub name: String,
    pub value: String,
    pub updated: Timestamp,
}

#[derive(Clone, Debug)]
pub struct EnvironmentData {
    pub name: String,
    pub wait_timer: u64,
    pub reviewers: Vec<String>,
    pub protected_branches: bool,
}

#[derive(Clone, Debug)]
pub struct HookData {
    pub id: u64,
    pub url: String,
    pub events: Vec<String>,
    pub active: bool,
    pub secret: String,
    /// The last delivery's code and message.
    pub last: Option<(u16, String)>,
}

#[derive(Clone, Debug)]
pub struct PagesData {
    /// `workflow` or `legacy` (a branch).
    pub build_type: String,
    pub branch: String,
    /// `/` or `/docs`.
    pub path: String,
    pub cname: Option<String>,
    pub https_enforced: bool,
}

#[derive(Clone, Debug)]
pub struct DeployKeyData {
    pub id: u64,
    pub title: String,
    pub key: String,
    pub read_only: bool,
    pub created: Timestamp,
}

#[derive(Clone, Debug)]
pub struct AutolinkData {
    pub id: u64,
    pub prefix: String,
    pub template: String,
}

impl Settings {
    /// A repository as GitHub makes one, with `admin` its only collaborator.
    pub fn new(admin: &str) -> Settings {
        Settings {
            owner_is_org: false,
            description: String::new(),
            homepage: String::new(),
            topics: Vec::new(),
            visibility: "private".to_owned(),
            archived: false,
            is_template: false,
            default_branch: "main".to_owned(),
            has_issues: true,
            has_projects: true,
            has_wiki: true,
            has_discussions: false,
            allow_merge_commit: true,
            allow_squash_merge: true,
            allow_rebase_merge: true,
            allow_auto_merge: false,
            allow_update_branch: false,
            delete_branch_on_merge: false,
            web_commit_signoff_required: false,
            squash_merge_commit_title: "COMMIT_OR_PR_TITLE".to_owned(),
            squash_merge_commit_message: "COMMIT_MESSAGES".to_owned(),
            merge_commit_title: "MERGE_MESSAGE".to_owned(),
            merge_commit_message: "PR_TITLE".to_owned(),
            collaborators: vec![(admin.to_owned(), "admin".to_owned())],
            invitations: Vec::new(),
            teams: Vec::new(),
            base_role: None,
            rulesets: Vec::new(),
            actions_enabled: true,
            allowed_actions: "all".to_owned(),
            workflow_permissions: "read".to_owned(),
            approve_reviews: false,
            secrets: Vec::new(),
            variables: Vec::new(),
            environments: Vec::new(),
            hooks: Vec::new(),
            pages: None,
            dependabot_alerts: false,
            dependabot_updates: false,
            secret_scanning: false,
            push_protection: false,
            deploy_keys: Vec::new(),
            autolinks: Vec::new(),
            transferred_to: None,
            deleted: false,
        }
    }

    /// `login`'s role name here: `admin`, `write`, `read`…
    pub fn role_of(&self, login: &str) -> &str {
        self.collaborators.iter().find(|(l, _)| l == login).map_or("read", |(_, r)| r.as_str())
    }

    /// The secrets of `scope`, made if it has none yet.
    pub fn secrets_mut(&mut self, scope: &str) -> &mut Vec<SecretData> {
        let i = match self.secrets.iter().position(|(s, _)| s == scope) {
            Some(i) => i,
            None => {
                self.secrets.push((scope.to_owned(), Vec::new()));
                self.secrets.len() - 1
            }
        };
        &mut self.secrets[i].1
    }

    /// A secret's unsealed value, for checks.
    pub fn secret(&self, scope: &str, name: &str) -> Option<&str> {
        let list = &self.secrets.iter().find(|(s, _)| s == scope)?.1;
        list.iter().find(|x| x.name == name).map(|x| x.value.as_str())
    }

    /// The repository as `GET /repos/{owner}/{repo}` returns it, for `viewer`.
    pub(super) fn repo_json(&self, owner: &str, name: &str, viewer: &str) -> Value {
        let role = self.role_of(viewer);
        let at_least = |r: &str| {
            let rank = |x: &str| ["read", "triage", "write", "maintain", "admin"].iter().position(|y| *y == x);
            rank(role) >= rank(r)
        };
        let status = |on: bool| json!({ "status": if on { "enabled" } else { "disabled" } });
        json!({
            "name": name,
            "full_name": format!("{owner}/{name}"),
            "owner": { "login": owner, "type": if self.owner_is_org { "Organization" } else { "User" } },
            "description": if self.description.is_empty() { Value::Null } else { json!(self.description) },
            "homepage": self.homepage,
            "topics": self.topics,
            "private": self.visibility == "private",
            "visibility": self.visibility,
            "archived": self.archived,
            "is_template": self.is_template,
            "default_branch": self.default_branch,
            "has_issues": self.has_issues,
            "has_projects": self.has_projects,
            "has_wiki": self.has_wiki,
            "has_discussions": self.has_discussions,
            "allow_merge_commit": self.allow_merge_commit,
            "allow_squash_merge": self.allow_squash_merge,
            "allow_rebase_merge": self.allow_rebase_merge,
            "allow_auto_merge": self.allow_auto_merge,
            "allow_update_branch": self.allow_update_branch,
            "delete_branch_on_merge": self.delete_branch_on_merge,
            "web_commit_signoff_required": self.web_commit_signoff_required,
            "squash_merge_commit_title": self.squash_merge_commit_title,
            "squash_merge_commit_message": self.squash_merge_commit_message,
            "merge_commit_title": self.merge_commit_title,
            "merge_commit_message": self.merge_commit_message,
            "permissions": {
                "admin": at_least("admin"),
                "maintain": at_least("maintain"),
                "push": at_least("write"),
                "triage": at_least("triage"),
                "pull": true,
            },
            "security_and_analysis": {
                "secret_scanning": status(self.secret_scanning),
                "secret_scanning_push_protection": status(self.push_protection),
                "dependabot_security_updates": status(self.dependabot_updates),
            },
        })
    }

    /// Applies a `PATCH /repos/{owner}/{repo}` body.
    pub(super) fn patch(&mut self, body: &Value) {
        let Some(fields) = body.as_object() else { return };
        for (k, v) in fields {
            let text = || v.as_str().unwrap_or_default().to_owned();
            let on = v.as_bool().unwrap_or(false);
            match k.as_str() {
                "description" => self.description = text(),
                "homepage" => self.homepage = text(),
                "visibility" => self.visibility = text(),
                "private" => self.visibility = if on { "private" } else { "public" }.to_owned(),
                "archived" => self.archived = on,
                "is_template" => self.is_template = on,
                "default_branch" => self.default_branch = text(),
                "has_issues" => self.has_issues = on,
                "has_projects" => self.has_projects = on,
                "has_wiki" => self.has_wiki = on,
                "has_discussions" => self.has_discussions = on,
                "allow_merge_commit" => self.allow_merge_commit = on,
                "allow_squash_merge" => self.allow_squash_merge = on,
                "allow_rebase_merge" => self.allow_rebase_merge = on,
                "allow_auto_merge" => self.allow_auto_merge = on,
                "allow_update_branch" => self.allow_update_branch = on,
                "delete_branch_on_merge" => self.delete_branch_on_merge = on,
                "web_commit_signoff_required" => self.web_commit_signoff_required = on,
                "squash_merge_commit_title" => self.squash_merge_commit_title = text(),
                "squash_merge_commit_message" => self.squash_merge_commit_message = text(),
                "merge_commit_title" => self.merge_commit_title = text(),
                "merge_commit_message" => self.merge_commit_message = text(),
                "security_and_analysis" => {
                    let enabled = |f: &str| v[f]["status"].as_str().map(|s| s == "enabled");
                    if let Some(x) = enabled("secret_scanning") {
                        self.secret_scanning = x;
                    }
                    if let Some(x) = enabled("secret_scanning_push_protection") {
                        self.push_protection = x;
                    }
                    if let Some(x) = enabled("dependabot_security_updates") {
                        self.dependabot_updates = x;
                    }
                }
                _ => {}
            }
        }
    }
}
