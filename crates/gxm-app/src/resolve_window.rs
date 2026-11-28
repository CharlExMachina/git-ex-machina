//! The resolve-conflicts window: a pull request's conflicts with its base, every one at once,
//! in a window of its own beside the main one. The main window opens it on a pull request
//! ([`ResolveLink::open`]); the window runs [`gxm_core::resolve::flow`], carrying out its
//! effects off the UI thread (git, the files, the coding agent), and tells the main window what
//! came of it ([`ResolveLink::notes`]).

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use gxm_core::env::Env;
use gxm_core::forge;
use gxm_core::harness::{self, Event as AgentEvent, Harness, RuntimeMode, Session, Start};
use gxm_core::remote::Remote;
use gxm_core::resolve::merge::{self, NotPushed};
use gxm_core::resolve::{By, Effect, Event, File, Flow, Merge, Target, flow};
use rune::{AppSpec, Key, KeyEvent, Latest, TextEdit, Tx, Waker, WindowHandle};

use crate::tag_window::Mailbox;
use crate::theme;

/// The window's key among the app's windows (`--window resolve` selects it headless).
pub const KEY: &str = "resolve";
/// Room for the sidebar, two panes side by side and the result under them, on a 1440×900 screen.
pub const SIZE: (f32, f32) = (1360.0, 800.0);
const MIN_SIZE: (f32, f32) = (980.0, 600.0);
/// How often the window looks again for a usable agent while there's none: often while none is
/// installed (a look on the path), seldom while one is but can't be used (asking it starts it).
const AGENT_RECHECK: f64 = 1.0;
const AGENT_RECHECK_INSTALLED: f64 = 10.0;

/// What the main window asks: resolve this pull request's conflicts.
#[derive(Clone, Debug)]
pub struct Opening {
    pub env: Env,
    pub target: Target,
    /// The repository's remote, whose pull requests say why the base changed.
    pub origin: Option<Remote>,
}

/// What the window tells the main one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    /// It's over (committed and pushed, or aborted): say this, and read the work again.
    Done(String),
    /// An agent started by gxm works in this worktree, or none does any more.
    Agent(Option<PathBuf>),
}

/// How the main window and the resolve window reach each other.
#[derive(Clone, Default)]
pub struct ResolveLink {
    pub open: Mailbox<Box<Opening>>,
    pub notes: Mailbox<Note>,
    /// The native window, once the main window's app has added it; journeys follow
    /// [`ResolveWindow::shown`] instead.
    pub window: Arc<OnceLock<WindowHandle>>,
}

/// The machine's work off the UI thread, each answer with the attempt that asked.
#[derive(Default)]
pub struct Jobs {
    merge: Latest<(u64, Result<Merge, String>)>,
    write: Latest<Result<(), String>>,
    commit: Latest<(u64, Result<String, NotPushed>)>,
    push: Latest<(u64, Result<(), String>)>,
    abort: Latest<(u64, Result<(), String>)>,
    reread: Latest<(u64, Result<Vec<File>, String>)>,
    /// The agent to use, and whether any is installed.
    agent: Latest<(Option<Harness>, bool)>,
    start: Latest<Result<Session, String>>,
}

impl Jobs {
    fn set_waker(&self, waker: &Waker) {
        self.merge.set_waker(waker.clone());
        self.write.set_waker(waker.clone());
        self.commit.set_waker(waker.clone());
        self.push.set_waker(waker.clone());
        self.abort.set_waker(waker.clone());
        self.reread.set_waker(waker.clone());
        self.agent.set_waker(waker.clone());
        self.start.set_waker(waker.clone());
    }

    fn busy(&self) -> bool {
        self.merge.is_pending()
            || self.write.is_pending()
            || self.commit.is_pending()
            || self.push.is_pending()
            || self.abort.is_pending()
            || self.reread.is_pending()
            || self.agent.is_pending()
            || self.start.is_pending()
    }
}

pub struct ResolveWindow {
    pub env: Env,
    pub link: ResolveLink,
    pub flow: Flow,
    pub jobs: Jobs,
    /// The window is on screen.
    pub shown: bool,
    /// The result pane: the conflict shown, as it's to be resolved.
    pub result: TextEdit,
    /// What the result pane was filled with; anything else in it was written by hand.
    pub filled: String,
    /// The agent to ask, once looked for; None when there's no usable one.
    pub agent: Option<Harness>,
    /// The window's clock at its last poll, for how long the agent has been at work.
    pub now: f64,
    origin: Option<Remote>,
    session: Option<Session>,
    waker: Option<Waker>,
    /// When to look for an agent again.
    recheck: f64,
}

impl ResolveWindow {
    pub fn new(mut env: Env, link: ResolveLink) -> ResolveWindow {
        env.claim_ui_thread();
        ResolveWindow {
            env,
            link,
            flow: Flow::default(),
            jobs: Jobs::default(),
            shown: false,
            result: TextEdit::new(""),
            filled: String::new(),
            agent: None,
            now: 0.0,
            origin: None,
            session: None,
            waker: None,
            recheck: 0.0,
        }
    }

    /// Something runs off the UI thread, or the main window has asked for something not yet
    /// taken. An agent at work isn't counted: it takes as long as it takes.
    pub fn busy(&self) -> bool {
        self.jobs.busy() || self.link.open.pending()
    }

    /// The result pane holds something other than what it was filled with.
    pub fn edited(&self) -> bool {
        self.result.text != self.filled
    }

    /// Feeds `event` to the machine and carries out what it asks for.
    pub fn send(&mut self, event: Event, tx: Option<&mut Tx>) {
        let mut tx = tx;
        for effect in flow::step(&mut self.flow, event) {
            self.effect(effect, tx.as_deref_mut());
        }
    }

    fn effect(&mut self, effect: Effect, tx: Option<&mut Tx>) {
        let env = self.env.clone();
        match effect {
            Effect::Show => {
                self.shown = true;
                self.recheck = self.now;
                if let Some(window) = self.link.window.get() {
                    window.open();
                }
            }
            Effect::Merge(a, target) => {
                let origin = self.origin.clone();
                self.jobs.merge.request((env, target, origin), move |(env, target, origin)| {
                    let forge = origin.and_then(|o| forge::connect(&env, &o).ok());
                    (a, merge::begin(&env, &target, forge.as_deref()))
                });
            }
            Effect::Write(target, files) => {
                self.jobs.write.request((env, target, files), |(env, t, files)| merge::write(&env, &t, &files));
            }
            Effect::CommitAndPush(a, target, files) => {
                self.jobs.commit.request((env, target, files), move |(env, t, files)| {
                    (a, merge::commit_and_push(&env, &t, &files))
                });
            }
            Effect::Push(a, target) => {
                self.jobs.push.request((env, target), move |(env, t)| {
                    let pushed = merge::push(&env, &t).map_err(|e| match e {
                        NotPushed::Commit(why) | NotPushed::Push(why) => why,
                    });
                    (a, pushed)
                });
            }
            Effect::Abort(a, target, head) => {
                self.jobs.abort.request((env, target, head), move |(env, t, head)| (a, merge::abort(&env, &t, &head)));
            }
            Effect::Reread(a, target, files) => {
                self.jobs.reread.request((env, target, files), move |(env, t, files)| {
                    (a, merge::reread(&env, &t, &files, By::Agent))
                });
            }
            Effect::StartAgent(target, prompt) => {
                let Some(h) = self.agent else { return };
                let wake = self.waker.clone();
                let start = Start { dir: target.worktree.clone(), prompt, mode: RuntimeMode::FullAccess };
                self.jobs.start.request((env, start, wake), move |(env, start, wake)| {
                    let wake: harness::Wake = Box::new(move || {
                        if let Some(w) = &wake {
                            w();
                        }
                    });
                    harness::start(&env, h, &start, wake)
                });
                self.link.notes.post(Note::Agent(Some(target.worktree)));
            }
            Effect::StopAgent => {
                if let Some(s) = &self.session {
                    s.stop();
                }
            }
            Effect::Refill => self.refill(),
            Effect::Close => {
                self.shown = false;
                if let Some(tx) = tx {
                    tx.close_window();
                }
            }
            Effect::Done(said) => self.link.notes.post(Note::Done(said)),
        }
    }

    /// The result pane, filled afresh with the conflict shown as it stands.
    fn refill(&mut self) {
        let text = self
            .flow
            .merge
            .as_ref()
            .and_then(|m| m.conflict(self.flow.current))
            .map(|(_, c)| c.text().to_owned())
            .unwrap_or_default();
        self.result = TextEdit::new(text.clone());
        self.filled = text;
    }

    /// Takes what the main window asked for and what finished off the UI thread. True when
    /// the state changed.
    fn land(&mut self, now: f64, tx: &mut Tx) -> bool {
        self.now = now;
        self.env.tell_clock(now);
        let mut changed = false;
        for opening in self.link.open.take() {
            let Opening { mut env, target, origin } = *opening;
            env.claim_ui_thread();
            self.env = env;
            self.origin = origin;
            self.send(Event::Open(target), Some(tx));
            changed = true;
        }
        if let Some((a, m)) = self.jobs.merge.take() {
            self.send(Event::Merged(a, m), Some(tx));
            changed = true;
        }
        if let Some(wrote) = self.jobs.write.take() {
            self.send(Event::Wrote(wrote), Some(tx));
            changed = true;
        }
        if let Some((a, r)) = self.jobs.commit.take() {
            self.send(Event::Committed(a, r), Some(tx));
            changed = true;
        }
        if let Some((a, r)) = self.jobs.push.take() {
            self.send(Event::Pushed(a, r), Some(tx));
            changed = true;
        }
        if let Some((a, r)) = self.jobs.abort.take() {
            self.send(Event::Aborted(a, r), Some(tx));
            changed = true;
        }
        if let Some((a, r)) = self.jobs.reread.take() {
            self.send(Event::Reread(a, r), Some(tx));
            changed = true;
        }
        if let Some((found, installed)) = self.jobs.agent.take() {
            self.agent = found;
            self.recheck = now + if installed { AGENT_RECHECK_INSTALLED } else { AGENT_RECHECK };
            changed = true;
        }
        if let Some(started) = self.jobs.start.take() {
            let at = match started {
                Ok(session) => {
                    self.session = Some(session);
                    Ok(now)
                }
                Err(why) => {
                    self.link.notes.post(Note::Agent(None));
                    Err(why)
                }
            };
            self.send(Event::AgentStarted(at), Some(tx));
            changed = true;
        }
        changed |= self.hear_agent(tx);
        // No agent yet: look again now and then, so one installed meanwhile shows up.
        if self.shown && self.agent.is_none() && now >= self.recheck && !self.jobs.agent.is_pending() {
            self.recheck = now + AGENT_RECHECK;
            self.jobs.agent.request(self.env.clone(), |env| {
                let all = harness::available(&env);
                (harness::pick(harness::choice(&env), &all), all.iter().any(|s| s.installed()))
            });
        }
        if self.shown && self.agent.is_none() {
            tx.wake_at(self.recheck);
        }
        // While the agent works, its "started 40s ago" moves on.
        if matches!(self.flow.agent, flow::Agent::Working { .. }) {
            tx.wake_at(now.floor() + 1.0);
        }
        changed
    }

    /// What the agent said since the last poll.
    fn hear_agent(&mut self, tx: &mut Tx) -> bool {
        let mut heard = Vec::new();
        if let Some(s) = &self.session {
            while let Some(e) = s.try_event() {
                heard.push(e);
            }
        }
        let changed = !heard.is_empty();
        for e in heard {
            match e {
                AgentEvent::Ended(ending) => {
                    self.session = None;
                    self.link.notes.post(Note::Agent(None));
                    self.send(Event::AgentEnded(ending), Some(tx));
                }
                other => self.send(Event::AgentSaid(other.line()), Some(tx)),
            }
        }
        changed
    }
}

/// ⌥↓ and ⌥↑ step through every conflict, across files.
fn on_key(s: &mut ResolveWindow, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if !ev.mods.alt || ev.mods.cmd || ev.mods.ctrl {
        return false;
    }
    let by = match ev.key {
        Key::Down => 1,
        Key::Up => -1,
        _ => return false,
    };
    s.send(Event::Step(by), Some(tx));
    true
}

/// The window, as the main window's app adds it and as headless runs build it. `link` reaches
/// the main window.
pub fn app(link: ResolveLink) -> rune::App<ResolveWindow> {
    let init_link = link;
    rune::App::new(AppSpec {
        poll: Some(|s: &mut ResolveWindow, t, tx| s.land(t, tx)),
        setup: Some(|s: &mut ResolveWindow, waker| {
            s.jobs.set_waker(&waker);
            s.link.open.set_waker(waker.clone());
            s.waker = Some(waker);
        }),
        on_key: Some(on_key),
        min_size: Some(MIN_SIZE),
        fonts: theme::FONTS,
        families: (theme::SANS, theme::MONO),
        ..AppSpec::new(
            "Resolve conflicts",
            SIZE,
            || ResolveWindow::new(Env::from_process(), ResolveLink::default()),
            crate::view::resolve_window,
        )
    })
    .init(move || ResolveWindow::new(Env::from_process(), init_link))
    .chrome(crate::CHROME)
    .font_smoothing(crate::FONT_SMOOTHING)
}

/// The window, added to `main` hidden: it shows when the main window opens it on a pull
/// request. Returns the link the main window's state keeps.
pub fn add_to<S: 'static>(main: &mut rune::App<S>) -> ResolveLink {
    let link = ResolveLink::default();
    let window = app(link.clone()).on_close(|s: &mut ResolveWindow, _| {
        s.shown = false;
        flow::step(&mut s.flow, Event::Close);
    });
    let hidden = rune::WindowOptions { visible: false, min_size: Some(MIN_SIZE), ..Default::default() };
    let spec = rune::WindowSpec::from_app(KEY, window).options(hidden);
    let (_, handle) = main.window(spec);
    link.window.get_or_init(|| handle);
    link
}

impl crate::state::App {
    /// Opens the resolve window on `repo`'s pull request `number`, in its worktree.
    pub fn open_resolve(&self, repo: &std::path::Path, number: u64) {
        let Some(r) = self.work.of(repo) else { return };
        let Some(work) = r.work.as_ref() else { return };
        let Some(p) = work.pull(number) else { return };
        let Some(wt) = work.local.on_branch(&p.summary.head_ref) else { return };
        let origin = self.repos.iter().find(|e| e.summary.path == repo).and_then(|e| e.summary.origin.clone());
        let target = Target {
            repo: repo.to_path_buf(),
            worktree: wt.path.clone(),
            worktree_name: wt.name.clone(),
            branch: p.summary.head_ref.clone(),
            base: p.summary.base_ref.clone(),
            remote: "origin".to_owned(),
            number,
            title: p.summary.title.clone(),
        };
        self.resolve_link.open.post(Box::new(Opening { env: self.env.clone(), target, origin }));
    }

    /// What the resolve window said: a merge pushed or aborted (a toast, and the work read
    /// again), or an agent starting or stopping in a worktree. True when it said anything.
    pub fn land_resolve(&mut self, tx: &mut Tx) -> bool {
        let notes = self.resolve_link.notes.take();
        for note in &notes {
            match note {
                Note::Done(said) => {
                    self.toasts.show(said, tx);
                    self.read_work();
                    self.reload_repo();
                }
                Note::Agent(at) => self.agent_at = at.clone(),
            }
        }
        !notes.is_empty()
    }
}
