//! The sheets over the window: Add (a repository already on disk) and Clone (one from a
//! remote, over SSH). Each is a small machine: editing, then working off the UI thread, then
//! closed with the repository open in the window, or back to editing with what went wrong. A
//! clone can be cancelled while it runs.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use gxm_core::clone::{self, CloneError};
use gxm_core::env::Env;
use gxm_core::git::GitError;
use gxm_core::repo::RepoSummary;
use gxm_core::workspace::{self, AddError};
use rune::{Latest, TextEdit, Tx, Waker};

use crate::keys;
use crate::ops::Ops;
use crate::state::{App, RepoEntry};

pub enum Sheet {
    Closed,
    Add(Phase),
    Clone(Phase),
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
}

impl Default for Fields {
    fn default() -> Self {
        Fields { path: TextEdit::new(""), url: TextEdit::new(""), into: TextEdit::new("") }
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
    Landed(Result<Box<RepoEntry>, String>),
}

pub enum Effect {
    Focus(&'static str),
    Add(PathBuf),
    Clone { url: String, dest: PathBuf },
    StopClone,
    Open(Box<RepoEntry>),
}

/// One transition. `env` reads typed paths; `into` is where a new clone goes by default.
pub fn step(sheet: &mut Sheet, fields: &mut Fields, event: Event, env: &Env, into: &Path) -> Vec<Effect> {
    let editing = Phase::Editing { error: None };
    let fail = |why: &str| Phase::Editing { error: Some(why.to_owned()) };
    let working = matches!(sheet, Sheet::Add(p) | Sheet::Clone(p) if !p.editing());
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
        (Sheet::Add(phase) | Sheet::Clone(phase), Event::Landed(Err(error))) if working => {
            *phase = Phase::Editing { error: Some(error) };
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
    job: Latest<Result<Box<RepoEntry>, String>>,
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
                    let top = workspace::add_repo(&env, &path).map_err(|e| add_message(&env, &e))?;
                    open(&env, &top)
                }),
                Effect::Clone { url, dest } => self.start_clone(url, dest),
                Effect::StopClone => self.sheet_jobs.cancel.store(true, Ordering::Relaxed),
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
                    .map_err(|e| clone_message(&env, &e))?;
                workspace::add_repo(&env, &dest).map_err(|e| add_message(&env, &e))?;
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
fn open(env: &Env, top: &Path) -> Result<Box<RepoEntry>, String> {
    let summary = RepoSummary::read(env, top).map_err(|e| e.to_string())?;
    Ok(Box::new(RepoEntry { summary, pinned: false }))
}

fn add_message(env: &Env, e: &AddError) -> String {
    match e {
        AddError::Missing(path) => format!("Nothing is at {}", env.tilde(path)),
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
