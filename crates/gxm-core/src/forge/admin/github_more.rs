//! GitHub's settings for automation and security, through its REST API: Actions, secrets and
//! variables, environments, webhooks, Pages, security features, deploy keys and autolinks.

use serde_json::{Value, json};

use super::github::{GitHub, list, s, when};
use super::model::*;
use super::{Page, Reading, Write, seal};
use crate::forge::ForgeError;

impl GitHub {
    pub(super) fn read_more(&self, page: Page) -> Result<Reading, ForgeError> {
        Ok(match page {
            Page::Actions => Reading::Actions(self.actions()?),
            Page::Secrets(scope) => {
                let v = self.get(&self.repo_path(&format!("/{}/secrets?per_page=100", scope.path())))?;
                let secrets =
                    list(&v["secrets"]).iter().map(|x| Secret { name: s(&x["name"]), updated: when(&x["updated_at"]) });
                Reading::Secrets(scope, secrets.collect())
            }
            Page::Variables => {
                let v = self.get(&self.repo_path("/actions/variables?per_page=30"))?;
                let vars = list(&v["variables"]).iter().map(|x| Variable {
                    name: s(&x["name"]),
                    value: s(&x["value"]),
                    updated: when(&x["updated_at"]),
                });
                Reading::Variables(vars.collect())
            }
            Page::Environments => {
                let v = self.get(&self.repo_path("/environments"))?;
                Reading::Environments(list(&v["environments"]).iter().map(environment).collect())
            }
            Page::Hooks => Reading::Hooks(list(&self.get(&self.repo_path("/hooks"))?).iter().map(hook).collect()),
            Page::Pages => Reading::Pages(match self.get(&self.repo_path("/pages")) {
                Ok(v) => Some(pages(&v)),
                Err(ForgeError::Status(404)) => None,
                Err(e) => return Err(e),
            }),
            Page::Security => Reading::Security(self.security()?),
            Page::DeployKeys => {
                let v = self.get(&self.repo_path("/keys"))?;
                let keys = list(&v).iter().map(|k| DeployKey {
                    id: k["id"].as_u64().unwrap_or(0),
                    title: s(&k["title"]),
                    key: s(&k["key"]),
                    read_only: k["read_only"].as_bool().unwrap_or(true),
                    added: when(&k["created_at"]),
                });
                Reading::DeployKeys(keys.collect())
            }
            Page::Autolinks => {
                let v = self.get(&self.repo_path("/autolinks"))?;
                let links = list(&v).iter().map(|a| Autolink {
                    id: a["id"].as_u64().unwrap_or(0),
                    prefix: s(&a["key_prefix"]),
                    template: s(&a["url_template"]),
                });
                Reading::Autolinks(links.collect())
            }
            Page::Repo | Page::People | Page::Rules => {
                return Err(ForgeError::Api(format!("{page:?} is read elsewhere")));
            }
        })
    }

    pub(super) fn write_more(&self, w: &Write) -> Result<(), ForgeError> {
        let repo = |rest: &str| self.repo_path(rest);
        match w {
            Write::Actions(a) => {
                let mut body = json!({ "enabled": a.enabled });
                if a.enabled {
                    body["allowed_actions"] = json!(match a.allowed {
                        Allowed::All => "all",
                        Allowed::Local => "local_only",
                        Allowed::Selected => "selected",
                    });
                }
                self.call("PUT", &repo("/actions/permissions"), Some(body))?;
                if a.enabled && a.allowed == Allowed::Selected {
                    let selected =
                        json!({ "github_owned_allowed": true, "verified_allowed": true, "patterns_allowed": [] });
                    self.call("PUT", &repo("/actions/permissions/selected-actions"), Some(selected))?;
                }
                let workflow = json!({
                    "default_workflow_permissions": if a.write { "write" } else { "read" },
                    "can_approve_pull_request_reviews": a.approve_reviews,
                });
                self.call("PUT", &repo("/actions/permissions/workflow"), Some(workflow)).map(drop)
            }
            Write::PutSecret { scope, name, value } => {
                let key = self.get(&repo(&format!("/{}/secrets/public-key", scope.path())))?;
                let sealed = seal::seal(key["key"].as_str().unwrap_or_default(), value).map_err(ForgeError::Api)?;
                let body = json!({ "encrypted_value": sealed, "key_id": key["key_id"] });
                self.call("PUT", &repo(&format!("/{}/secrets/{name}", scope.path())), Some(body)).map(drop)
            }
            Write::DeleteSecret { scope, name } => {
                self.call("DELETE", &repo(&format!("/{}/secrets/{name}", scope.path())), None).map(drop)
            }
            Write::PutVariable { name, value, new: true } => {
                self.call("POST", &repo("/actions/variables"), Some(json!({ "name": name, "value": value }))).map(drop)
            }
            Write::PutVariable { name, value, new: false } => {
                let body = json!({ "name": name, "value": value });
                self.call("PATCH", &repo(&format!("/actions/variables/{name}")), Some(body)).map(drop)
            }
            Write::DeleteVariable(name) => {
                self.call("DELETE", &repo(&format!("/actions/variables/{name}")), None).map(drop)
            }
            Write::SaveEnvironment(e) => {
                let mut reviewers = Vec::new();
                for login in &e.reviewers {
                    let id = self.get(&format!("/users/{login}"))?["id"].as_u64().unwrap_or(0);
                    reviewers.push(json!({ "type": "User", "id": id }));
                }
                let policy = if e.protected_only {
                    json!({ "protected_branches": true, "custom_branch_policies": false })
                } else {
                    Value::Null
                };
                let body =
                    json!({ "wait_timer": e.wait_minutes, "reviewers": reviewers, "deployment_branch_policy": policy });
                self.call("PUT", &repo(&format!("/environments/{}", e.name)), Some(body)).map(drop)
            }
            Write::DeleteEnvironment(name) => {
                self.call("DELETE", &repo(&format!("/environments/{name}")), None).map(drop)
            }
            Write::AddHook(h) => {
                let body = json!({
                    "name": "web",
                    "active": true,
                    "events": h.events,
                    "config": { "url": h.url, "content_type": "json", "secret": h.secret, "insecure_ssl": "0" },
                });
                self.call("POST", &repo("/hooks"), Some(body)).map(drop)
            }
            Write::HookActive { id, active } => {
                self.call("PATCH", &repo(&format!("/hooks/{id}")), Some(json!({ "active": active }))).map(drop)
            }
            Write::PingHook(id) => self.call("POST", &repo(&format!("/hooks/{id}/pings")), None).map(drop),
            Write::DeleteHook(id) => self.call("DELETE", &repo(&format!("/hooks/{id}")), None).map(drop),
            Write::PagesSource(None) => self.call("DELETE", &repo("/pages"), None).map(drop),
            Write::PagesSource(Some(source)) => {
                let body = match source {
                    PagesSource::Workflow => json!({ "build_type": "workflow" }),
                    PagesSource::Branch { branch, docs } => json!({
                        "build_type": "legacy",
                        "source": { "branch": branch, "path": if *docs { "/docs" } else { "/" } },
                    }),
                };
                // Publishing starts with a POST; changing where from is a PUT.
                match self.get(&repo("/pages")) {
                    Ok(_) => self.call("PUT", &repo("/pages"), Some(body)).map(drop),
                    Err(ForgeError::Status(404)) => self.call("POST", &repo("/pages"), Some(body)).map(drop),
                    Err(e) => Err(e),
                }
            }
            Write::PagesDomain { cname, https } => {
                let body = json!({ "cname": cname, "https_enforced": https });
                self.call("PUT", &repo("/pages"), Some(body)).map(drop)
            }
            Write::Security(SecurityFeature::DependabotAlerts, on) => {
                self.call(if *on { "PUT" } else { "DELETE" }, &repo("/vulnerability-alerts"), None).map(drop)
            }
            Write::Security(feature, on) => {
                let field = match feature {
                    SecurityFeature::DependabotUpdates => "dependabot_security_updates",
                    SecurityFeature::SecretScanning => "secret_scanning",
                    SecurityFeature::PushProtection => "secret_scanning_push_protection",
                    SecurityFeature::DependabotAlerts => "",
                };
                let status = if *on { "enabled" } else { "disabled" };
                let body = json!({ "security_and_analysis": { field: { "status": status } } });
                self.call("PATCH", &repo(""), Some(body)).map(drop)
            }
            Write::AddDeployKey { title, key, read_only } => {
                let body = json!({ "title": title, "key": key, "read_only": read_only });
                self.call("POST", &repo("/keys"), Some(body)).map(drop)
            }
            Write::DeleteDeployKey(id) => self.call("DELETE", &repo(&format!("/keys/{id}")), None).map(drop),
            Write::AddAutolink { prefix, template } => {
                let body = json!({ "key_prefix": prefix, "url_template": template, "is_alphanumeric": true });
                self.call("POST", &repo("/autolinks"), Some(body)).map(drop)
            }
            Write::DeleteAutolink(id) => self.call("DELETE", &repo(&format!("/autolinks/{id}")), None).map(drop),
            other => Err(ForgeError::Api(format!("{other:?} is written elsewhere"))),
        }
    }

    fn actions(&self) -> Result<Actions, ForgeError> {
        let p = self.get(&self.repo_path("/actions/permissions"))?;
        let w = self.get(&self.repo_path("/actions/permissions/workflow"))?;
        Ok(Actions {
            enabled: p["enabled"].as_bool().unwrap_or(false),
            allowed: match p["allowed_actions"].as_str() {
                Some("local_only") => Allowed::Local,
                Some("selected") => Allowed::Selected,
                _ => Allowed::All,
            },
            write: w["default_workflow_permissions"].as_str() == Some("write"),
            approve_reviews: w["can_approve_pull_request_reviews"].as_bool().unwrap_or(false),
        })
    }

    fn security(&self) -> Result<Security, ForgeError> {
        let repo = self.get(&self.repo_path(""))?;
        let sa = &repo["security_and_analysis"];
        let on = |k: &str| sa[k]["status"].as_str() == Some("enabled");
        let dependabot_alerts = match self.get(&self.repo_path("/vulnerability-alerts")) {
            Ok(_) => true,
            Err(ForgeError::Status(404)) => false,
            Err(e) => return Err(e),
        };
        Ok(Security {
            dependabot_alerts,
            dependabot_updates: on("dependabot_security_updates"),
            secret_scanning: on("secret_scanning"),
            push_protection: on("secret_scanning_push_protection"),
        })
    }
}

fn environment(e: &Value) -> Environment {
    let mut env = Environment {
        name: s(&e["name"]),
        wait_minutes: 0,
        reviewers: Vec::new(),
        protected_only: e["deployment_branch_policy"]["protected_branches"].as_bool().unwrap_or(false),
    };
    for rule in list(&e["protection_rules"]) {
        match rule["type"].as_str() {
            Some("wait_timer") => env.wait_minutes = rule["wait_timer"].as_u64().unwrap_or(0) as u32,
            Some("required_reviewers") => {
                env.reviewers = list(&rule["reviewers"]).iter().map(|r| s(&r["reviewer"]["login"])).collect();
            }
            _ => {}
        }
    }
    env
}

fn hook(h: &Value) -> Hook {
    let last = &h["last_response"];
    Hook {
        id: h["id"].as_u64().unwrap_or(0),
        url: s(&h["config"]["url"]),
        events: list(&h["events"]).iter().map(s).collect(),
        active: h["active"].as_bool().unwrap_or(false),
        last: last["code"]
            .as_u64()
            .map(|code| format!("{code} {}", last["message"].as_str().unwrap_or_default()).trim().to_owned()),
    }
}

fn pages(v: &Value) -> Pages {
    let source = if v["build_type"].as_str() == Some("workflow") {
        PagesSource::Workflow
    } else {
        PagesSource::Branch { branch: s(&v["source"]["branch"]), docs: v["source"]["path"].as_str() == Some("/docs") }
    };
    Pages {
        url: s(&v["html_url"]),
        source,
        cname: v["cname"].as_str().filter(|c| !c.is_empty()).map(str::to_owned),
        https: v["https_enforced"].as_bool().unwrap_or(false),
    }
}
