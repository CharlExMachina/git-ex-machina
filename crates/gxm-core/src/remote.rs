//! Remote URLs: which host and provider a remote lives on, who owns it, and over which protocol.
//!
//! SSH is the default everywhere; HTTPS is something a person opts into.

/// A parsed remote URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub protocol: Protocol,
    /// `github.com`, `gitlab.example.com`. An `ssh` config alias (`github.com-work`) stays as written.
    pub host: String,
    /// The path before the repository name: `studio`, or `group/subgroup` on GitLab.
    pub owner: String,
    /// The repository name without `.git`.
    pub name: String,
    pub provider: Provider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Ssh,
    Https,
    File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    GitHub,
    GitLab,
    Bitbucket,
    Other,
}

impl Provider {
    /// Recognises the big three by host, including `ssh` config aliases that start with their
    /// host (`github.com-work`) and self-hosted GitLab (`gitlab.example.com`).
    pub fn from_host(host: &str) -> Provider {
        let h = host.to_ascii_lowercase();
        if h.starts_with("github.com") || h.ends_with(".github.com") {
            Provider::GitHub
        } else if h.starts_with("gitlab.") || h.contains(".gitlab.") || h == "gitlab" {
            Provider::GitLab
        } else if h.starts_with("bitbucket.org") || h.starts_with("bitbucket.") {
            Provider::Bitbucket
        } else {
            Provider::Other
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Provider::GitHub => "GitHub",
            Provider::GitLab => "GitLab",
            Provider::Bitbucket => "Bitbucket",
            Provider::Other => "Git",
        }
    }
}

impl Remote {
    /// Parses scp-like SSH (`git@github.com:studio/atlas-web.git`), `ssh://`, `https://` and
    /// `file://` URLs. None for anything else.
    pub fn parse(url: &str) -> Option<Remote> {
        let url = url.trim();
        let (protocol, host, path) = if let Some(rest) = url.strip_prefix("ssh://") {
            let (authority, path) = rest.split_once('/')?;
            (Protocol::Ssh, host_of(authority), path)
        } else if let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) {
            let (authority, path) = rest.split_once('/')?;
            (Protocol::Https, host_of(authority), path)
        } else if let Some(path) = url.strip_prefix("file://") {
            (Protocol::File, String::new(), path)
        } else if let Some((authority, path)) = url.split_once(':').filter(|(a, _)| !a.contains('/')) {
            (Protocol::Ssh, host_of(authority), path)
        } else {
            return None;
        };
        let path = path.trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        let (owner, name) = path.rsplit_once('/').unwrap_or(("", path));
        if name.is_empty() {
            return None;
        }
        let owner = if protocol == Protocol::File { owner.rsplit('/').next().unwrap_or("") } else { owner };
        Some(Remote {
            provider: Provider::from_host(&host),
            protocol,
            host,
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }

    /// The same repository over SSH: `git@host:owner/name.git`.
    pub fn ssh_url(&self) -> String {
        format!("git@{}:{}.git", self.host, self.repo_path())
    }

    /// The same repository over HTTPS, for people who opt in.
    pub fn https_url(&self) -> String {
        format!("https://{}/{}.git", self.host, self.repo_path())
    }

    fn repo_path(&self) -> String {
        if self.owner.is_empty() { self.name.clone() } else { format!("{}/{}", self.owner, self.name) }
    }
}

/// `user@host:port` → `host`.
fn host_of(authority: &str) -> String {
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.split_once(':').map_or(host, |(h, _)| h);
    host.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remotes_parse_on_every_provider_and_protocol() {
        let r = Remote::parse("git@github.com:studio/atlas-web.git").unwrap();
        assert_eq!(
            (r.protocol, r.host.as_str(), r.owner.as_str(), r.name.as_str()),
            (Protocol::Ssh, "github.com", "studio", "atlas-web")
        );
        assert_eq!(r.provider, Provider::GitHub);
        assert_eq!(r.https_url(), "https://github.com/studio/atlas-web.git");
        assert_eq!(r.ssh_url(), "git@github.com:studio/atlas-web.git");

        let r = Remote::parse("https://gitlab.com/group/sub/infra.git").unwrap();
        assert_eq!(
            (r.protocol, r.owner.as_str(), r.name.as_str(), r.provider),
            (Protocol::Https, "group/sub", "infra", Provider::GitLab)
        );

        let r = Remote::parse("ssh://git@bitbucket.org:22/team/payments-api").unwrap();
        assert_eq!((r.host.as_str(), r.owner.as_str(), r.provider), ("bitbucket.org", "team", Provider::Bitbucket));

        let r = Remote::parse("git@github.com-work:studio/x.git").unwrap();
        assert_eq!(
            (r.host.as_str(), r.provider),
            ("github.com-work", Provider::GitHub),
            "an ssh config alias keeps its provider"
        );

        assert_eq!(Remote::parse("git@gitlab.example.com:ops/tf.git").unwrap().provider, Provider::GitLab);
        assert_eq!(Remote::parse("git@git.sr.ht:~me/notes").unwrap().provider, Provider::Other);
        assert_eq!(Remote::parse("file:///tmp/remotes/personal/dotfiles.git").unwrap().owner, "personal");
        assert_eq!(Remote::parse("not a url"), None);
    }
}
