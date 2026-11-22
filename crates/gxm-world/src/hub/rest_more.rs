//! The world's GitHub REST API for automation and security settings: Actions, secrets and
//! variables, environments, webhooks, Pages, security features, deploy keys and autolinks.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use crypto_box::SecretKey;
use jiff::Timestamp;
use serde_json::{Value, json};

use super::admin::{
    AutolinkData, DeployKeyData, EnvironmentData, HookData, PagesData, SecretData, Settings, VariableData,
};
use super::rest::{Answer, created, done, invalid, not_found, ok};

/// The key the world's repositories seal secrets with. Fixed, so a world is deterministic; it
/// only has to prove the app sealed for this key.
fn key() -> SecretKey {
    SecretKey::from([0x42; 32])
}

pub const KEY_ID: &str = "3380204578043523366";

#[expect(clippy::too_many_arguments, reason = "one REST call's whole context, passed down from the router")]
pub(super) fn answer(
    st: &mut Settings,
    site: &str,
    method: &str,
    rest: &[&str],
    body: &Value,
    now: Timestamp,
    next_id: u32,
    bump: &mut bool,
) -> Answer {
    let text = |k: &str| body[k].as_str().unwrap_or_default().to_owned();
    let stamp = |t: Timestamp| t.to_string();
    match (method, rest) {
        ("GET", ["actions", "permissions"]) => {
            ok(json!({ "enabled": st.actions_enabled, "allowed_actions": st.allowed_actions }))
        }
        ("PUT", ["actions", "permissions"]) => {
            st.actions_enabled = body["enabled"].as_bool().unwrap_or(true);
            if let Some(a) = body["allowed_actions"].as_str() {
                st.allowed_actions = a.to_owned();
            }
            done()
        }
        ("PUT", ["actions", "permissions", "selected-actions"]) => done(),
        ("GET", ["actions", "permissions", "workflow"]) => ok(json!({
            "default_workflow_permissions": st.workflow_permissions,
            "can_approve_pull_request_reviews": st.approve_reviews,
        })),
        ("PUT", ["actions", "permissions", "workflow"]) => {
            st.workflow_permissions = text("default_workflow_permissions");
            st.approve_reviews = body["can_approve_pull_request_reviews"].as_bool().unwrap_or(false);
            done()
        }
        ("GET", [scope, "secrets"]) => {
            let list = st.secrets_mut(scope);
            let secrets: Vec<Value> =
                list.iter().map(|x| json!({ "name": x.name, "updated_at": stamp(x.updated) })).collect();
            ok(json!({ "total_count": secrets.len(), "secrets": secrets }))
        }
        ("GET", [_, "secrets", "public-key"]) => {
            ok(json!({ "key_id": KEY_ID, "key": STANDARD.encode(key().public_key().as_bytes()) }))
        }
        ("PUT", [scope, "secrets", name]) => {
            if body["key_id"].as_str() != Some(KEY_ID) {
                return invalid("key_id doesn't match the repository's public key");
            }
            let opened = STANDARD.decode(text("encrypted_value")).ok().and_then(|sealed| key().unseal(&sealed).ok());
            let Some(value) = opened.and_then(|v| String::from_utf8(v).ok()) else {
                return invalid("encrypted_value isn't sealed with the repository's public key");
            };
            let list = st.secrets_mut(scope);
            let fresh = !list.iter().any(|x| x.name == *name);
            list.retain(|x| x.name != *name);
            list.push(SecretData { name: (*name).to_owned(), value, updated: now });
            list.sort_by(|a, b| a.name.cmp(&b.name));
            if fresh { created(json!({})) } else { done() }
        }
        ("DELETE", [scope, "secrets", name]) => {
            st.secrets_mut(scope).retain(|x| x.name != *name);
            done()
        }
        ("GET", ["actions", "variables"]) => {
            let vars: Vec<Value> = st
                .variables
                .iter()
                .map(|v| json!({ "name": v.name, "value": v.value, "updated_at": stamp(v.updated) }))
                .collect();
            ok(json!({ "total_count": vars.len(), "variables": vars }))
        }
        ("POST", ["actions", "variables"]) => {
            let name = text("name");
            if name.is_empty() || st.variables.iter().any(|v| v.name == name) {
                return invalid("Variable already exists");
            }
            st.variables.push(VariableData { name, value: text("value"), updated: now });
            st.variables.sort_by(|a, b| a.name.cmp(&b.name));
            created(json!({}))
        }
        ("PATCH", ["actions", "variables", name]) => match st.variables.iter_mut().find(|v| v.name == *name) {
            Some(v) => {
                v.value = text("value");
                v.updated = now;
                done()
            }
            None => not_found(),
        },
        ("DELETE", ["actions", "variables", name]) => {
            st.variables.retain(|v| v.name != *name);
            done()
        }
        ("GET", ["environments"]) => {
            let envs: Vec<Value> = st.environments.iter().map(environment).collect();
            ok(json!({ "total_count": envs.len(), "environments": envs }))
        }
        ("PUT", ["environments", name]) => {
            let reviewers = body["reviewers"]
                .as_array()
                .map(|a| a.iter().filter_map(|r| r["id"].as_u64().and_then(super::login_of)).collect())
                .unwrap_or_default();
            let e = EnvironmentData {
                name: (*name).to_owned(),
                wait_timer: body["wait_timer"].as_u64().unwrap_or(0),
                reviewers,
                protected_branches: body["deployment_branch_policy"]["protected_branches"].as_bool().unwrap_or(false),
            };
            match st.environments.iter_mut().find(|x| x.name == *name) {
                Some(x) => *x = e.clone(),
                None => st.environments.push(e.clone()),
            }
            ok(environment(&e))
        }
        ("DELETE", ["environments", name]) => {
            st.environments.retain(|e| e.name != *name);
            done()
        }
        ("GET", ["hooks"]) => ok(json!(st.hooks.iter().map(hook).collect::<Vec<_>>())),
        ("POST", ["hooks"]) => {
            let url = body["config"]["url"].as_str().unwrap_or_default().to_owned();
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return invalid("url is not a valid URL");
            }
            *bump = true;
            let h = HookData {
                id: next_id.into(),
                url,
                events: strings(&body["events"]),
                active: body["active"].as_bool().unwrap_or(true),
                secret: body["config"]["secret"].as_str().unwrap_or_default().to_owned(),
                last: None,
            };
            st.hooks.push(h.clone());
            created(hook(&h))
        }
        (_, ["hooks", id, more @ ..]) => {
            let Some(h) = st.hooks.iter_mut().find(|h| h.id.to_string() == *id) else { return not_found() };
            match (method, more) {
                ("PATCH", []) => {
                    if let Some(a) = body["active"].as_bool() {
                        h.active = a;
                    }
                    ok(hook(h))
                }
                ("POST", ["pings"]) => {
                    h.last = Some((200, "OK".to_owned()));
                    done()
                }
                ("DELETE", []) => {
                    let id = h.id;
                    st.hooks.retain(|h| h.id != id);
                    done()
                }
                _ => not_found(),
            }
        }
        ("GET", ["pages"]) => match &st.pages {
            Some(p) => ok(pages(p, site)),
            None => not_found(),
        },
        ("POST" | "PUT", ["pages"]) => {
            let mut p = st.pages.clone().unwrap_or(PagesData {
                build_type: "legacy".to_owned(),
                branch: st.default_branch.clone(),
                path: "/".to_owned(),
                cname: None,
                https_enforced: true,
            });
            if method == "POST" && st.pages.is_some() {
                return invalid("GitHub Pages is already enabled");
            }
            if method == "PUT" && st.pages.is_none() {
                return not_found();
            }
            if let Some(b) = body["build_type"].as_str() {
                p.build_type = b.to_owned();
            }
            if let Some(branch) = body["source"]["branch"].as_str() {
                p.branch = branch.to_owned();
                p.path = body["source"]["path"].as_str().unwrap_or("/").to_owned();
            }
            if body.get("cname").is_some() {
                p.cname = body["cname"].as_str().filter(|c| !c.is_empty()).map(str::to_owned);
            }
            if let Some(h) = body["https_enforced"].as_bool() {
                p.https_enforced = h;
            }
            st.pages = Some(p.clone());
            if method == "POST" { created(pages(&p, site)) } else { done() }
        }
        ("DELETE", ["pages"]) => {
            st.pages = None;
            done()
        }
        ("GET", ["vulnerability-alerts"]) if st.dependabot_alerts => done(),
        ("GET", ["vulnerability-alerts"]) => not_found(),
        ("PUT" | "DELETE", ["vulnerability-alerts"]) => {
            st.dependabot_alerts = method == "PUT";
            done()
        }
        ("GET", ["keys"]) => ok(json!(st.deploy_keys.iter().map(deploy_key).collect::<Vec<_>>())),
        ("POST", ["keys"]) => {
            let key = text("key");
            if !key.starts_with("ssh-") && !key.starts_with("ecdsa-") {
                return invalid("key is invalid. You must supply a key in OpenSSH public key format");
            }
            *bump = true;
            let k = DeployKeyData {
                id: next_id.into(),
                title: text("title"),
                key,
                read_only: body["read_only"].as_bool().unwrap_or(true),
                created: now,
            };
            st.deploy_keys.push(k.clone());
            created(deploy_key(&k))
        }
        ("DELETE", ["keys", id]) => {
            st.deploy_keys.retain(|k| k.id.to_string() != *id);
            done()
        }
        ("GET", ["autolinks"]) => ok(json!(st.autolinks.iter().map(autolink).collect::<Vec<_>>())),
        ("POST", ["autolinks"]) => {
            let template = text("url_template");
            if !template.contains("<num>") {
                return invalid("url_template must contain <num>");
            }
            *bump = true;
            let a = AutolinkData { id: next_id.into(), prefix: text("key_prefix"), template };
            st.autolinks.push(a.clone());
            created(autolink(&a))
        }
        ("DELETE", ["autolinks", id]) => {
            st.autolinks.retain(|a| a.id.to_string() != *id);
            done()
        }
        _ => not_found(),
    }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

fn environment(e: &EnvironmentData) -> Value {
    let mut rules = Vec::new();
    if e.wait_timer > 0 {
        rules.push(json!({ "type": "wait_timer", "wait_timer": e.wait_timer }));
    }
    if !e.reviewers.is_empty() {
        let reviewers: Vec<Value> = e
            .reviewers
            .iter()
            .map(|l| json!({ "type": "User", "reviewer": { "login": l, "id": super::user_id(l) } }))
            .collect();
        rules.push(json!({ "type": "required_reviewers", "reviewers": reviewers }));
    }
    let policy = if e.protected_branches {
        json!({ "protected_branches": true, "custom_branch_policies": false })
    } else {
        Value::Null
    };
    json!({ "name": e.name, "protection_rules": rules, "deployment_branch_policy": policy })
}

fn hook(h: &HookData) -> Value {
    let last = match &h.last {
        Some((code, message)) => json!({ "code": code, "status": "active", "message": message }),
        None => json!({ "code": null, "status": "unused", "message": null }),
    };
    json!({ "id": h.id, "name": "web", "active": h.active, "events": h.events, "config": { "url": h.url, "content_type": "json" }, "last_response": last })
}

/// Pages as GitHub returns it; `site` is where it's served without a custom domain.
fn pages(p: &PagesData, site: &str) -> Value {
    let url = match &p.cname {
        Some(c) => format!("https://{c}/"),
        None => site.to_owned(),
    };
    json!({
        "html_url": url,
        "build_type": p.build_type,
        "source": { "branch": p.branch, "path": p.path },
        "cname": p.cname,
        "https_enforced": p.https_enforced,
    })
}

fn deploy_key(k: &DeployKeyData) -> Value {
    json!({ "id": k.id, "title": k.title, "key": k.key, "read_only": k.read_only, "created_at": k.created.to_string() })
}

fn autolink(a: &AutolinkData) -> Value {
    json!({ "id": a.id, "key_prefix": a.prefix, "url_template": a.template, "is_alphanumeric": true })
}
