//! Pull requests and reviews on the provider a repository lives on (a "forge"): GitHub first,
//! with GitLab and Bitbucket behind the same [`Forge`] trait.
//!
//! The provider is asked only for what git can't know: which pull requests exist, their
//! reviews, checks and threads, and sending a review. What a pull request changes (its commits,
//! files and diff) comes from git, from the commits already in the repository ([`local`]).
//!
//! Every call blocks on the network, so the app makes them off the UI thread. Answers are
//! cached on disk ([`cache`]), so a pull request opens at once and refreshes behind the scenes.

pub mod cache;
pub mod drafts;
pub(crate) mod github;
pub(crate) mod http;
pub mod local;
pub mod model;

use std::sync::Arc;

use crate::env::Env;
use crate::remote::{Provider, Remote};
use model::{Pull, PullSummary, Submission, User};

/// A provider's API for one repository.
pub trait Forge: Send + Sync {
    fn provider(&self) -> Provider;
    /// Who the token belongs to.
    fn viewer(&self) -> Result<User, ForgeError>;
    /// Open pull requests and recently merged ones, newest first.
    fn pulls(&self) -> Result<Vec<PullSummary>, ForgeError>;
    /// One pull request in full.
    fn pull(&self, number: u64) -> Result<Pull, ForgeError>;
    /// Sends a review of `pull`, comments and replies included, as one review.
    fn submit(&self, pull: &Pull, review: &Submission) -> Result<(), ForgeError>;
    /// Squashes `pull` into its base as one commit with this message, provided its branch is
    /// still at the commit the person saw.
    fn squash_merge(&self, pull: &Pull, headline: &str) -> Result<(), ForgeError>;
}

/// Why a provider call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForgeError {
    /// The provider couldn't be reached.
    Network(String),
    /// It answered with an HTTP error.
    Status(u16),
    /// It answered, but refused or failed the request, saying why.
    Api(String),
    /// It answered with something we don't understand.
    Parse(String),
}

impl std::fmt::Display for ForgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForgeError::Network(e) => write!(f, "couldn't reach it: {e}"),
            ForgeError::Status(401) => write!(f, "it refused the token (401)"),
            ForgeError::Status(403) => write!(f, "the token isn't allowed to do that (403)"),
            ForgeError::Status(404) => write!(f, "it doesn't know this repository (404)"),
            ForgeError::Status(code) => write!(f, "it answered {code}"),
            ForgeError::Api(e) => write!(f, "{e}"),
            ForgeError::Parse(e) => write!(f, "its answer didn't make sense: {e}"),
        }
    }
}

impl std::error::Error for ForgeError {}

/// Why a repository's pull requests can't be shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotConnected {
    /// No remote, or a host that isn't a known provider.
    NoProvider,
    /// A provider this version doesn't talk to yet.
    NotYet(Provider),
    /// The provider needs a token and there isn't one.
    NoToken(Provider),
}

impl NotConnected {
    /// What the app says about it.
    pub fn message(&self) -> String {
        match self {
            NotConnected::NoProvider => "This repository has no remote on GitHub, GitLab or Bitbucket.".to_owned(),
            NotConnected::NotYet(p) => format!("Pull requests on {} are coming next.", p.name()),
            NotConnected::NoToken(p) => format!("Sign in to {} to see pull requests.", p.name()),
        }
    }
}

/// Where a repository's pull requests live, for caching and drafts: `github.com/studio/atlas-web`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RepoKey {
    pub host: String,
    pub owner: String,
    pub name: String,
}

impl RepoKey {
    pub fn of(remote: &Remote) -> RepoKey {
        RepoKey { host: remote.host.clone(), owner: remote.owner.clone(), name: remote.name.clone() }
    }
}

/// The provider for a repository whose `origin` is `remote`, if it can be reached.
pub fn connect(env: &Env, remote: &Remote) -> Result<Arc<dyn Forge>, NotConnected> {
    match remote.provider {
        Provider::GitHub => {
            let token = env.forge.github_token.clone().ok_or(NotConnected::NoToken(Provider::GitHub))?;
            // github.com (and its ssh aliases) use the public API; GitHub Enterprise serves its
            // own under /api.
            let api = if remote.host.starts_with("github.com") {
                env.forge.github_api.clone()
            } else {
                format!("https://{}/api", remote.host)
            };
            Ok(Arc::new(github::GitHub::new(&api, token, remote.owner.clone(), remote.name.clone())))
        }
        Provider::GitLab | Provider::Bitbucket => Err(NotConnected::NotYet(remote.provider)),
        Provider::Other => Err(NotConnected::NoProvider),
    }
}
