//! Resolving as a machine: one [`Flow`], and one [`step`] that turns what happened (the person
//! pressed something, git or the agent answered) into [`Effect`]s for the window to carry out.
//!
//! The phases: merging (fetch, merge, read every conflict), resolving (by side, by hand, or by
//! the agent), then committing and pushing, or aborting. Each answer from off the UI thread
//! carries the attempt that asked for it, and counts only while that attempt is the one waited
//! on.

use super::merge::NotPushed;
use super::{By, File, Merge, Resolution, Side, Target, markers, prompt};
use crate::harness::Ending;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Nothing to resolve: the window is closed, or was never opened.
    Closed,
    /// Fetching the base, merging it, reading the conflicts.
    Merging(u64),
    /// The merge wouldn't start, and why.
    Failed(String),
    Resolving,
    Committing(u64),
    /// The merge is committed here; its push failed (`None`) or is being tried again.
    Committed(Option<u64>),
    Aborting(u64),
}

/// The coding agent, while it's asked to help.
#[derive(Clone, Debug, PartialEq)]
pub enum Agent {
    Idle,
    /// Its process is starting.
    Starting {
        name: String,
        conflicts: usize,
    },
    /// At work since `since` (the window's clock), its latest event `last`.
    Working {
        name: String,
        conflicts: usize,
        since: f64,
        last: String,
    },
    /// Done; the files are read again to see what it resolved.
    Rereading {
        attempt: u64,
        name: String,
        conflicts: usize,
        ending: Ending,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Flow {
    pub phase: Phase,
    pub target: Option<Target>,
    pub merge: Option<Merge>,
    /// The conflict shown, counting across files.
    pub current: usize,
    /// What went wrong last.
    pub error: Option<String>,
    pub agent: Agent,
    /// What the agent's last run came to.
    pub agent_said: Option<String>,
    attempts: u64,
}

impl Default for Flow {
    fn default() -> Self {
        Flow {
            phase: Phase::Closed,
            target: None,
            merge: None,
            current: 0,
            error: None,
            agent: Agent::Idle,
            agent_said: None,
            attempts: 0,
        }
    }
}

#[derive(Debug)]
pub enum Event {
    /// Resolve this pull request's conflicts: start, or show the merge already under way.
    Open(Target),
    Merged(u64, Result<Merge, String>),
    /// Show conflict `n`.
    Go(usize),
    /// The next conflict (`1`) or the previous one (`-1`), across files, wrapping.
    Step(isize),
    /// Resolve the conflict shown with a side.
    Take(Side),
    /// Resolve it with what the result pane holds, and show the next open one.
    Resolve(String),
    /// Back to its markers.
    Reset,
    Wrote(Result<(), String>),
    Commit,
    Committed(u64, Result<String, NotPushed>),
    /// Try the push again.
    Push,
    Pushed(u64, Result<(), String>),
    Abort,
    Aborted(u64, Result<(), String>),
    /// The person dismissed the window.
    Close,
    /// Ask the agent called `name` to resolve what's open.
    Ask(String),
    /// Its process started (at the window's clock), or didn't.
    AgentStarted(Result<f64, String>),
    /// One of its events, on one line.
    AgentSaid(String),
    AgentEnded(Ending),
    Reread(u64, Result<Vec<File>, String>),
    /// The person stops the agent.
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Bring the window up.
    Show,
    Merge(u64, Target),
    /// Write these files as they now stand.
    Write(Target, Vec<File>),
    CommitAndPush(u64, Target, Vec<File>),
    Push(u64, Target),
    /// Abort the merge, back to this commit.
    Abort(u64, Target, String),
    /// Start the agent in the worktree with this prompt.
    StartAgent(Target, String),
    StopAgent,
    Reread(u64, Target, Vec<File>),
    /// The conflict shown, or its resolution, changed: show it afresh in the result pane.
    Refill,
    Close,
    /// Tell the main window: say this, and read the work again.
    Done(String),
}

impl Flow {
    /// The conflict shown can be changed by hand: nothing else is writing the files.
    pub fn editable(&self) -> bool {
        self.phase == Phase::Resolving && self.agent == Agent::Idle
    }

    /// Every conflict resolved, nothing left outside: the merge can be committed.
    pub fn committable(&self) -> bool {
        self.editable() && self.merge.as_ref().is_some_and(|m| m.open() == 0 && m.elsewhere.is_empty())
    }

    /// Something runs off the UI thread for the machine.
    pub fn waiting(&self) -> bool {
        matches!(self.phase, Phase::Merging(_) | Phase::Committing(_) | Phase::Aborting(_) | Phase::Committed(Some(_)))
            || matches!(self.agent, Agent::Starting { .. } | Agent::Rereading { .. })
    }

    fn attempt(&mut self) -> u64 {
        self.attempts += 1;
        self.attempts
    }

    fn write(&self) -> Vec<Effect> {
        match (&self.target, &self.merge) {
            (Some(t), Some(m)) => vec![Effect::Write(t.clone(), m.files.clone()), Effect::Refill],
            _ => Vec::new(),
        }
    }
}

/// One transition.
pub fn step(f: &mut Flow, event: Event) -> Vec<Effect> {
    match event {
        Event::Open(target) => {
            let busy = !matches!(f.phase, Phase::Closed | Phase::Failed(_));
            if busy {
                return vec![Effect::Show];
            }
            let a = f.attempt();
            *f = Flow {
                phase: Phase::Merging(a),
                target: Some(target.clone()),
                attempts: f.attempts,
                ..Flow::default()
            };
            vec![Effect::Show, Effect::Merge(a, target)]
        }
        Event::Merged(a, result) if f.phase == Phase::Merging(a) => match result {
            Ok(m) => {
                f.current = m.next_open(0).unwrap_or(0);
                f.merge = Some(m);
                f.phase = Phase::Resolving;
                vec![Effect::Refill]
            }
            Err(why) => {
                f.phase = Phase::Failed(why);
                Vec::new()
            }
        },
        Event::Go(n) => {
            let total = f.merge.as_ref().map_or(0, Merge::total);
            if n >= total {
                return Vec::new();
            }
            f.current = n;
            vec![Effect::Refill]
        }
        Event::Step(by) => {
            let total = f.merge.as_ref().map_or(0, Merge::total) as isize;
            if total == 0 {
                return Vec::new();
            }
            f.current = (f.current as isize + by).rem_euclid(total) as usize;
            vec![Effect::Refill]
        }
        Event::Take(side) => {
            if !f.editable() {
                return Vec::new();
            }
            let n = f.current;
            let Some(c) = f.merge.as_mut().and_then(|m| m.conflict_mut(n)) else { return Vec::new() };
            c.resolution = Some(Resolution { text: c.side(side), by: By::Side(side) });
            f.error = None;
            f.write()
        }
        Event::Resolve(text) => {
            if !f.editable() {
                return Vec::new();
            }
            if markers::has_markers(&text) {
                f.error = Some("The result still has conflict markers.".to_owned());
                return Vec::new();
            }
            let n = f.current;
            let Some(m) = f.merge.as_mut() else { return Vec::new() };
            let Some(c) = m.conflict_mut(n) else { return Vec::new() };
            let text = super::ended(&text);
            let by = [Side::Yours, Side::Base, Side::Both]
                .into_iter()
                .find(|s| c.side(*s) == text)
                .map_or(By::Hand, By::Side);
            c.resolution = Some(Resolution { text, by });
            f.current = m.next_open(n + 1).unwrap_or(n);
            f.error = None;
            f.write()
        }
        Event::Reset => {
            if !f.editable() {
                return Vec::new();
            }
            let n = f.current;
            let Some(c) = f.merge.as_mut().and_then(|m| m.conflict_mut(n)) else { return Vec::new() };
            c.resolution = None;
            f.write()
        }
        Event::Wrote(result) => {
            if let Err(why) = result {
                f.error = Some(format!("Couldn't write the file: {why}"));
            }
            Vec::new()
        }
        Event::Commit => {
            if !f.committable() {
                return Vec::new();
            }
            let (Some(t), Some(files)) = (f.target.clone(), f.merge.as_ref().map(|m| m.files.clone())) else {
                return Vec::new();
            };
            let a = f.attempt();
            f.phase = Phase::Committing(a);
            f.error = None;
            vec![Effect::CommitAndPush(a, t, files)]
        }
        Event::Committed(a, result) if f.phase == Phase::Committing(a) => match result {
            Ok(_) => finish(f),
            Err(NotPushed::Commit(why)) => {
                f.phase = Phase::Resolving;
                f.error = Some(why);
                Vec::new()
            }
            Err(NotPushed::Push(why)) => {
                f.phase = Phase::Committed(None);
                f.error = Some(format!("The merge is committed, but the push failed: {why}"));
                Vec::new()
            }
        },
        Event::Push if f.phase == Phase::Committed(None) => {
            let Some(t) = f.target.clone() else { return Vec::new() };
            let a = f.attempt();
            f.phase = Phase::Committed(Some(a));
            f.error = None;
            vec![Effect::Push(a, t)]
        }
        Event::Pushed(a, result) if f.phase == Phase::Committed(Some(a)) => match result {
            Ok(()) => finish(f),
            Err(why) => {
                f.phase = Phase::Committed(None);
                f.error = Some(format!("The merge is committed, but the push failed: {why}"));
                Vec::new()
            }
        },
        Event::Abort => {
            if !f.editable() {
                return Vec::new();
            }
            let (Some(t), Some(head)) = (f.target.clone(), f.merge.as_ref().map(|m| m.head.clone())) else {
                return Vec::new();
            };
            let a = f.attempt();
            f.phase = Phase::Aborting(a);
            f.error = None;
            vec![Effect::Abort(a, t, head)]
        }
        Event::Aborted(a, result) if f.phase == Phase::Aborting(a) => match result {
            Ok(()) => {
                let name = f.target.as_ref().map(|t| t.worktree_name.clone()).unwrap_or_default();
                *f = Flow { attempts: f.attempts, ..Flow::default() };
                vec![Effect::Close, Effect::Done(format!("Merge aborted: {name} is as it was"))]
            }
            Err(why) => {
                f.phase = Phase::Resolving;
                f.error = Some(format!("Couldn't abort the merge: {why}"));
                Vec::new()
            }
        },
        Event::Close => {
            if matches!(f.phase, Phase::Failed(_)) {
                f.phase = Phase::Closed;
            }
            vec![Effect::Close]
        }
        Event::Ask(name) => {
            let open = f.merge.as_ref().map_or(0, Merge::open);
            if !f.editable() || open == 0 {
                return Vec::new();
            }
            let (Some(t), Some(m)) = (f.target.clone(), f.merge.as_ref()) else { return Vec::new() };
            let text = prompt::build(&t, m);
            f.agent = Agent::Starting { name, conflicts: open };
            f.agent_said = None;
            vec![Effect::StartAgent(t, text)]
        }
        Event::AgentStarted(result) => {
            let Agent::Starting { name, conflicts } = f.agent.clone() else { return Vec::new() };
            match result {
                Ok(since) => f.agent = Agent::Working { name, conflicts, since, last: String::new() },
                Err(why) => {
                    f.agent = Agent::Idle;
                    f.agent_said = Some(format!("{name} couldn't start: {why}"));
                }
            }
            Vec::new()
        }
        Event::AgentSaid(line) => {
            if let Agent::Working { last, .. } = &mut f.agent
                && !line.trim().is_empty()
            {
                *last = line;
            }
            Vec::new()
        }
        Event::AgentEnded(ending) => {
            let Agent::Working { name, conflicts, .. } = f.agent.clone() else { return Vec::new() };
            let (Some(t), Some(files)) = (f.target.clone(), f.merge.as_ref().map(|m| m.files.clone())) else {
                return Vec::new();
            };
            let attempt = f.attempt();
            f.agent = Agent::Rereading { attempt, name, conflicts, ending };
            vec![Effect::Reread(attempt, t, files)]
        }
        Event::Reread(a, result) => {
            let Agent::Rereading { attempt, name, conflicts, ending } = f.agent.clone() else { return Vec::new() };
            if attempt != a {
                return Vec::new();
            }
            f.agent = Agent::Idle;
            let before = f.merge.as_ref().map_or(0, Merge::open);
            match result {
                Ok(files) => {
                    if let Some(m) = f.merge.as_mut() {
                        m.files = files;
                        f.current = m.next_open(f.current).unwrap_or(f.current);
                    }
                }
                Err(why) => f.error = Some(format!("Couldn't read the files again: {why}")),
            }
            let after = f.merge.as_ref().map_or(0, Merge::open);
            let solved = before.saturating_sub(after);
            let of = |n: usize| if n == 1 { "1 conflict".to_owned() } else { format!("{n} conflicts") };
            f.agent_said = Some(match ending {
                Ending::Completed => {
                    format!("{name} finished: it resolved {solved} of {conflicts}. Review what it wrote, then commit.")
                }
                Ending::Stopped => format!("You stopped {name}. It had resolved {}.", of(solved)),
                Ending::Failed(why) => format!("{name} failed: {why}"),
            });
            vec![Effect::Refill]
        }
        Event::Stop => match f.agent {
            Agent::Starting { .. } | Agent::Working { .. } => vec![Effect::StopAgent],
            _ => Vec::new(),
        },
        // An answer for an attempt no longer waited on.
        Event::Merged(..) | Event::Committed(..) | Event::Push | Event::Pushed(..) | Event::Aborted(..) => Vec::new(),
    }
}

/// Committed and pushed: the window closes, and the main window reads the work again.
fn finish(f: &mut Flow) -> Vec<Effect> {
    let said =
        f.target.as_ref().map(|t| format!("Merged {} into {} and pushed it", t.base, t.branch)).unwrap_or_default();
    *f = Flow { attempts: f.attempts, ..Flow::default() };
    vec![Effect::Close, Effect::Done(said)]
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn target() -> Target {
        Target {
            repo: PathBuf::from("/code/atlas-web"),
            worktree: PathBuf::from("/worktrees/avatar-loading"),
            worktree_name: "avatar-loading".to_owned(),
            branch: "perf/avatar-loading".to_owned(),
            base: "main".to_owned(),
            remote: "origin".to_owned(),
            number: 118,
            title: "Load avatars in parallel".to_owned(),
        }
    }

    fn merge() -> Merge {
        let two = "a\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> origin/main\nb\n<<<<<<< HEAD\nmine\n=======\n>>>>>>> origin/main\n";
        let one = "<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> origin/main\n";
        Merge {
            head: "abc".to_owned(),
            base_commit: "def".to_owned(),
            files: vec![File::parse("src/a.rs", two), File::parse("src/b.rs", one)],
            regenerated: Vec::new(),
            lockfiles: Vec::new(),
            elsewhere: Vec::new(),
        }
    }

    /// A flow resolving `merge()`.
    fn resolving() -> Flow {
        let mut f = Flow::default();
        let effects = step(&mut f, Event::Open(target()));
        assert_eq!(effects, [Effect::Show, Effect::Merge(1, target())]);
        assert_eq!(step(&mut f, Event::Open(target())), [Effect::Show], "one merge at a time");
        assert_eq!(step(&mut f, Event::Merged(1, Ok(merge()))), [Effect::Refill]);
        f
    }

    fn open(f: &Flow) -> usize {
        f.merge.as_ref().map_or(0, Merge::open)
    }

    #[test]
    fn walking_and_resolving_writes_each_change_and_commits_once_none_are_left() {
        let mut f = resolving();
        assert_eq!((f.phase.clone(), f.current, open(&f)), (Phase::Resolving, 0, 3));
        step(&mut f, Event::Step(-1));
        assert_eq!(f.current, 2, "back from the first wraps to the last, across files");
        step(&mut f, Event::Step(1));
        assert_eq!(f.current, 0);
        // A side: written at once, and shown.
        let effects = step(&mut f, Event::Take(Side::Base));
        assert!(
            matches!(effects.as_slice(), [Effect::Write(_, files), Effect::Refill] if files[0].render().starts_with("a\ntheirs\nb\n"))
        );
        assert_eq!(step(&mut f, Event::Commit), [], "two still open");
        // By hand: on to the next open one.
        step(&mut f, Event::Go(1));
        assert!(step(&mut f, Event::Resolve("<<<<<<< x\n".to_owned())).is_empty());
        assert!(f.error.is_some(), "markers are refused");
        step(&mut f, Event::Resolve("mine, kept".to_owned()));
        assert_eq!(f.current, 2);
        let c = f.merge.as_ref().and_then(|m| m.conflict(1)).map(|(_, c)| c.resolution.clone());
        assert_eq!(c, Some(Some(Resolution { text: "mine, kept\n".to_owned(), by: By::Hand })));
        step(&mut f, Event::Resolve("x\ny\n".to_owned()));
        let by = f.merge.as_ref().and_then(|m| m.conflict(2)).and_then(|(_, c)| c.resolution.clone()).map(|r| r.by);
        assert_eq!(by, Some(By::Side(Side::Both)), "what a side gives counts as that side");
        assert_eq!(open(&f), 0);
        let effects = step(&mut f, Event::Commit);
        let [Effect::CommitAndPush(a, ..)] = effects.as_slice() else { panic!("{effects:?}") };
        let a = *a;
        assert!(step(&mut f, Event::Take(Side::Yours)).is_empty(), "nothing changes while committing");
        assert!(step(&mut f, Event::Committed(a + 9, Ok("1234".to_owned()))).is_empty(), "a stale answer");
        let done = step(&mut f, Event::Committed(a, Ok("1234".to_owned())));
        assert_eq!(
            done,
            [Effect::Close, Effect::Done("Merged main into perf/avatar-loading and pushed it".to_owned())]
        );
        assert_eq!(f.phase, Phase::Closed);
    }

    #[test]
    fn a_failed_push_keeps_the_commit_and_tries_again() {
        let mut f = resolving();
        for n in 0..3 {
            step(&mut f, Event::Go(n));
            step(&mut f, Event::Take(Side::Yours));
        }
        let effects = step(&mut f, Event::Commit);
        let [Effect::CommitAndPush(a, ..)] = effects.as_slice() else { panic!("{effects:?}") };
        step(&mut f, Event::Committed(*a, Err(NotPushed::Push("rejected".to_owned()))));
        assert_eq!(f.phase, Phase::Committed(None));
        assert_eq!(f.error.as_deref(), Some("The merge is committed, but the push failed: rejected"));
        let effects = step(&mut f, Event::Push);
        let [Effect::Push(b, _)] = effects.as_slice() else { panic!("{effects:?}") };
        let b = *b;
        assert!(matches!(step(&mut f, Event::Pushed(b, Ok(()))).as_slice(), [Effect::Close, Effect::Done(_)]));
    }

    #[test]
    fn aborting_closes_and_a_merge_that_wont_start_says_why() {
        let mut f = resolving();
        let effects = step(&mut f, Event::Abort);
        assert!(matches!(effects.as_slice(), [Effect::Abort(_, _, head)] if head == "abc"));
        let Phase::Aborting(a) = f.phase else { panic!("{:?}", f.phase) };
        let effects = step(&mut f, Event::Aborted(a, Ok(())));
        assert_eq!(effects, [Effect::Close, Effect::Done("Merge aborted: avatar-loading is as it was".to_owned())]);
        assert!(f.merge.is_none());

        let effects = step(&mut f, Event::Open(target()));
        let [_, Effect::Merge(b, _)] = effects.as_slice() else { panic!("{effects:?}") };
        step(&mut f, Event::Merged(*b, Err("uncommitted changes".to_owned())));
        assert_eq!(f.phase, Phase::Failed("uncommitted changes".to_owned()));
        step(&mut f, Event::Close);
        assert_eq!(f.phase, Phase::Closed);
    }

    #[test]
    fn the_agent_works_alone_then_what_it_resolved_is_read_back() {
        let mut f = resolving();
        step(&mut f, Event::Take(Side::Yours));
        let effects = step(&mut f, Event::Ask("Claude Code".to_owned()));
        let [Effect::StartAgent(_, prompt)] = effects.as_slice() else { panic!("{effects:?}") };
        assert!(prompt.contains("src/a.rs:4"), "{prompt}");
        assert!(!prompt.contains("src/a.rs:2"), "what's resolved isn't asked again: {prompt}");
        assert!(step(&mut f, Event::Take(Side::Base)).is_empty(), "the agent has the files");
        step(&mut f, Event::AgentStarted(Ok(12.0)));
        step(&mut f, Event::AgentSaid("✎ edited src/b.rs".to_owned()));
        assert!(
            matches!(&f.agent, Agent::Working { conflicts: 2, since, last, .. } if *since == 12.0 && last == "✎ edited src/b.rs")
        );
        assert_eq!(step(&mut f, Event::Stop), [Effect::StopAgent]);
        let effects = step(&mut f, Event::AgentEnded(Ending::Completed));
        let [Effect::Reread(a, _, files)] = effects.as_slice() else { panic!("{effects:?}") };
        let mut files = files.clone();
        files[1] = files[1].reconcile("x and y\n", By::Agent);
        step(&mut f, Event::Reread(*a, Ok(files)));
        assert_eq!(f.agent, Agent::Idle);
        assert_eq!(open(&f), 1);
        assert_eq!(
            f.agent_said.as_deref(),
            Some("Claude Code finished: it resolved 1 of 2. Review what it wrote, then commit.")
        );
        assert_eq!(f.current, 1, "the one still open");
    }
}
