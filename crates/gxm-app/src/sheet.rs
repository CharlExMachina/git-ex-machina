//! The sheets over the window: Add (a repository already on disk), Init (a folder Add found
//! without git, made a repository where it is) and Clone (one from a remote, over SSH). (The
//! account's and the update's sheets have machines of their own: `account`, `update`.) Each is
//! a small machine: editing, then working off the UI thread, then closed with the repository
//! open in the window, or back to editing with what went wrong. A clone can be cancelled while
//! it runs.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use gxm_core::clone::{self, CloneError};
use gxm_core::env::Env;
use gxm_core::git::GitError;
use gxm_core::init::{self, Plan};
use gxm_core::repo::RepoSummary;
use gxm_core::workspace::{self, AddError};
use rune::{Latest, TextEdit, Tx, Waker};

use crate::keys;
use crate::ops::Ops;
use crate::state::{App, RepoEntry};

pub enum Sheet {
    Closed,
    /// Signing in to GitHub, or who's signed in.
    Account(crate::account::SignIn),
    Add(Phase),
    Init {
        offer: Offer,
        phase: Phase,
    },
    Clone(Phase),
    /// An update is in place: restart into it, with its notes, and why a restart failed.
    Update {
        error: Option<String>,
    },
}

impl Sheet {
    fn phase(&self) -> Option<&Phase> {
        match self {
            Sheet::Closed | Sheet::Account(_) | Sheet::Update { .. } => None,
            Sheet::Add(phase) | Sheet::Clone(phase) | Sheet::Init { phase, .. } => Some(phase),
        }
    }
}

/// A folder without git, and what it already holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub folder: PathBuf,
    pub survey: init::Folder,
}

/// Why an add or a clone didn't open a repository.
pub enum Refusal {
    /// What went wrong, to show.
    Said(String),
    /// Add found a folder without git: it can become a repository.
    NotARepo(Box<Offer>),
}

/// What the sheets' fields hold. They outlive a sheet, so an edit that lands as one closes has
/// somewhere to go; opening a sheet sets them afresh.
pub struct Fields {
    /// Add's folder.
    pub path: TextEdit,
    /// Clone's repository, as typed or pasted.
    pub url: TextEdit,
    /// The folder Clone clones into.
    pub into: TextEdit,
    /// Init's name for the repository (the README's heading, its name on GitHub).
    pub name: TextEdit,
    pub readme: TextEdit,
    /// The README is still the one made from the name, so it follows the name.
    pub readme_follows: bool,
    pub write_readme: bool,
    pub write_ignore: bool,
    /// Track the folder's binary files with Git LFS. Off until the person turns it on: it needs
    /// `git-lfs` wherever the repository goes, and counts against the host's LFS quota.
    pub use_lfs: bool,
}

impl Default for Fields {
    fn default() -> Self {
        Fields {
            path: TextEdit::new(""),
            url: TextEdit::new(""),
            into: TextEdit::new(""),
            name: TextEdit::new(""),
            readme: TextEdit::new(""),
            readme_follows: true,
            write_readme: true,
            write_ignore: true,
            use_lfs: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Editing {
        error: Option<String>,
    },
    Working,
    /// A clone told to stop, until it has.
    Cancelling,
}

impl Phase {
    pub fn editing(&self) -> bool {
        matches!(self, Phase::Editing { .. })
    }
}

pub enum Event {
    OpenAdd,
    OpenClone,
    Submit,
    /// Escape, a click outside, or Cancel.
    Dismiss,
    /// Init's name changed; the README's heading follows it until the README is edited.
    NameEdited,
    ReadmeEdited,
    ToggleReadme,
    ToggleIgnore,
    ToggleLfs,
    Landed(Result<Box<RepoEntry>, Refusal>),
}

pub enum Effect {
    Focus(&'static str),
    Add(PathBuf),
    Init { folder: PathBuf, plan: Plan },
    Clone { url: String, dest: PathBuf },
    StopClone,
    StopSignIn,
    Open(Box<RepoEntry>),
}

/// One transition. `env` reads typed paths; `into` is where a new clone goes by default.
pub fn step(sheet: &mut Sheet, fields: &mut Fields, event: Event, env: &Env, into: &Path) -> Vec<Effect> {
    let editing = Phase::Editing { error: None };
    let fail = |why: &str| Phase::Editing { error: Some(why.to_owned()) };
    let working = sheet.phase().is_some_and(|p| !p.editing());
    match (sheet, event) {
        (_, Event::OpenAdd | Event::OpenClone) if working => vec![],
        (sheet, Event::OpenAdd) => {
            *sheet = Sheet::Add(editing);
            fields.path = TextEdit::new(format!("{}/", env.tilde(into)));
            vec![Effect::Focus(keys::ADD_PATH)]
        }
        (sheet, Event::OpenClone) => {
            *sheet = Sheet::Clone(editing);
            fields.url = TextEdit::new("");
            fields.into = TextEdit::new(env.tilde(into));
            vec![Effect::Focus(keys::CLONE_URL)]
        }
        (Sheet::Add(phase), Event::Submit) if phase.editing() => {
            if fields.path.text.trim().is_empty() {
                *phase = fail("Type the folder's path");
                return vec![];
            }
            *phase = Phase::Working;
            vec![Effect::Add(env.expand(&fields.path.text))]
        }
        (Sheet::Init { offer, phase }, Event::Submit) if phase.editing() => {
            *phase = Phase::Working;
            let plan = Plan {
                readme: (fields.write_readme && !offer.survey.has_readme).then(|| fields.readme.text.clone()),
                ignore: offer.survey.kind.filter(|_| fields.write_ignore && !offer.survey.has_ignore),
                lfs: if fields.use_lfs && offer.survey.lfs { offer.survey.binary.clone() } else { Vec::new() },
            };
            vec![Effect::Init { folder: offer.folder.clone(), plan }]
        }
        (Sheet::Init { phase, .. }, Event::NameEdited) if phase.editing() && fields.readme_follows => {
            fields.readme = TextEdit::new(init::readme(&fields.name.text));
            vec![]
        }
        (Sheet::Init { .. }, Event::ReadmeEdited) => {
            fields.readme_follows = false;
            vec![]
        }
        (Sheet::Init { phase, .. }, Event::ToggleReadme) if phase.editing() => {
            fields.write_readme = !fields.write_readme;
            vec![]
        }
        (Sheet::Init { phase, .. }, Event::ToggleIgnore) if phase.editing() => {
            fields.write_ignore = !fields.write_ignore;
            vec![]
        }
        (Sheet::Init { offer, phase }, Event::ToggleLfs) if phase.editing() && offer.survey.lfs => {
            fields.use_lfs = !fields.use_lfs;
            vec![]
        }
        (Sheet::Clone(phase), Event::Submit) if phase.editing() => match plan(env, fields) {
            Some((source, dest)) => {
                *phase = Phase::Working;
                vec![Effect::Clone { url: source.url, dest }]
            }
            None => {
                *phase = fail("Paste the repository's URL, or type owner/name for GitHub");
                vec![]
            }
        },
        (Sheet::Clone(phase), Event::Dismiss) if *phase == Phase::Working => {
            *phase = Phase::Cancelling;
            vec![Effect::StopClone]
        }
        (sheet @ Sheet::Account(_), Event::Dismiss) => {
            *sheet = Sheet::Closed;
            vec![Effect::StopSignIn]
        }
        (sheet, Event::Dismiss) if !working => {
            *sheet = Sheet::Closed;
            vec![]
        }
        (sheet @ Sheet::Clone(Phase::Cancelling), Event::Landed(Err(_))) => {
            *sheet = Sheet::Closed;
            vec![]
        }
        (sheet, Event::Landed(Ok(entry))) if working => {
            *sheet = Sheet::Closed;
            vec![Effect::Open(entry)]
        }
        (sheet @ Sheet::Add(_), Event::Landed(Err(Refusal::NotARepo(offer)))) if working => {
            let name = offer.folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            fields.readme = TextEdit::new(init::readme(&name));
            fields.name = TextEdit::new(name);
            fields.readme_follows = true;
            fields.write_readme = !offer.survey.has_readme;
            fields.write_ignore = offer.survey.kind.is_some() && !offer.survey.has_ignore;
            fields.use_lfs = false;
            *sheet = Sheet::Init { offer: *offer, phase: editing };
            vec![Effect::Focus(keys::INIT_NAME)]
        }
        (sheet, Event::Landed(Err(refusal))) if working => {
            let said = match refusal {
                Refusal::Said(said) => said,
                Refusal::NotARepo(offer) => format!("{} isn't a git repository", env.tilde(&offer.folder)),
            };
            if let Sheet::Add(phase) | Sheet::Clone(phase) | Sheet::Init { phase, .. } = sheet {
                *phase = Phase::Editing { error: Some(said) };
            }
            vec![]
        }
        _ => vec![],
    }
}

/// What the Clone sheet would do: the source and the folder it lands in.
pub fn plan(env: &Env, fields: &Fields) -> Option<(clone::Source, PathBuf)> {
    let source = clone::source(&fields.url.text)?;
    let dest = clone::destination(&env.expand(&fields.into.text), &source);
    Some((source, dest))
}

/// The work a sheet sends off the UI thread, and a running clone's progress.
#[derive(Default)]
pub struct SheetJobs {
    job: Latest<Result<Box<RepoEntry>, Refusal>>,
    progress: Arc<Mutex<Option<String>>>,
    cancel: Arc<AtomicBool>,
    waker: Option<Waker>,
    /// The progress the window shows, copied from `progress` when it changes.
    pub shown: Option<String>,
}

impl SheetJobs {
    pub fn set_waker(&mut self, waker: Waker) {
        self.job.set_waker(waker.clone());
        self.waker = Some(waker);
    }
}

impl App {
    /// Feeds `event` to the sheet and carries out what it asks for. `tx` is there when the
    /// event came from a control, so focus can move.
    pub fn sheet_event(&mut self, event: Event, tx: Option<&mut Tx>) {
        let into = self.clone_into();
        let mut tx = tx;
        for effect in step(&mut self.sheet, &mut self.fields, event, &self.env, &into) {
            match effect {
                Effect::Focus(key) => {
                    if let Some(tx) = tx.as_deref_mut() {
                        tx.focus(key, true);
                    }
                }
                Effect::Add(path) => self.sheet_jobs.job.request((self.env.clone(), path), |(env, path)| {
                    match workspace::add_repo(&env, &path) {
                        Ok(top) => open(&env, &top),
                        Err(AddError::NotARepo(folder)) => {
                            Err(Refusal::NotARepo(Box::new(Offer { survey: init::survey(&env, &folder), folder })))
                        }
                        Err(e) => Err(Refusal::Said(add_message(&env, &e))),
                    }
                }),
                Effect::Init { folder, plan } => {
                    self.sheet_jobs.job.request((self.env.clone(), folder, plan), |(env, folder, plan)| {
                        init::create(&env, &folder, &plan).map_err(|e| Refusal::Said(e.to_string()))?;
                        workspace::add_repo(&env, &folder).map_err(|e| Refusal::Said(add_message(&env, &e)))?;
                        open(&env, &folder)
                    });
                }
                Effect::Clone { url, dest } => self.start_clone(url, dest),
                Effect::StopClone => self.sheet_jobs.cancel.store(true, Ordering::Relaxed),
                Effect::StopSignIn => self.account_jobs.stop(),
                Effect::Open(entry) => self.open_repo(*entry),
            }
        }
    }

    fn start_clone(&mut self, url: String, dest: PathBuf) {
        self.sheet_jobs.cancel.store(false, Ordering::Relaxed);
        self.sheet_jobs.shown = None;
        if let Ok(mut p) = self.sheet_jobs.progress.lock() {
            *p = None;
        }
        let shared = (self.sheet_jobs.progress.clone(), self.sheet_jobs.cancel.clone(), self.sheet_jobs.waker.clone());
        self.sheet_jobs.job.request(
            (self.env.clone(), url, dest, shared),
            |(env, url, dest, (progress, cancel, waker))| {
                let mut heard = |step: &str| {
                    if let Ok(mut p) = progress.lock() {
                        *p = Some(step.to_owned());
                    }
                    if let Some(wake) = &waker {
                        wake();
                    }
                };
                clone::run(&env, &url, &dest, &mut heard, &|| cancel.load(Ordering::Relaxed))
                    .map_err(|e| Refusal::Said(clone_message(&env, &e)))?;
                workspace::add_repo(&env, &dest).map_err(|e| Refusal::Said(add_message(&env, &e)))?;
                open(&env, &dest)
            },
        );
    }

    /// Selects `entry`'s repository, adding it to the sidebar unless it's there already.
    fn open_repo(&mut self, entry: RepoEntry) {
        let i = match self.repos.iter().position(|r| r.summary.path == entry.summary.path) {
            Some(i) => i,
            None => {
                self.repos.push(entry);
                self.repos.len() - 1
            }
        };
        self.select_repo(i);
    }

    /// Where a clone goes unless the person says otherwise: beside the repository added last,
    /// else `~/code`.
    pub fn clone_into(&self) -> PathBuf {
        self.repos
            .iter()
            .rev()
            .find_map(|r| r.summary.path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| self.env.home.join("code"))
    }

    /// Takes a finished add or clone, and a clone's newest progress. True when the state changed.
    pub fn land_sheet(&mut self, tx: &mut Tx) -> bool {
        let progress = self.sheet_jobs.progress.lock().ok().and_then(|p| p.clone());
        let moved = progress != self.sheet_jobs.shown;
        self.sheet_jobs.shown = progress;
        let Some(result) = self.sheet_jobs.job.take() else { return moved };
        self.sheet_event(Event::Landed(result), Some(tx));
        true
    }
}

/// A repository just added, read for the sidebar.
fn open(env: &Env, top: &Path) -> Result<Box<RepoEntry>, Refusal> {
    let summary = RepoSummary::read(env, top).map_err(|e| Refusal::Said(e.to_string()))?;
    Ok(Box::new(RepoEntry { summary, pinned: false }))
}

fn add_message(env: &Env, e: &AddError) -> String {
    match e {
        AddError::Missing(path) => format!("Nothing is at {}", env.tilde(path)),
        AddError::NotAFolder(path) => format!("{} is a file, not a folder", env.tilde(path)),
        AddError::NotARepo(path) => format!("{} isn't a git repository", env.tilde(path)),
        AddError::Workspace(e) => e.to_string(),
    }
}

fn clone_message(env: &Env, e: &CloneError) -> String {
    match e {
        CloneError::Exists(path) => format!("{} already exists: pick another folder", env.tilde(path)),
        CloneError::Git(GitError::Failed { stderr, .. }) => Ops::reason(stderr).to_owned(),
        other => other.to_string(),
    }
}
