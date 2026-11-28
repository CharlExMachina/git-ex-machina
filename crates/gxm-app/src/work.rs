//! The Work view's state: every repository's open work, read off the UI thread, the row
//! picked, and the provider writes its buttons make (merging, running checks again).
//!
//! Reading is one latest-wins job over the whole workspace, each repository on its own thread:
//! git first (its checkouts), then its provider. What the window shows stays until a new read
//! lands. Writes are a small machine ([`Acting`], [`step`]): one at a time, and an answer only
//! counts for the attempt still waited on.

use std::path::{Path, PathBuf};

use gxm_core::env::Env;
use gxm_core::forge::{self, NotConnected};
use gxm_core::remote::Remote;
use gxm_core::work::{Entry, Item, Local, Open, Work};
use rune::machine::{Attempt, Effects, Seq};
use rune::{Latest, Tx, Waker};

use crate::state::{App, View};
use gxm_core::ops::Op;

/// What one repository's work read found.
#[derive(Clone, Debug)]
pub struct RepoWork {
    pub repo: PathBuf,
    /// None when even git couldn't read it.
    pub work: Option<Work>,
    /// Why there are no pull requests in it, when there aren't: no provider, no sign-in, an
    /// error from the provider or git.
    pub problem: Option<String>,
}

/// A write the Work view asks of the provider, on `repo`'s pull request `number`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    pub repo: PathBuf,
    pub number: u64,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Squash and merge, with the pull request's title and number as the commit's headline.
    Merge,
    /// Run this failed check again.
    Rerun(String),
    /// Run every failed check again.
    RerunFailed,
}

/// The write under way, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Acting {
    Idle,
    Running { attempt: Attempt, job: Job },
    Failed { job: Job, message: String },
}

#[derive(Debug)]
pub enum Event {
    Start(Job),
    Landed(Attempt, Result<(), String>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    Run(Attempt, Job),
    /// It went through: say so, and read the work again.
    Done(Job),
}

pub fn step(acting: &mut Acting, seq: &mut Seq, event: Event) -> Effects<Effect> {
    match event {
        Event::Start(_) if matches!(acting, Acting::Running { .. }) => Effects::none(),
        Event::Start(job) => {
            let attempt = seq.next();
            *acting = Acting::Running { attempt, job: job.clone() };
            Effects::one(Effect::Run(attempt, job))
        }
        Event::Landed(a, result) => match std::mem::replace(acting, Acting::Idle) {
            Acting::Running { attempt, job } if attempt == a => match result {
                Ok(()) => Effects::one(Effect::Done(job)),
                Err(message) => {
                    *acting = Acting::Failed { job, message };
                    Effects::none()
                }
            },
            other => {
                *acting = other;
                Effects::none()
            }
        },
    }
}

pub struct Model {
    /// Each repository's last read, in the workspace's order.
    pub repos: Vec<RepoWork>,
    /// The row picked; None shows the one stuck the longest.
    pub picked: Option<(PathBuf, Item)>,
    /// The list shows every repository's work, not only the selected one's.
    pub everywhere: bool,
    pub acting: Acting,
    seq: Seq,
    reads: Latest<Vec<RepoWork>>,
    writes: Latest<(Attempt, Result<(), String>)>,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            repos: Vec::new(),
            picked: None,
            everywhere: false,
            acting: Acting::Idle,
            seq: Seq::default(),
            reads: Latest::default(),
            writes: Latest::default(),
        }
    }
}

impl Model {
    pub fn set_waker(&self, waker: &Waker) {
        self.reads.set_waker(waker.clone());
        self.writes.set_waker(waker.clone());
    }

    pub fn busy(&self) -> bool {
        self.reads.is_pending() || self.writes.is_pending()
    }

    /// The last read of `repo`.
    pub fn of(&self, repo: &Path) -> Option<&RepoWork> {
        self.repos.iter().find(|r| r.repo == repo)
    }

    /// The write in progress on pull request `number`, if any.
    pub fn running(&self, number: u64) -> Option<&Action> {
        match &self.acting {
            Acting::Running { job, .. } if job.number == number => Some(&job.action),
            _ => None,
        }
    }
}

/// One row of the list, with the repository it's in.
#[derive(Clone, Copy, Debug)]
pub struct Row<'a> {
    pub repo: &'a Path,
    pub work: &'a Work,
    pub entry: &'a Entry,
}

impl Row<'_> {
    /// Its repository's name: the folder's.
    pub fn repo_name(&self) -> String {
        self.repo.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    }
}

/// Reads one repository's work: its checkouts, then, through its provider, its pull requests.
fn read_one(env: &Env, repo: &Path, origin: Option<&Remote>) -> RepoWork {
    let local = match Local::read(env, repo) {
        Ok(l) => l,
        Err(e) => return RepoWork { repo: repo.to_path_buf(), work: None, problem: Some(e.to_string()) },
    };
    let forge = match origin.map(|o| forge::connect(env, o)) {
        Some(Ok(f)) => Ok(f),
        Some(Err(why)) => Err(why),
        None => Err(NotConnected::NoProvider),
    };
    let (open, problem) = match forge {
        Ok(f) => match Open::read(env, &local, &*f) {
            Ok(open) => (Some(open), None),
            Err(e) => (None, Some(format!("Couldn't read pull requests from {}: {e}", f.provider().name()))),
        },
        Err(why) => (None, Some(why.message())),
    };
    RepoWork { repo: repo.to_path_buf(), work: Some(Work::new(local, open)), problem }
}

impl App {
    /// Reads every repository's work again, off the UI thread.
    pub fn read_work(&mut self) {
        let repos: Vec<(PathBuf, Option<Remote>)> =
            self.repos.iter().map(|r| (r.summary.path.clone(), r.summary.origin.clone())).collect();
        self.work.reads.request((self.env.clone(), repos), |(env, repos)| {
            std::thread::scope(|scope| {
                let reads: Vec<_> =
                    repos.iter().map(|(path, origin)| scope.spawn(|| read_one(&env, path, origin.as_ref()))).collect();
                reads
                    .into_iter()
                    .zip(&repos)
                    .map(|(h, (path, _))| {
                        h.join().unwrap_or_else(|_| RepoWork {
                            repo: path.clone(),
                            work: None,
                            problem: Some("the read panicked".to_owned()),
                        })
                    })
                    .collect()
            })
        });
    }

    /// The rows the list shows: the selected repository's, or every repository's, each
    /// section oldest first.
    pub fn work_rows(&self) -> Vec<Row<'_>> {
        let here = self.repo().map(|r| r.summary.path.as_path());
        let mut rows: Vec<Row<'_>> = self
            .work
            .repos
            .iter()
            .filter(|r| self.work.everywhere || Some(r.repo.as_path()) == here)
            .filter_map(|r| Some((r.repo.as_path(), r.work.as_ref()?)))
            .flat_map(|(repo, work)| work.entries.iter().map(move |entry| Row { repo, work, entry }))
            .collect();
        rows.sort_by_key(|r| (r.entry.section, r.entry.since));
        rows
    }

    /// The row the inspector shows: the one picked, else the one stuck the longest.
    pub fn work_row(&self) -> Option<Row<'_>> {
        let rows = self.work_rows();
        if let Some((repo, item)) = &self.work.picked
            && let Some(r) = rows.iter().find(|r| r.repo == repo && r.entry.item == *item)
        {
            return Some(*r);
        }
        rows.into_iter().min_by_key(|r| r.entry.since)
    }

    pub fn pick_work(&mut self, repo: PathBuf, item: Item) {
        self.work.picked = Some((repo, item));
    }

    /// Shows every repository's work at once, or only the selected one's.
    pub fn show_everywhere(&mut self, on: bool) {
        self.work.everywhere = on;
        self.work.picked = None;
        if on {
            self.show_view(View::Work);
        }
    }

    /// Opens `repo`'s pull request `number` for review: the repository, its history, and the
    /// review over it.
    pub fn review_pull(&mut self, repo: &Path, number: u64) {
        if self.repo().map(|r| r.summary.path.as_path()) != Some(repo)
            && let Some(i) = self.repos.iter().position(|r| r.summary.path == repo)
        {
            self.select_repo(i);
        }
        self.show_view(View::History);
        self.open_pull(number);
        self.open_cockpit();
    }

    /// Switches the main view. History is one repository's; Work leaves an open pull request.
    pub fn show_view(&mut self, view: View) {
        if view == View::Work && self.pull_view().is_some() {
            self.close_pull();
        }
        if view == View::History {
            self.work.everywhere = false;
        }
        self.view = view;
    }

    /// Starts a write on the provider.
    pub fn act(&mut self, job: Job) {
        let effects = step(&mut self.work.acting, &mut self.work.seq, Event::Start(job));
        self.carry_out(effects, None);
    }

    fn carry_out(&mut self, effects: Effects<Effect>, mut tx: Option<&mut Tx>) {
        for effect in effects {
            match effect {
                Effect::Run(attempt, job) => self.run_job(attempt, job),
                Effect::Done(job) => {
                    let said = match &job.action {
                        Action::Merge => format!("Merged #{}", job.number),
                        Action::Rerun(check) => format!("Running {check} again"),
                        Action::RerunFailed => format!("Running #{}'s failed checks again", job.number),
                    };
                    if let Some(tx) = tx.as_deref_mut() {
                        self.toasts.show(&said, tx);
                    }
                    // The merge is on the provider: fetch, so it reaches the history too.
                    if job.action == Action::Merge && self.repo().is_some_and(|r| r.summary.path == job.repo) {
                        self.start(Op::Fetch);
                    }
                    self.read_work();
                }
            }
        }
    }

    fn run_job(&mut self, attempt: Attempt, job: Job) {
        let origin = self.repos.iter().find(|r| r.summary.path == job.repo).and_then(|r| r.summary.origin.clone());
        let pull = self.work.of(&job.repo).and_then(|r| r.work.as_ref()?.pull(job.number).cloned());
        self.work.writes.request((self.env.clone(), origin, pull, job), move |(env, origin, pull, job)| {
            let result = (|| {
                let origin = origin.ok_or("this repository has no provider")?;
                let forge = forge::connect(&env, &origin).map_err(|why| why.message())?;
                let pull = pull.ok_or("the pull request isn't loaded yet")?;
                let done = match &job.action {
                    Action::Merge => {
                        let headline = format!("{} (#{})", pull.summary.title, pull.summary.number);
                        forge.squash_merge(&pull, &headline)
                    }
                    Action::Rerun(name) => match pull.checks.iter().find(|c| c.name == *name) {
                        Some(check) => forge.rerun_check(check),
                        None => return Err(format!("#{} has no check named {name}", job.number)),
                    },
                    Action::RerunFailed => forge.rerun_failed(&pull),
                };
                done.map_err(|e| e.to_string())
            })();
            (attempt, result)
        });
    }

    /// Takes the reads and writes that landed. True when the state changed.
    pub fn land_work(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(repos) = self.work.reads.take() {
            self.work.repos = repos;
            changed = true;
        }
        if let Some((attempt, result)) = self.work.writes.take() {
            let effects = step(&mut self.work.acting, &mut self.work.seq, Event::Landed(attempt, result));
            self.carry_out(effects, Some(tx));
            changed = true;
        }
        changed
    }

    /// Makes `path` (a worktree of the selected repository, or None for its main checkout)
    /// what Fetch, Pull, Push and History act on.
    pub fn pick_checkout(&mut self, path: Option<PathBuf>) {
        self.checkout_menu = false;
        let main = self.repo().map(|r| r.summary.path.clone());
        let path = path.filter(|p| Some(p) != main.as_ref());
        if path == self.checkout {
            return;
        }
        self.checkout = path;
        self.checkout_summary = None;
        if let Some(i) = self.selected_repo {
            self.close_pull();
            self.selection = crate::state::Selection::default();
            self.reload_checkout(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(action: Action) -> Job {
        Job { repo: PathBuf::from("/r"), number: 118, action }
    }

    #[test]
    fn one_write_at_a_time_and_only_its_answer_counts() {
        let (mut acting, mut seq) = (Acting::Idle, Seq::default());
        let effects: Vec<Effect> =
            step(&mut acting, &mut seq, Event::Start(job(Action::RerunFailed))).into_iter().collect();
        let [Effect::Run(a, _)] = effects.as_slice() else { panic!("{effects:?}") };
        let a = *a;
        assert!(step(&mut acting, &mut seq, Event::Start(job(Action::Merge))).is_empty(), "one at a time");
        // An answer for another attempt changes nothing.
        let mut other = Seq::default();
        other.next();
        let stale = other.next();
        assert!(step(&mut acting, &mut seq, Event::Landed(stale, Ok(()))).is_empty());
        assert!(matches!(acting, Acting::Running { .. }));
        let done: Vec<Effect> = step(&mut acting, &mut seq, Event::Landed(a, Ok(()))).into_iter().collect();
        assert_eq!(done, [Effect::Done(job(Action::RerunFailed))]);
        assert_eq!(acting, Acting::Idle);

        let effects: Vec<Effect> = step(&mut acting, &mut seq, Event::Start(job(Action::Merge))).into_iter().collect();
        let [Effect::Run(b, _)] = effects.as_slice() else { panic!("{effects:?}") };
        step(&mut acting, &mut seq, Event::Landed(*b, Err("Head branch was modified".to_owned())));
        assert_eq!(acting, Acting::Failed { job: job(Action::Merge), message: "Head branch was modified".to_owned() });
    }
}
