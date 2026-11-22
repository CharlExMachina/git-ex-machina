//! The settings dialog's work off the UI thread: reads and changes on the provider, in git and
//! on disk, and the stronger sign-in deleting a repository needs. Reads queue behind the one in
//! flight rather than replacing it, so every read asked for lands.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gxm_core::auth::{self, DeviceCode};
use gxm_core::env::Env;
use gxm_core::forge::admin::{self, Admin, Change, Page, Reading, Write};
use gxm_core::forge::model::{PullSummary, User};
use gxm_core::forge::{ForgeError, NotConnected};
use gxm_core::remote::{Protocol, Remote};
use gxm_core::{git, local, tidy};
use rune::{Latest, Waker};

use super::{Action, Answers, Effect, Event, Found, Model, Section, StepUp, What};
use crate::state::App;

/// What a change says when it's done, and what it read back.
type Wrote = Result<(Option<String>, Answers), String>;

/// A write the provider refused because the token lacks a permission the person can grant.
const NEEDS_SCOPE: &str = "needs-scope";

#[derive(Default)]
pub struct Jobs {
    read: Latest<Answers>,
    waiting: Vec<What>,
    write: Latest<(Action, Wrote)>,
    code: Latest<Result<DeviceCode, String>>,
    approved: Latest<Result<(User, String), String>>,
    cancel: Arc<AtomicBool>,
    /// The repository's provider, connected when the dialog opens.
    pub admin: Option<Arc<dyn Admin>>,
    /// Why there's no provider to change settings on.
    pub not_connected: Option<NotConnected>,
    repo: PathBuf,
}

impl Jobs {
    pub fn set_waker(&self, waker: &Waker) {
        self.read.set_waker(waker.clone());
        self.write.set_waker(waker.clone());
        self.code.set_waker(waker.clone());
        self.approved.set_waker(waker.clone());
    }

    /// Work is landing. Waiting for the person to approve a code on GitHub isn't work: it can
    /// take as long as they like.
    pub fn busy(&self) -> bool {
        self.read.is_pending() || self.write.is_pending() || self.code.is_pending()
    }

    /// The provider's name, for "Saved on GitHub".
    pub fn provider(&self) -> &'static str {
        self.admin.as_ref().map_or("GitHub", |a| a.provider().name())
    }
}

impl App {
    /// Opens the settings of the selected repository.
    pub fn open_settings(&mut self) {
        let Some(r) = self.repo() else { return };
        let (path, origin) = (r.summary.path.clone(), r.summary.origin.clone());
        self.settings_jobs.repo = path;
        (self.settings_jobs.admin, self.settings_jobs.not_connected) = match origin.as_ref() {
            None => (None, Some(NotConnected::NoProvider)),
            Some(remote) => match admin::connect(&self.env, remote) {
                Ok(a) => (Some(a), None),
                Err(why) => (None, Some(why)),
            },
        };
        self.settings_event(Event::Open);
        // Without a provider, the dialog opens on the first section that lives here.
        let open = if self.settings_jobs.admin.is_none() { Section::Remotes } else { self.settings.section };
        self.settings_event(Event::Pick(open));
    }

    pub fn settings_do(&mut self, action: Action) {
        self.settings_event(Event::Do(action));
    }

    /// Feeds the dialog's machine and carries out what it asks for.
    pub fn settings_event(&mut self, event: Event) {
        let provider = self.settings_jobs.provider();
        let picked = matches!(event, Event::Pick(_));
        for effect in super::step(&mut self.settings, event, provider) {
            match effect {
                Effect::Read(whats) => self.settings_read(whats),
                Effect::Write(action) => self.settings_write(action),
            }
        }
        if picked {
            self.fill_drafts();
        }
    }

    fn settings_read(&mut self, mut whats: Vec<What>) {
        // Without a provider, its pages can't be read; the dialog says why instead.
        if self.settings_jobs.admin.is_none() {
            whats.retain(|w| !matches!(w, What::Forge(_)));
            for w in self.settings.reading.clone() {
                if matches!(w, What::Forge(_)) {
                    self.settings.reading.remove(&w);
                }
            }
        }
        if whats.is_empty() {
            return;
        }
        let pulls = self.pull_list();
        let jobs = &mut self.settings_jobs;
        if jobs.read.is_pending() {
            jobs.waiting.extend(whats);
            return;
        }
        let input = (self.env.clone(), jobs.repo.clone(), jobs.admin.clone(), pulls, whats);
        jobs.read
            .request(input, |(env, repo, admin, pulls, whats)| read_all(&env, &repo, admin.as_deref(), &pulls, &whats));
    }

    fn settings_write(&mut self, action: Action) {
        let jobs = &self.settings_jobs;
        let input = (self.env.clone(), jobs.repo.clone(), jobs.admin.clone(), self.pull_list(), action);
        jobs.write.request(input, |(env, repo, admin, pulls, action)| {
            let wrote = write(&env, &repo, admin.as_deref(), &action)
                .map(|said| (said, read_all(&env, &repo, admin.as_deref(), &pulls, &action.rereads())));
            (action, wrote)
        });
    }

    /// The pull requests known for the open repository, for tidying up.
    fn pull_list(&self) -> Vec<PullSummary> {
        self.pulls.index.as_ref().map(|i| i.pulls.clone()).unwrap_or_default()
    }

    /// Takes what the dialog's jobs have answered. True when the state changed.
    pub fn land_settings(&mut self) -> bool {
        let mut changed = false;
        if let Some(answers) = self.settings_jobs.read.take() {
            self.settings_event(Event::Read(answers));
            self.fill_drafts();
            let waiting = std::mem::take(&mut self.settings_jobs.waiting);
            if !waiting.is_empty() {
                self.settings_read(waiting);
            }
            changed = true;
        }
        if let Some((action, wrote)) = self.settings_jobs.write.take() {
            changed = true;
            if matches!(action, Action::Forge(Write::Delete))
                && wrote.as_ref().err().map(String::as_str) == Some(NEEDS_SCOPE)
            {
                self.step_up();
                self.settings_event(Event::Wrote(action, Err(String::new())));
                // Nothing failed: the danger zone says what GitHub wants.
                self.settings.status = super::Status::Idle;
                return true;
            }
            let said = wrote.as_ref().ok().and_then(|(said, _)| said.clone());
            let reload = matches!(
                action,
                Action::DeleteBranches(_) | Action::Remote(_) | Action::Forge(Write::Change(Change::Name(_)))
            );
            self.settings_event(Event::Wrote(action, wrote.map(|(_, answers)| answers)));
            if let (Some(said), super::Status::Saved(_)) = (said, &self.settings.status) {
                self.settings.status = super::Status::Saved(said);
            }
            self.fill_drafts();
            if reload {
                self.reload_repo();
            }
        }
        if let Some(code) = self.settings_jobs.code.take() {
            changed = true;
            match code {
                Ok(code) => {
                    self.settings.step_up = StepUp::Code(code.clone());
                    self.settings_jobs.cancel.store(false, Ordering::Relaxed);
                    let cancel = self.settings_jobs.cancel.clone();
                    self.settings_jobs.approved.request((self.env.clone(), code, cancel), |(env, code, cancel)| {
                        let token =
                            auth::wait(&env, &code, &|| cancel.load(Ordering::Relaxed)).map_err(|e| e.to_string())?;
                        let account = auth::whoami(&env, &token).map_err(|e| e.to_string())?;
                        auth::remember(&env, &account, &token).map_err(|e| e.to_string())?;
                        Ok((account, token))
                    });
                }
                Err(e) => self.settings.step_up = StepUp::Failed(e),
            }
        }
        if let Some(approved) = self.settings_jobs.approved.take() {
            changed = true;
            match approved {
                Ok((_, token)) => {
                    self.settings.step_up = StepUp::None;
                    self.env.forge.github_token = Some(token);
                    if let Some(remote) = self.repo().and_then(|r| r.summary.origin.clone()) {
                        self.settings_jobs.admin = admin::connect(&self.env, &remote).ok();
                    }
                    self.settings_do(Action::Forge(Write::Delete));
                }
                Err(e) => self.settings.step_up = StepUp::Failed(e),
            }
        }
        changed
    }

    /// Asks GitHub for a code that grants deleting repositories.
    pub fn step_up(&mut self) {
        self.settings.step_up = StepUp::Asking;
        self.settings_jobs
            .code
            .request(self.env.clone(), |env| auth::start_with(&env, auth::DELETE_SCOPES).map_err(|e| e.to_string()));
    }

    pub fn cancel_step_up(&mut self) {
        self.settings_jobs.cancel.store(true, Ordering::Relaxed);
        self.settings.step_up = StepUp::None;
    }

    /// Sets the fields from what was read, where the person hasn't started typing.
    fn fill_drafts(&mut self) {
        let m = &mut self.settings;
        let d = &mut m.drafts;
        let fill = |edit: &mut rune::TextEdit, value: &str| {
            if edit.text.is_empty() {
                *edit = rune::TextEdit::new(value);
            }
        };
        if m.section == Section::General
            && let Some(Found::Forge(Reading::Repo(r))) = m.found.get(&What::Forge(Page::Repo))
        {
            fill(&mut d.name, &r.name);
            fill(&mut d.description, &r.description);
            fill(&mut d.homepage, &r.homepage);
        }
        if m.section == Section::Identity
            && let Some(Found::Local(l)) = m.found.get(&What::Local)
        {
            fill(&mut d.user_name, &l.identity.name);
            fill(&mut d.user_email, &l.identity.email);
            if let gxm_core::local::Signing::Ssh(k) | gxm_core::local::Signing::Gpg(k) = &l.identity.signing {
                fill(&mut d.signing_key, k);
            }
        }
        if m.section == Section::Pages
            && let Some(Found::Forge(Reading::Pages(Some(p)))) = m.found.get(&What::Forge(Page::Pages))
        {
            fill(&mut d.cname, p.cname.as_deref().unwrap_or_default());
        }
    }
}

/// Reads `whats` in order. The default branch tidying up measures against comes from the
/// provider when it was read first, else from git.
fn read_all(env: &Env, repo: &Path, admin: Option<&dyn Admin>, pulls: &[PullSummary], whats: &[What]) -> Answers {
    let mut default: Option<String> = None;
    let mut out = Vec::new();
    for what in whats {
        let found = match *what {
            What::Forge(page) => match admin {
                Some(a) => a.read(page).map(Found::Forge).map_err(|e| say(&e)),
                None => Err("not connected".to_owned()),
            },
            What::Local => local::read(env, repo).map(Found::Local).map_err(|e| e.to_string()),
            What::Remotes => local::remotes(env, repo).map(Found::Remotes).map_err(|e| e.to_string()),
            What::Lfs => Ok(Found::Lfs(local::lfs(env, repo))),
            What::Branches => {
                let default = default.clone().unwrap_or_else(|| tidy::default_branch(env, repo));
                tidy::survey(env, repo, &default, pulls).map(Found::Branches).map_err(|e| e.to_string())
            }
            What::RemoteBranches => {
                git::run(env, repo, &["for-each-ref", "--format=%(refname:lstrip=3)", "refs/remotes/origin"])
                    .map(|out| Found::RemoteBranches(out.lines().filter(|b| *b != "HEAD").map(str::to_owned).collect()))
                    .map_err(|e| e.to_string())
            }
        };
        if let Ok(Found::Forge(Reading::Repo(r))) = &found {
            default = Some(r.default_branch.clone());
        }
        out.push((*what, found));
    }
    out
}

/// Carries out `action`; what it says when done, if more than where it's saved.
fn write(env: &Env, repo: &Path, admin: Option<&dyn Admin>, action: &Action) -> Result<Option<String>, String> {
    match action {
        Action::Forge(w) => {
            let a = admin.ok_or("not connected")?;
            match a.write(w) {
                Err(ForgeError::Status(403)) if *w == Write::Delete => return Err(NEEDS_SCOPE.to_owned()),
                Err(e) => return Err(say(&e)),
                Ok(()) => {}
            }
            match w {
                // A renamed repository: origin follows it, over the protocol it used.
                Write::Change(Change::Name(new)) => {
                    let url =
                        git::run(env, repo, &["config", "--get", "remote.origin.url"]).map_err(|e| e.to_string())?;
                    if let Some(mut remote) = Remote::parse(&url) {
                        remote.name = new.clone();
                        let url =
                            if remote.protocol == Protocol::Https { remote.https_url() } else { remote.ssh_url() };
                        local::change_remote(
                            env,
                            repo,
                            &local::RemoteChange::SetUrl { name: "origin".to_owned(), url },
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    Ok(Some(format!("Renamed to {new}")))
                }
                Write::Delete => Ok(Some(format!("Deleted {} on {}", a.full_name(), a.provider().name()))),
                Write::Transfer(to) => Ok(Some(format!("Transfer to {to} started"))),
                _ => Ok(None),
            }
        }
        Action::Local(c) => local::write(env, repo, c).map(|()| None).map_err(|e| e.to_string()),
        Action::Remote(c) => local::change_remote(env, repo, c).map(|()| None).map_err(|e| e.to_string()),
        Action::Track(p) => local::track(repo, p).map(|()| None).map_err(|e| e.to_string()),
        Action::Untrack(p) => local::untrack(repo, p).map(|()| None).map_err(|e| e.to_string()),
        Action::DeleteBranches(bs) => {
            tidy::delete(env, repo, bs).map(|d| Some(d.sentence())).map_err(|e| e.to_string())
        }
    }
}

/// A provider's refusal as the dialog says it.
fn say(e: &ForgeError) -> String {
    let s = match e {
        ForgeError::Status(403) => "GitHub refused: only the repository's admins can change this".to_owned(),
        ForgeError::Status(404) => "GitHub doesn't know this repository any more".to_owned(),
        other => other.to_string(),
    };
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

impl Model {
    /// Why provider sections can't be changed, if they can't.
    pub fn locked(&self, jobs: &Jobs) -> Option<String> {
        if let Some(why) = &jobs.not_connected {
            return Some(match why {
                NotConnected::NoProvider => {
                    "This repository has no remote on GitHub, GitLab or Bitbucket: only what lives on this computer is here.".to_owned()
                }
                NotConnected::NotYet(p) => format!("{}'s settings are coming next. What lives on this computer is here now.", p.name()),
                NotConnected::NoToken(p) => format!("Sign in to {} to change its settings there.", p.name()),
            });
        }
        self.repo().filter(|r| r.role != admin::Role::Admin).map(|r| {
            format!(
                "Only admins change {}/{}'s settings on GitHub. You have {} access, so you see them here and change what lives on this computer.",
                r.owner,
                r.name,
                r.role.word()
            )
        })
    }
}
