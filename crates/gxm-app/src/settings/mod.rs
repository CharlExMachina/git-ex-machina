//! The repository settings dialog: every setting of the selected repository, on its provider
//! (GitHub's settings pages, all of them) and on this computer (its `.git/config`, its large
//! files), tidying up its merged branches, and the app's own settings for every repository (the
//! agent harness it drives).
//!
//! One machine ([`Model`], [`step`]): opening reads the repository, picking a section reads what
//! it shows, and every change is an [`Action`] carried out off the UI thread, one at a time, in
//! order. Each action reads back what it changed, so the dialog always shows what the provider
//! and git hold; a checkbox flips at once and settles when the answer lands.

mod work;

use std::collections::{HashMap, HashSet, VecDeque};

use gxm_core::forge::admin::{Page, Reading, Repo, Role, Scope, Write};
use gxm_core::harness::{Harness, HarnessStatus};
use gxm_core::local::{Lfs, LocalChange, LocalSettings, RemoteChange, RemoteEntry, Signing};
use gxm_core::tidy::Branch;
use rune::TextEdit;

pub use work::Jobs;

/// The dialog's sections, in the order its sidebar lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    General,
    Branches,
    PullRequests,
    Rules,
    People,
    Actions,
    Secrets,
    Environments,
    Webhooks,
    Pages,
    CodeSecurity,
    DeployKeys,
    Autolinks,
    Remotes,
    Identity,
    LargeFiles,
    Agents,
    DangerZone,
}

impl Section {
    pub const ALL: [Section; 18] = [
        Section::General,
        Section::Branches,
        Section::PullRequests,
        Section::Rules,
        Section::People,
        Section::Actions,
        Section::Secrets,
        Section::Environments,
        Section::Webhooks,
        Section::Pages,
        Section::CodeSecurity,
        Section::DeployKeys,
        Section::Autolinks,
        Section::Remotes,
        Section::Identity,
        Section::LargeFiles,
        Section::Agents,
        Section::DangerZone,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Branches => "Branches",
            Section::PullRequests => "Pull requests",
            Section::Rules => "Rules",
            Section::People => "People",
            Section::Actions => "Actions",
            Section::Secrets => "Secrets and variables",
            Section::Environments => "Environments",
            Section::Webhooks => "Webhooks",
            Section::Pages => "Pages",
            Section::CodeSecurity => "Code security",
            Section::DeployKeys => "Deploy keys",
            Section::Autolinks => "Autolinks",
            Section::Remotes => "Remotes",
            Section::Identity => "Identity",
            Section::LargeFiles => "Large files",
            Section::Agents => "Agents",
            Section::DangerZone => "Danger zone",
        }
    }

    /// The sidebar's group heading above it, if it starts one.
    pub fn group(self) -> Option<&'static str> {
        match self {
            Section::People => Some("Access"),
            Section::Actions => Some("Automation"),
            Section::CodeSecurity => Some("Security"),
            Section::Remotes => Some("This computer"),
            Section::Agents => Some("Every repository"),
            _ => None,
        }
    }

    /// It lives on the provider (and needs a connection to it).
    pub fn on_provider(self) -> bool {
        !matches!(self, Section::Remotes | Section::Identity | Section::LargeFiles | Section::Agents)
    }

    /// What it reads to show itself.
    pub fn reads(self, scope: Scope) -> Vec<What> {
        let forge = |p| What::Forge(p);
        match self {
            Section::General | Section::PullRequests | Section::DangerZone => vec![forge(Page::Repo)],
            Section::Branches => {
                vec![forge(Page::Repo), What::Local, What::RemoteBranches, What::Branches]
            }
            Section::Rules => vec![forge(Page::Rules)],
            Section::People => vec![forge(Page::People)],
            Section::Actions => vec![forge(Page::Actions)],
            Section::Secrets => vec![forge(Page::Secrets(scope)), forge(Page::Variables)],
            Section::Environments => vec![forge(Page::Environments)],
            Section::Webhooks => vec![forge(Page::Hooks)],
            Section::Pages => vec![forge(Page::Pages), What::RemoteBranches],
            Section::CodeSecurity => vec![forge(Page::Security)],
            Section::DeployKeys => vec![forge(Page::DeployKeys)],
            Section::Autolinks => vec![forge(Page::Autolinks)],
            Section::Remotes => vec![What::Remotes, What::Local],
            Section::Identity => vec![What::Local],
            Section::LargeFiles => vec![What::Lfs],
            Section::Agents => vec![What::Agents],
        }
    }
}

/// Something the dialog reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum What {
    Forge(Page),
    /// This computer's settings for the repository.
    Local,
    Remotes,
    Lfs,
    /// The branches, done and kept (tidying up).
    Branches,
    /// `origin`'s branches, to pick a default branch or a Pages branch from.
    RemoteBranches,
    /// The agent harnesses on this computer, and the one chosen.
    Agents,
}

/// What a read found.
#[derive(Clone, Debug)]
pub enum Found {
    Forge(Reading),
    Local(LocalSettings),
    Remotes(Vec<RemoteEntry>),
    Lfs(Lfs),
    Branches(Vec<Branch>),
    RemoteBranches(Vec<String>),
    Agents(Agents),
}

/// The agent harnesses found on this computer, in the order the page lists them, and the one
/// the person chose, if they did.
#[derive(Clone, Debug)]
pub struct Agents {
    pub statuses: Vec<HarnessStatus>,
    pub chosen: Option<Harness>,
}

impl Agents {
    /// The one a session would use: the chosen one while it's usable, else the first usable.
    pub fn preferred(&self) -> Option<Harness> {
        gxm_core::harness::pick(self.chosen, &self.statuses)
    }
}

/// Each read's answer, in the order asked.
pub type Answers = Vec<(What, Result<Found, String>)>;

/// One change the dialog makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Forge(Write),
    Local(LocalChange),
    Remote(RemoteChange),
    Track(String),
    Untrack(String),
    /// Deletes these branches, done ones only, here and on origin.
    DeleteBranches(Vec<Branch>),
    /// Makes this the agent harness the app drives.
    PreferAgent(Harness),
}

impl Action {
    /// What to read again once it's done.
    pub fn rereads(&self) -> Vec<What> {
        match self {
            Action::Forge(w @ Write::Change(gxm_core::forge::admin::Change::DefaultBranch(_))) => {
                vec![What::Forge(w.page()), What::Branches]
            }
            Action::Forge(w @ Write::Change(gxm_core::forge::admin::Change::Name(_))) => {
                vec![What::Forge(w.page()), What::Remotes]
            }
            Action::Forge(w) => vec![What::Forge(w.page())],
            Action::Local(_) => vec![What::Local],
            Action::Remote(_) => vec![What::Remotes],
            Action::Track(_) | Action::Untrack(_) => vec![What::Lfs],
            Action::DeleteBranches(_) => vec![What::Branches, What::RemoteBranches],
            Action::PreferAgent(_) => vec![What::Agents],
        }
    }

    /// Where it's kept, for the header once it's done: `Saved on GitHub`.
    fn saved(&self, provider: &str) -> String {
        match self {
            Action::Forge(Write::PingHook(_)) => format!("Pinged by {provider}"),
            Action::Forge(_) => format!("Saved on {provider}"),
            Action::Local(_) | Action::Remote(_) => "Saved in .git/config".to_owned(),
            Action::Track(_) | Action::Untrack(_) => "Saved in .gitattributes".to_owned(),
            Action::DeleteBranches(_) => "Deleted".to_owned(),
            Action::PreferAgent(_) => "Saved for every repository".to_owned(),
        }
    }
}

/// What the header says about the last change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Saving,
    Saved(String),
    Failed(String),
}

/// Deleting the repository needs a stronger sign-in than the app asks for at first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepUp {
    None,
    Asking,
    Code(gxm_core::auth::DeviceCode),
    Failed(String),
}

/// A confirmation the danger zone is waiting for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    None,
    Visibility,
    Archive,
    Transfer,
}

/// The dialog's state.
pub struct Model {
    pub open: bool,
    pub section: Section,
    pub status: Status,
    /// Changes waiting for the one in flight.
    queue: VecDeque<Action>,
    pub writing: Option<Action>,
    /// What's being read.
    pub reading: HashSet<What>,
    pub found: HashMap<What, Found>,
    /// Why a read failed.
    pub errors: HashMap<What, String>,
    pub scope: Scope,
    /// The rule set open for editing, by name.
    pub rule_open: Option<String>,
    /// The role an invitation gives.
    pub invite_role: Role,
    /// Which events a new webhook hears: pushes, pushes and pull requests, or everything.
    pub hook_events: usize,
    /// A new deploy key may write.
    pub key_write: bool,
    /// How commits are to be signed, while its key is being typed.
    pub signing: Signing,
    /// Branches left unticked in the tidy-up list.
    pub untick: HashSet<String>,
    pub show_kept: bool,
    pub confirm: Confirm,
    pub step_up: StepUp,
    /// The repository was deleted on its provider.
    pub deleted: bool,
    pub drafts: Drafts,
}

/// The dialog's fields. They outlive a section, so a half-written secret survives a look at
/// another one; opening the dialog sets them afresh.
#[derive(Default)]
pub struct Drafts {
    pub name: TextEdit,
    pub description: TextEdit,
    pub homepage: TextEdit,
    pub topic: TextEdit,
    pub invite: TextEdit,
    pub rule_name: TextEdit,
    pub rule_target: TextEdit,
    pub rule_check: TextEdit,
    pub secret_name: TextEdit,
    pub secret_value: TextEdit,
    pub var_name: TextEdit,
    pub var_value: TextEdit,
    pub env_name: TextEdit,
    /// Each environment's field for a new reviewer, by its name.
    pub reviewers: HashMap<String, TextEdit>,
    pub hook_url: TextEdit,
    pub hook_secret: TextEdit,
    pub cname: TextEdit,
    pub key_title: TextEdit,
    pub key_value: TextEdit,
    pub link_prefix: TextEdit,
    pub link_template: TextEdit,
    pub remote_name: TextEdit,
    pub remote_url: TextEdit,
    pub user_name: TextEdit,
    pub user_email: TextEdit,
    pub signing_key: TextEdit,
    pub lfs_pattern: TextEdit,
    pub transfer_to: TextEdit,
    pub confirm_delete: TextEdit,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            open: false,
            section: Section::General,
            status: Status::Idle,
            queue: VecDeque::new(),
            writing: None,
            reading: HashSet::new(),
            found: HashMap::new(),
            errors: HashMap::new(),
            scope: Scope::Actions,
            rule_open: None,
            invite_role: Role::Write,
            hook_events: 0,
            key_write: false,
            signing: Signing::Off,
            untick: HashSet::new(),
            show_kept: false,
            confirm: Confirm::None,
            step_up: StepUp::None,
            deleted: false,
            drafts: Drafts::default(),
        }
    }
}

#[derive(Debug)]
pub enum Event {
    Open,
    Close,
    Pick(Section),
    Scope(Scope),
    Do(Action),
    Read(Answers),
    Wrote(Action, Result<Answers, String>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    Read(Vec<What>),
    Write(Action),
}

/// One transition. `provider` names where provider settings live (`GitHub`).
pub fn step(m: &mut Model, event: Event, provider: &str) -> Vec<Effect> {
    match event {
        Event::Open => {
            *m = Model { open: true, ..Model::default() };
            let mut reads = vec![What::Forge(Page::Repo)];
            reads.extend(m.section.reads(m.scope));
            reads.dedup();
            m.reading.extend(reads.iter().copied());
            vec![Effect::Read(reads)]
        }
        Event::Close => {
            m.open = false;
            m.confirm = Confirm::None;
            vec![]
        }
        Event::Pick(section) => {
            m.section = section;
            m.confirm = Confirm::None;
            read(m, section.reads(m.scope))
        }
        Event::Scope(scope) => {
            m.scope = scope;
            read(m, vec![What::Forge(Page::Secrets(scope))])
        }
        Event::Do(action) => {
            optimistic(m, &action);
            m.status = Status::Saving;
            if m.writing.is_some() {
                m.queue.push_back(action);
                return vec![];
            }
            m.writing = Some(action.clone());
            vec![Effect::Write(action)]
        }
        Event::Read(found) => {
            land(m, found);
            vec![]
        }
        Event::Wrote(action, result) => {
            m.status = match &result {
                Ok(_) => Status::Saved(action.saved(provider)),
                Err(e) => Status::Failed(e.clone()),
            };
            if let (Action::Forge(Write::Delete), Ok(_)) = (&action, &result) {
                m.deleted = true;
            }
            if let Ok(found) = result {
                land(m, found);
            } else {
                // What the dialog guessed is wrong: read the truth again.
                return read(m, action.rereads()).into_iter().chain(next(m)).collect();
            }
            next(m)
        }
    }
}

/// Reads `whats`, unless they're being read already.
fn read(m: &mut Model, whats: Vec<What>) -> Vec<Effect> {
    let fresh: Vec<What> = whats.into_iter().filter(|w| m.reading.insert(*w)).collect();
    if fresh.is_empty() { vec![] } else { vec![Effect::Read(fresh)] }
}

/// The next queued change, now that the last one landed.
fn next(m: &mut Model) -> Vec<Effect> {
    m.writing = m.queue.pop_front();
    match &m.writing {
        Some(a) => {
            m.status = Status::Saving;
            vec![Effect::Write(a.clone())]
        }
        None => vec![],
    }
}

fn land(m: &mut Model, found: Answers) {
    for (what, result) in found {
        m.reading.remove(&what);
        match result {
            Ok(f) => {
                m.errors.remove(&what);
                m.found.insert(what, f);
            }
            Err(e) => {
                m.errors.insert(what, e);
            }
        }
    }
}

/// Shows a change before the provider confirms it: a ticked box ticks at once.
fn optimistic(m: &mut Model, action: &Action) {
    match action {
        Action::Forge(Write::Change(c)) => {
            if let Some(Found::Forge(Reading::Repo(r))) = m.found.get_mut(&What::Forge(Page::Repo)) {
                c.apply(r);
            }
        }
        Action::Local(change) => {
            if let Some(Found::Local(l)) = m.found.get_mut(&What::Local) {
                match change.clone() {
                    LocalChange::FetchPrune(v) => l.fetch_prune = v,
                    LocalChange::Pull(v) => l.pull = v,
                    LocalChange::AutoSetupRemote(v) => l.auto_setup_remote = v,
                    LocalChange::FollowTags(v) => l.follow_tags = v,
                    LocalChange::DeleteMerged(v) => l.delete_merged = v,
                    LocalChange::Signing(v) => l.identity.signing = v,
                    LocalChange::Name(_) | LocalChange::Email(_) => {}
                }
            }
        }
        Action::PreferAgent(h) => {
            if let Some(Found::Agents(a)) = m.found.get_mut(&What::Agents) {
                a.chosen = Some(*h);
            }
        }
        _ => {}
    }
}

impl Model {
    /// The repository as the provider has it, once read.
    pub fn repo(&self) -> Option<&Repo> {
        match self.found.get(&What::Forge(Page::Repo)) {
            Some(Found::Forge(Reading::Repo(r))) => Some(r),
            _ => None,
        }
    }

    /// The signed-in person may change the provider's settings.
    pub fn admin(&self) -> bool {
        self.repo().is_some_and(|r| r.role == Role::Admin)
    }

    pub fn forge(&self, page: Page) -> Option<&Reading> {
        match self.found.get(&What::Forge(page)) {
            Some(Found::Forge(r)) => Some(r),
            _ => None,
        }
    }

    pub fn local(&self) -> Option<&LocalSettings> {
        match self.found.get(&What::Local) {
            Some(Found::Local(l)) => Some(l),
            _ => None,
        }
    }

    pub fn branches(&self) -> Option<&[Branch]> {
        match self.found.get(&What::Branches) {
            Some(Found::Branches(b)) => Some(b),
            _ => None,
        }
    }

    pub fn agents(&self) -> Option<&Agents> {
        match self.found.get(&What::Agents) {
            Some(Found::Agents(a)) => Some(a),
            _ => None,
        }
    }

    pub fn remote_branches(&self) -> &[String] {
        match self.found.get(&What::RemoteBranches) {
            Some(Found::RemoteBranches(b)) => b,
            _ => &[],
        }
    }

    /// The branches ticked for deleting: every done one the person hasn't unticked.
    pub fn ticked(&self) -> Vec<Branch> {
        self.branches()
            .unwrap_or_default()
            .iter()
            .filter(|b| b.done() && !self.untick.contains(&b.name))
            .cloned()
            .collect()
    }

    /// Something is being read or written.
    pub fn busy(&self) -> bool {
        self.writing.is_some() || !self.reading.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use gxm_core::forge::admin::Change;

    use super::*;

    #[test]
    fn changes_go_one_at_a_time_in_order_and_a_refused_one_reads_the_truth_again() {
        let mut m = Model::default();
        let opened = step(&mut m, Event::Open, "GitHub");
        assert_eq!(opened, [Effect::Read(vec![What::Forge(Page::Repo)])]);
        let wiki = Action::Forge(Write::Change(Change::Wiki(true)));
        let rebase = Action::Forge(Write::Change(Change::Rebase(true)));
        assert_eq!(step(&mut m, Event::Do(wiki.clone()), "GitHub"), [Effect::Write(wiki.clone())]);
        assert!(step(&mut m, Event::Do(rebase.clone()), "GitHub").is_empty(), "the second waits");
        assert_eq!(m.status, Status::Saving);
        let after = step(&mut m, Event::Wrote(wiki, Err("403".to_owned())), "GitHub");
        assert_eq!(after, [Effect::Write(rebase.clone())], "the repository is being read already");
        assert_eq!(m.status, Status::Saving);
        step(&mut m, Event::Wrote(rebase, Ok(vec![])), "GitHub");
        assert_eq!(m.status, Status::Saved("Saved on GitHub".to_owned()));
        assert!(m.writing.is_none());
    }
}
