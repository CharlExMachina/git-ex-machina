//! A repository's settings on its provider: everything its settings pages hold, read and
//! changed from here, so nobody needs the website for them. GitHub first, through its REST API.
//!
//! Reads come a page at a time ([`Page`]); every change is one [`Write`]. The app re-reads a
//! page after each write, so it always shows what the provider holds.

mod github;
mod github_more;
pub mod model;
pub mod seal;

use std::sync::Arc;

pub use model::*;

use super::{ForgeError, NotConnected};
use crate::env::Env;
use crate::remote::{Provider, Remote};

/// What a settings page reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Repo,
    People,
    Rules,
    Actions,
    Secrets(Scope),
    Variables,
    Environments,
    Hooks,
    Pages,
    Security,
    DeployKeys,
    Autolinks,
}

/// What a page read found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reading {
    Repo(Box<Repo>),
    People(People),
    Rules(Vec<Ruleset>),
    Actions(Actions),
    Secrets(Scope, Vec<Secret>),
    Variables(Vec<Variable>),
    Environments(Vec<Environment>),
    Hooks(Vec<Hook>),
    /// None when the repository doesn't publish a site.
    Pages(Option<Pages>),
    Security(Security),
    DeployKeys(Vec<DeployKey>),
    Autolinks(Vec<Autolink>),
}

/// One change to the repository's settings.
#[derive(Clone, PartialEq, Eq)]
pub enum Write {
    Change(Change),
    Topics(Vec<String>),
    Invite {
        login: String,
        role: Role,
    },
    SetRole {
        who: Grantee,
        role: Role,
    },
    Remove(Grantee),
    CancelInvite(u64),
    SaveRuleset(Box<Ruleset>),
    DeleteRuleset(u64),
    Actions(Actions),
    PutSecret {
        scope: Scope,
        name: String,
        value: String,
    },
    DeleteSecret {
        scope: Scope,
        name: String,
    },
    PutVariable {
        name: String,
        value: String,
        new: bool,
    },
    DeleteVariable(String),
    SaveEnvironment(Environment),
    DeleteEnvironment(String),
    AddHook(NewHook),
    HookActive {
        id: u64,
        active: bool,
    },
    PingHook(u64),
    DeleteHook(u64),
    /// Publishes from this source, or stops publishing with None.
    PagesSource(Option<PagesSource>),
    PagesDomain {
        cname: Option<String>,
        https: bool,
    },
    Security(SecurityFeature, bool),
    AddDeployKey {
        title: String,
        key: String,
        read_only: bool,
    },
    DeleteDeployKey(u64),
    AddAutolink {
        prefix: String,
        template: String,
    },
    DeleteAutolink(u64),
    Transfer(String),
    Delete,
}

impl std::fmt::Debug for Write {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Never print a secret's value.
            Write::PutSecret { scope, name, .. } => {
                f.debug_struct("PutSecret").field("scope", scope).field("name", name).finish_non_exhaustive()
            }
            Write::Change(c) => f.debug_tuple("Change").field(c).finish(),
            other => f.write_str(other.name()),
        }
    }
}

impl Write {
    /// The page whose reading the write changes, read again once it lands.
    pub fn page(&self) -> Page {
        match self {
            Write::Change(_) | Write::Topics(_) | Write::Transfer(_) | Write::Delete => Page::Repo,
            Write::Invite { .. } | Write::SetRole { .. } | Write::Remove(_) | Write::CancelInvite(_) => Page::People,
            Write::SaveRuleset(_) | Write::DeleteRuleset(_) => Page::Rules,
            Write::Actions(_) => Page::Actions,
            Write::PutSecret { scope, .. } | Write::DeleteSecret { scope, .. } => Page::Secrets(*scope),
            Write::PutVariable { .. } | Write::DeleteVariable(_) => Page::Variables,
            Write::SaveEnvironment(_) | Write::DeleteEnvironment(_) => Page::Environments,
            Write::AddHook(_) | Write::HookActive { .. } | Write::PingHook(_) | Write::DeleteHook(_) => Page::Hooks,
            Write::PagesSource(_) | Write::PagesDomain { .. } => Page::Pages,
            Write::Security(..) => Page::Security,
            Write::AddDeployKey { .. } | Write::DeleteDeployKey(_) => Page::DeployKeys,
            Write::AddAutolink { .. } | Write::DeleteAutolink(_) => Page::Autolinks,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Write::Change(_) => "Change",
            Write::Topics(_) => "Topics",
            Write::Invite { .. } => "Invite",
            Write::SetRole { .. } => "SetRole",
            Write::Remove(_) => "Remove",
            Write::CancelInvite(_) => "CancelInvite",
            Write::SaveRuleset(_) => "SaveRuleset",
            Write::DeleteRuleset(_) => "DeleteRuleset",
            Write::Actions(_) => "Actions",
            Write::PutSecret { .. } => "PutSecret",
            Write::DeleteSecret { .. } => "DeleteSecret",
            Write::PutVariable { .. } => "PutVariable",
            Write::DeleteVariable(_) => "DeleteVariable",
            Write::SaveEnvironment(_) => "SaveEnvironment",
            Write::DeleteEnvironment(_) => "DeleteEnvironment",
            Write::AddHook(_) => "AddHook",
            Write::HookActive { .. } => "HookActive",
            Write::PingHook(_) => "PingHook",
            Write::DeleteHook(_) => "DeleteHook",
            Write::PagesSource(_) => "PagesSource",
            Write::PagesDomain { .. } => "PagesDomain",
            Write::Security(..) => "Security",
            Write::AddDeployKey { .. } => "AddDeployKey",
            Write::DeleteDeployKey(_) => "DeleteDeployKey",
            Write::AddAutolink { .. } => "AddAutolink",
            Write::DeleteAutolink(_) => "DeleteAutolink",
            Write::Transfer(_) => "Transfer",
            Write::Delete => "Delete",
        }
    }
}

/// A provider's settings API for one repository. Every call blocks on the network.
pub trait Admin: Send + Sync {
    fn provider(&self) -> Provider;
    /// `owner/name`, as the provider knows the repository.
    fn full_name(&self) -> String;
    fn read(&self, page: Page) -> Result<Reading, ForgeError>;
    fn write(&self, write: &Write) -> Result<(), ForgeError>;
}

/// The settings API of the repository whose `origin` is `remote`, with `token`.
pub fn connect(env: &Env, remote: &Remote) -> Result<Arc<dyn Admin>, NotConnected> {
    match remote.provider {
        Provider::GitHub => {
            let token = env.forge.github_token.clone().ok_or(NotConnected::NoToken(Provider::GitHub))?;
            let api = if remote.host.starts_with("github.com") {
                env.forge.github_api.clone()
            } else {
                format!("https://{}/api/v3", remote.host)
            };
            Ok(Arc::new(github::GitHub::new(&api, token, &remote.owner, &remote.name)))
        }
        Provider::GitLab | Provider::Bitbucket => Err(NotConnected::NotYet(remote.provider)),
        Provider::Other => Err(NotConnected::NoProvider),
    }
}
