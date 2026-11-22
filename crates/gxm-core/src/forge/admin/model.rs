//! A repository's settings on its provider, in the provider's terms but none of its formats.

use jiff::Timestamp;

/// What the person signed in may do with the repository, weakest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    Read,
    Triage,
    Write,
    Maintain,
    Admin,
}

impl Role {
    pub const ALL: [Role; 5] = [Role::Read, Role::Triage, Role::Write, Role::Maintain, Role::Admin];

    pub fn word(self) -> &'static str {
        match self {
            Role::Read => "Read",
            Role::Triage => "Triage",
            Role::Write => "Write",
            Role::Maintain => "Maintain",
            Role::Admin => "Admin",
        }
    }

    /// What the role can do, in a few words.
    pub fn means(self) -> &'static str {
        match self {
            Role::Read => "sees and clones",
            Role::Triage => "handles issues",
            Role::Write => "pushes",
            Role::Maintain => "runs the repository",
            Role::Admin => "changes every setting",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    /// Everyone in the enterprise (GitHub Enterprise only).
    Internal,
}

impl Visibility {
    pub fn word(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Internal => "internal",
        }
    }
}

/// What a squash commit or a merge commit says, by default, when a pull request merges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    /// The provider's default: the commits' messages (squash), or "Merge pull request #n" (merge).
    Default,
    /// The pull request's title alone.
    Title,
    /// Its title, with its description as the body.
    TitleAndDescription,
}

impl Message {
    pub const ALL: [Message; 3] = [Message::Default, Message::Title, Message::TitleAndDescription];

    pub fn word(self) -> &'static str {
        match self {
            Message::Default => "Default message",
            Message::Title => "Pull request title",
            Message::TitleAndDescription => "Title and description",
        }
    }
}

/// The repository's own settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    pub owner: String,
    /// Whether the owner is an organisation (teams, a base role) rather than a person.
    pub owner_is_org: bool,
    pub name: String,
    pub description: String,
    pub homepage: String,
    pub topics: Vec<String>,
    pub visibility: Visibility,
    pub archived: bool,
    pub template: bool,
    pub default_branch: String,
    pub issues: bool,
    pub projects: bool,
    pub wiki: bool,
    pub discussions: bool,
    pub merge_commit: bool,
    pub merge_message: Message,
    pub squash: bool,
    pub squash_message: Message,
    pub rebase: bool,
    pub auto_merge: bool,
    pub update_branch: bool,
    pub delete_branch_on_merge: bool,
    pub signoff: bool,
    /// What the person signed in may do here.
    pub role: Role,
}

/// One change to the repository's own settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Name(String),
    Description(String),
    Homepage(String),
    Visibility(Visibility),
    Archived(bool),
    Template(bool),
    DefaultBranch(String),
    Issues(bool),
    Projects(bool),
    Wiki(bool),
    Discussions(bool),
    MergeCommit(bool),
    MergeMessage(Message),
    Squash(bool),
    SquashMessage(Message),
    Rebase(bool),
    AutoMerge(bool),
    UpdateBranch(bool),
    DeleteBranchOnMerge(bool),
    Signoff(bool),
}

impl Change {
    /// The change made to `repo`, as the provider will have it once it agrees.
    pub fn apply(&self, repo: &mut Repo) {
        match self.clone() {
            Change::Name(v) => repo.name = v,
            Change::Description(v) => repo.description = v,
            Change::Homepage(v) => repo.homepage = v,
            Change::Visibility(v) => repo.visibility = v,
            Change::Archived(v) => repo.archived = v,
            Change::Template(v) => repo.template = v,
            Change::DefaultBranch(v) => repo.default_branch = v,
            Change::Issues(v) => repo.issues = v,
            Change::Projects(v) => repo.projects = v,
            Change::Wiki(v) => repo.wiki = v,
            Change::Discussions(v) => repo.discussions = v,
            Change::MergeCommit(v) => repo.merge_commit = v,
            Change::MergeMessage(v) => repo.merge_message = v,
            Change::Squash(v) => repo.squash = v,
            Change::SquashMessage(v) => repo.squash_message = v,
            Change::Rebase(v) => repo.rebase = v,
            Change::AutoMerge(v) => repo.auto_merge = v,
            Change::UpdateBranch(v) => repo.update_branch = v,
            Change::DeleteBranchOnMerge(v) => repo.delete_branch_on_merge = v,
            Change::Signoff(v) => repo.signoff = v,
        }
    }
}

/// People and teams with access to the repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct People {
    pub collaborators: Vec<Collaborator>,
    pub invitations: Vec<Invitation>,
    pub teams: Vec<Team>,
    /// The role everyone in the owning organisation has, when the owner is one and says.
    pub base: Option<Role>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collaborator {
    pub login: String,
    pub name: Option<String>,
    pub role: Role,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invitation {
    pub id: u64,
    pub login: String,
    pub role: Role,
    pub sent: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Team {
    pub slug: String,
    pub name: String,
    pub role: Role,
}

/// Who a role is given to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Grantee {
    Person(String),
    Team(String),
}

/// A rule set: rules that protect the branches or tags it targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ruleset {
    /// The provider's id; 0 for one not created yet.
    pub id: u64,
    pub name: String,
    pub target: Target,
    pub enforcement: Enforcement,
    /// The refs it covers, as people write them: `main`, `release/*`, or [`DEFAULT_BRANCH`].
    pub include: Vec<String>,
    pub rules: Rules,
    /// Admins may push past it.
    pub admins_bypass: bool,
}

/// How a rule set's targets name the default branch, whatever it's called.
pub const DEFAULT_BRANCH: &str = "~DEFAULT_BRANCH";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Branch,
    Tag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Enforcement {
    Active,
    /// A trial: it reports what it would block and blocks nothing.
    Evaluate,
    Disabled,
}

impl Enforcement {
    pub const ALL: [Enforcement; 3] = [Enforcement::Active, Enforcement::Evaluate, Enforcement::Disabled];

    pub fn word(self) -> &'static str {
        match self {
            Enforcement::Active => "Active",
            Enforcement::Evaluate => "Trial",
            Enforcement::Disabled => "Off",
        }
    }
}

/// The rules a rule set holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rules {
    /// Changes arrive by pull request, with this many approvals.
    pub pull_request: Option<u32>,
    /// These checks pass first.
    pub checks: Option<Vec<String>>,
    pub block_force_push: bool,
    pub block_deletion: bool,
    pub signed_commits: bool,
    pub linear_history: bool,
}

impl Ruleset {
    /// A new rule set protecting the default branch, as a person starts one.
    pub fn new_for_default_branch() -> Ruleset {
        Ruleset {
            id: 0,
            name: "Protect the default branch".to_owned(),
            target: Target::Branch,
            enforcement: Enforcement::Active,
            include: vec![DEFAULT_BRANCH.to_owned()],
            rules: Rules { pull_request: Some(1), block_force_push: true, block_deletion: true, ..Rules::default() },
            admins_bypass: true,
        }
    }

    /// Its rules, in a few words each: `Pull request (1 approval)`, `Block force-pushes`.
    pub fn summary(&self) -> Vec<String> {
        let r = &self.rules;
        let mut out = Vec::new();
        if let Some(n) = r.pull_request {
            out.push(match n {
                0 => "Pull request".to_owned(),
                1 => "Pull request (1 approval)".to_owned(),
                n => format!("Pull request ({n} approvals)"),
            });
        }
        if let Some(c) = &r.checks {
            out.push(if c.len() == 1 { "1 check".to_owned() } else { format!("{} checks", c.len()) });
        }
        for (on, words) in [
            (r.block_force_push, "Block force-pushes"),
            (r.block_deletion, "Block deleting"),
            (r.signed_commits, "Signed commits"),
            (r.linear_history, "Straight history"),
        ] {
            if on {
                out.push(words.to_owned());
            }
        }
        out
    }
}

/// Which actions workflows may use, and what their token may do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actions {
    pub enabled: bool,
    pub allowed: Allowed,
    /// The workflow token may write to the repository (else it reads).
    pub write: bool,
    /// Workflows may approve pull requests.
    pub approve_reviews: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allowed {
    All,
    /// Actions from the repository's own owner only.
    Local,
    /// GitHub's own and verified creators'.
    Selected,
}

impl Allowed {
    pub const ALL: [Allowed; 3] = [Allowed::All, Allowed::Selected, Allowed::Local];

    pub fn word(self) -> &'static str {
        match self {
            Allowed::All => "Any action",
            Allowed::Selected => "GitHub’s and verified",
            Allowed::Local => "Only the owner’s",
        }
    }
}

/// Where secrets are read: workflows, Dependabot, or codespaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    Actions,
    Dependabot,
    Codespaces,
}

impl Scope {
    pub const ALL: [Scope; 3] = [Scope::Actions, Scope::Dependabot, Scope::Codespaces];

    pub fn word(self) -> &'static str {
        match self {
            Scope::Actions => "Actions",
            Scope::Dependabot => "Dependabot",
            Scope::Codespaces => "Codespaces",
        }
    }

    /// The segment of the provider's paths: `actions`.
    pub fn path(self) -> &'static str {
        match self {
            Scope::Actions => "actions",
            Scope::Dependabot => "dependabot",
            Scope::Codespaces => "codespaces",
        }
    }
}

/// A secret: its name and when it last changed. Its value is never read back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Secret {
    pub name: String,
    pub updated: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub updated: Timestamp,
}

/// A deployment environment and what guards it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    pub name: String,
    /// Minutes a deployment waits before it starts.
    pub wait_minutes: u32,
    /// People who approve a deployment, by login.
    pub reviewers: Vec<String>,
    /// Only protected branches deploy to it.
    pub protected_only: bool,
}

/// A webhook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hook {
    pub id: u64,
    pub url: String,
    /// `push`, `pull_request`, or `*` for everything.
    pub events: Vec<String>,
    pub active: bool,
    /// The provider's answer from the last delivery: `200 OK`, or None before the first.
    pub last: Option<String>,
}

/// A new webhook.
#[derive(Clone, PartialEq, Eq)]
pub struct NewHook {
    pub url: String,
    pub secret: String,
    pub events: Vec<String>,
}

impl std::fmt::Debug for NewHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewHook").field("url", &self.url).field("events", &self.events).finish_non_exhaustive()
    }
}

/// The site the repository publishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pages {
    pub url: String,
    pub source: PagesSource,
    pub cname: Option<String>,
    pub https: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PagesSource {
    /// A workflow builds and deploys it.
    Workflow,
    /// A branch's root, or its `docs/` folder, is the site.
    Branch { branch: String, docs: bool },
}

/// The repository's security features.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Security {
    pub dependabot_alerts: bool,
    pub dependabot_updates: bool,
    pub secret_scanning: bool,
    pub push_protection: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityFeature {
    DependabotAlerts,
    DependabotUpdates,
    SecretScanning,
    PushProtection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeployKey {
    pub id: u64,
    pub title: String,
    /// The key's opening, enough to recognise it: `ssh-ed25519 AAAAC3Nz…`.
    pub key: String,
    pub read_only: bool,
    pub added: Timestamp,
}

/// A prefix that links references in text: `TICKET-123` → the tracker's page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Autolink {
    pub id: u64,
    pub prefix: String,
    /// The address, with `<num>` where the reference goes.
    pub template: String,
}
