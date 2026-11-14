//! Pull requests from the selected repository's provider, fetched off the UI thread.
//!
//! One machine ([`Machine`], [`step`]) follows the three conversations with the provider: the
//! repository's index (which pull requests exist and which commits are theirs), one pull
//! request in full, and sending a review. Answers carry the [`Attempt`] that asked, so an answer
//! for a repository the person already left changes nothing. The last answers are cached on
//! disk, so a pull request shows at once and refreshes behind the scenes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gxm_core::env::Env;
use gxm_core::forge::local::{self, Changes};
use gxm_core::forge::model::{Pull, PullSummary, Submission, User, Verdict};
use gxm_core::forge::{self, Forge, NotConnected, RepoKey, cache};
use gxm_core::remote::Remote;
use rune::Latest;
use rune::machine::{Attempt, Effects, Seq};
use serde::{Deserialize, Serialize};

/// Which pull requests a repository has, who's asking, and which commits are whose.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub viewer: Option<User>,
    pub pulls: Vec<PullSummary>,
    pub by_commit: HashMap<String, u64>,
}

/// A pull request in full: what the provider says, and what git says it changes (None until
/// git has been asked; an error when its commits aren't in the repository).
#[derive(Clone, Debug)]
pub struct Details {
    pub pull: Pull,
    pub changes: Option<Result<Changes, String>>,
}

/// The selected repository's provider.
pub enum Link {
    NoRepo,
    NotConnected(NotConnected),
    Connected { forge: Arc<dyn Forge>, key: RepoKey, repo: PathBuf },
}

/// A fetch: idle (with or without an answer), waiting for `Attempt`, or failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fetch {
    Idle,
    Loading(Attempt),
    Failed(String),
}

/// Sending a review: it goes, then the pull request is read again so the screen shows the
/// review as the provider has it, then it's sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sending {
    Idle,
    Sending { attempt: Attempt, number: u64, verdict: Verdict },
    Refreshing { attempt: Attempt, number: u64, verdict: Verdict },
    Sent { number: u64, verdict: Verdict },
    Failed { number: u64, message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    seq: Seq,
    pub index: Fetch,
    pub details: Fetch,
    /// The pull request `details` is about.
    pub details_for: Option<u64>,
    pub sending: Sending,
}

impl Default for Machine {
    fn default() -> Self {
        Machine { seq: Seq::default(), index: Fetch::Idle, details: Fetch::Idle, details_for: None, sending: Sending::Idle }
    }
}

#[derive(Debug)]
pub enum Event {
    /// A repository was opened: start over and read its index.
    Open,
    IndexLanded(Attempt, Result<Index, String>),
    /// Read this pull request in full.
    Want(u64),
    DetailsLanded(Attempt, u64, Result<Details, String>),
    Submit(u64, Verdict),
    Submitted(Attempt, u64, Result<(), String>),
}

#[derive(Debug)]
pub enum Effect {
    FetchIndex(Attempt),
    FetchDetails(Attempt, u64),
    Send(Attempt, u64),
    KeepIndex(Index),
    KeepDetails(u64, Details),
    /// The review is with the provider: its drafts can go.
    ClearDrafts(u64),
}

pub fn step(m: &mut Machine, event: Event) -> Effects<Effect> {
    match event {
        Event::Open => {
            let a = m.seq.next();
            *m = Machine { seq: m.seq, index: Fetch::Loading(a), ..Machine::default() };
            Effects::one(Effect::FetchIndex(a))
        }
        Event::IndexLanded(a, result) if m.index == Fetch::Loading(a) => match result {
            Ok(index) => {
                m.index = Fetch::Idle;
                Effects::one(Effect::KeepIndex(index))
            }
            Err(e) => {
                m.index = Fetch::Failed(e);
                Effects::none()
            }
        },
        Event::Want(n) if matches!(m.details, Fetch::Loading(_)) && m.details_for == Some(n) => Effects::none(),
        Event::Want(n) => {
            let a = m.seq.next();
            (m.details, m.details_for) = (Fetch::Loading(a), Some(n));
            Effects::one(Effect::FetchDetails(a, n))
        }
        Event::DetailsLanded(a, n, result) if m.details == Fetch::Loading(a) => {
            if let Sending::Refreshing { attempt, number, verdict } = m.sending
                && attempt == a
            {
                m.sending = Sending::Sent { number, verdict };
            }
            match result {
                Ok(d) => {
                    m.details = Fetch::Idle;
                    Effects::one(Effect::KeepDetails(n, d))
                }
                Err(e) => {
                    m.details = Fetch::Failed(e);
                    Effects::none()
                }
            }
        }
        Event::Submit(..) if matches!(m.sending, Sending::Sending { .. } | Sending::Refreshing { .. }) => {
            Effects::none()
        }
        Event::Submit(number, verdict) => {
            let attempt = m.seq.next();
            m.sending = Sending::Sending { attempt, number, verdict };
            Effects::one(Effect::Send(attempt, number))
        }
        Event::Submitted(a, n, result) => match m.sending {
            Sending::Sending { attempt, number, verdict } if attempt == a && number == n => match result {
                Ok(()) => {
                    let b = m.seq.next();
                    (m.details, m.details_for) = (Fetch::Loading(b), Some(n));
                    m.sending = Sending::Refreshing { attempt: b, number: n, verdict };
                    Effects::two(Effect::ClearDrafts(n), Effect::FetchDetails(b, n))
                }
                Err(message) => {
                    m.sending = Sending::Failed { number: n, message };
                    Effects::none()
                }
            },
            _ => Effects::none(),
        },
        // An answer to a question no longer asked.
        Event::IndexLanded(..) | Event::DetailsLanded(..) => Effects::none(),
    }
}

/// The machine, what it has learned, and the jobs carrying out its effects.
pub struct Pulls {
    pub link: Link,
    pub index: Option<Index>,
    pub details: HashMap<u64, Details>,
    pub machine: Machine,
    index_job: Latest<(Attempt, Result<Index, String>)>,
    details_job: Latest<(Attempt, u64, Result<Details, String>)>,
    send_job: Latest<(Attempt, u64, Result<(), String>)>,
}

impl Default for Pulls {
    fn default() -> Self {
        Pulls {
            link: Link::NoRepo,
            index: None,
            details: HashMap::new(),
            machine: Machine::default(),
            index_job: Latest::default(),
            details_job: Latest::default(),
            send_job: Latest::default(),
        }
    }
}

impl Pulls {
    pub fn set_waker(&self, waker: &rune::Waker) {
        self.index_job.set_waker(waker.clone());
        self.details_job.set_waker(waker.clone());
        self.send_job.set_waker(waker.clone());
    }

    /// Connects to `repo`'s provider and reads its index, showing the cached one meanwhile.
    pub fn open(&mut self, env: &Env, repo: PathBuf, origin: Option<&Remote>) {
        self.details.clear();
        self.link = match origin {
            None => Link::NotConnected(NotConnected::NoProvider),
            Some(remote) => match forge::connect(env, remote) {
                Ok(forge) => Link::Connected { forge, key: RepoKey::of(remote), repo },
                Err(why) => Link::NotConnected(why),
            },
        };
        self.index = match &self.link {
            Link::Connected { key, .. } => cache::load(env, key, "pulls"),
            _ => None,
        };
        if matches!(self.link, Link::Connected { .. }) {
            let effects = step(&mut self.machine, Event::Open);
            self.apply(env, effects, None);
        } else {
            self.machine = Machine::default();
        }
    }

    /// Reads pull request `n` in full: from the cache at once if it's there, then from the
    /// provider. `refresh` asks the provider even when it's already known.
    pub fn want(&mut self, env: &Env, n: u64, refresh: bool) {
        let Link::Connected { key, .. } = &self.link else { return };
        if !self.details.contains_key(&n)
            && let Some(pull) = cache::load::<Pull>(env, key, &format!("pull-{n}"))
        {
            self.details.insert(n, Details { pull, changes: None });
        }
        let known = self.details.get(&n).is_some_and(|d| d.changes.is_some());
        if refresh || !known {
            let effects = step(&mut self.machine, Event::Want(n));
            self.apply(env, effects, None);
        }
    }

    /// Sends `submission` as the review of pull request `n`.
    pub fn submit(&mut self, env: &Env, n: u64, submission: Submission) {
        let verdict = submission.verdict;
        let effects = step(&mut self.machine, Event::Submit(n, verdict));
        self.apply(env, effects, Some(submission));
    }

    /// Takes the answers that landed. Returns the pull requests whose drafts were sent, and
    /// whether anything changed.
    pub fn land(&mut self, env: &Env) -> (Vec<u64>, bool) {
        let mut events = Vec::new();
        if let Some((a, r)) = self.index_job.take() {
            events.push(Event::IndexLanded(a, r));
        }
        if let Some((a, n, r)) = self.details_job.take() {
            events.push(Event::DetailsLanded(a, n, r));
        }
        if let Some((a, n, r)) = self.send_job.take() {
            events.push(Event::Submitted(a, n, r));
        }
        let changed = !events.is_empty();
        let mut sent = Vec::new();
        for e in events {
            let effects = step(&mut self.machine, e);
            sent.extend(effects.into_iter().filter_map(|e| self.carry_out(env, e, None)));
        }
        (sent, changed)
    }

    fn apply(&mut self, env: &Env, effects: Effects<Effect>, mut submission: Option<Submission>) {
        for e in effects {
            self.carry_out(env, e, submission.take());
        }
    }

    /// Carries out one effect; returns the pull request whose drafts can go.
    fn carry_out(&mut self, env: &Env, effect: Effect, submission: Option<Submission>) -> Option<u64> {
        let Link::Connected { forge, key, repo } = &self.link else { return None };
        let (forge, key, repo, env) = (Arc::clone(forge), key.clone(), repo.clone(), env.clone());
        match effect {
            Effect::FetchIndex(a) => self.index_job.request((), move |()| (a, fetch_index(&env, &*forge, &key, &repo))),
            Effect::FetchDetails(a, n) => {
                self.details_job.request((), move |()| (a, n, fetch_details(&env, &*forge, &key, &repo, n)));
            }
            Effect::Send(a, n) => {
                let pull = self.details.get(&n).map(|d| d.pull.clone());
                let job = move |()| {
                    let result = match (pull, submission) {
                        (Some(pull), Some(review)) => forge.submit(&pull, &review).map_err(|e| e.to_string()),
                        _ => Err("the pull request isn't loaded yet".to_owned()),
                    };
                    (a, n, result)
                };
                self.send_job.request((), job);
            }
            Effect::KeepIndex(index) => self.index = Some(index),
            Effect::KeepDetails(n, d) => {
                self.details.insert(n, d);
            }
            Effect::ClearDrafts(n) => return Some(n),
        }
        None
    }

    /// The pull request commit `id` belongs to.
    pub fn of_commit(&self, id: &str) -> Option<&PullSummary> {
        let index = self.index.as_ref()?;
        let n = index.by_commit.get(id)?;
        index.pulls.iter().find(|p| p.number == *n)
    }

    /// The person the provider knows the token as.
    pub fn viewer(&self) -> Option<&User> {
        self.index.as_ref()?.viewer.as_ref()
    }

    /// Why pull requests can't be shown, when they can't.
    pub fn problem(&self) -> Option<String> {
        match (&self.link, &self.machine.index) {
            (Link::NotConnected(why), _) => Some(why.message()),
            (Link::Connected { forge, .. }, Fetch::Failed(e)) => {
                Some(format!("Couldn't read pull requests from {}: {e}", forge.provider().name()))
            }
            _ => None,
        }
    }
}

fn fetch_index(env: &Env, forge: &dyn Forge, key: &RepoKey, repo: &std::path::Path) -> Result<Index, String> {
    let viewer = forge.viewer().map_err(|e| e.to_string())?;
    let pulls = forge.pulls().map_err(|e| e.to_string())?;
    let by_commit = local::commit_pulls(env, repo, &pulls);
    let index = Index { viewer: Some(viewer), pulls, by_commit };
    cache::save(env, key, "pulls", &index);
    Ok(index)
}

fn fetch_details(
    env: &Env,
    forge: &dyn Forge,
    key: &RepoKey,
    repo: &std::path::Path,
    n: u64,
) -> Result<Details, String> {
    let pull = forge.pull(n).map_err(|e| e.to_string())?;
    cache::save(env, key, &format!("pull-{n}"), &pull);
    let changes = local::changes(env, repo, &pull.summary).map_err(|e| e.to_string());
    Ok(Details { pull, changes: Some(changes) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_land_only_for_the_question_still_asked() {
        let mut m = Machine::default();
        let first = match step(&mut m, Event::Open).into_iter().next() {
            Some(Effect::FetchIndex(a)) => a,
            other => panic!("{other:?}"),
        };
        // The person opens another repository before the first answer lands.
        step(&mut m, Event::Open);
        assert!(step(&mut m, Event::IndexLanded(first, Ok(Index::default()))).is_empty());
        assert!(matches!(m.index, Fetch::Loading(_)));
        // Asking twice for the same pull request asks once.
        assert_eq!(step(&mut m, Event::Want(131)).into_iter().count(), 1);
        assert!(step(&mut m, Event::Want(131)).is_empty());
    }

    #[test]
    fn a_sent_review_is_sent_once_the_pull_request_is_read_again() {
        let mut m = Machine::default();
        let Some(Effect::Send(a, 131)) = step(&mut m, Event::Submit(131, Verdict::Approve)).into_iter().next() else {
            panic!("no send")
        };
        assert!(step(&mut m, Event::Submit(131, Verdict::Approve)).is_empty(), "one review at a time");
        let effects: Vec<Effect> = step(&mut m, Event::Submitted(a, 131, Ok(()))).into_iter().collect();
        let [Effect::ClearDrafts(131), Effect::FetchDetails(b, 131)] = effects.as_slice() else { panic!("{effects:?}") };
        assert!(matches!(m.sending, Sending::Refreshing { .. }));
        step(&mut m, Event::DetailsLanded(*b, 131, Err("offline".to_owned())));
        assert_eq!(m.sending, Sending::Sent { number: 131, verdict: Verdict::Approve });

        let mut m = Machine::default();
        let Some(Effect::Send(a, _)) = step(&mut m, Event::Submit(131, Verdict::Comment)).into_iter().next() else {
            panic!("no send")
        };
        step(&mut m, Event::Submitted(a, 131, Err("422".to_owned())));
        assert_eq!(m.sending, Sending::Failed { number: 131, message: "422".to_owned() });
    }
}
