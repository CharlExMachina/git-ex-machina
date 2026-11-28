//! The harness as a running process: spawned in the session's folder, its stdout read line by
//! line on a thread of its own, and the [`session`](super::session) machine fed from one place,
//! so the person's stop and the harness's output are steps of the same machine, in order.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use super::event::Event;
use super::session::{self, Effect, Input, Machine, Protocol};
use super::{Harness, Start};
use crate::env::Env;

/// How much of a harness's stderr is kept, to say why it failed.
const STDERR_TAIL: usize = 4096;

/// Called after each event lands, so a window can wake to read it.
pub type Wake = Box<dyn Fn() + Send>;

/// A running session. Its events arrive in order and end with [`Event::Ended`]; dropping it
/// stops the agent.
pub struct Session {
    pub harness: Harness,
    inputs: Sender<Fed>,
    events: Receiver<Event>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("harness", &self.harness).finish_non_exhaustive()
    }
}

impl Session {
    /// The next event, if one has arrived.
    pub fn try_event(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// The next event, waiting up to `timeout` for it. None once the session has ended and
    /// every event has been read, or when none came in time.
    pub fn next_event(&self, timeout: Duration) -> Option<Event> {
        self.events.recv_timeout(timeout).ok()
    }

    /// Stops the agent: its turn is interrupted and its process killed. The session then ends
    /// with [`Ending::Stopped`](super::Ending::Stopped).
    pub fn stop(&self) {
        // Already over when nothing listens.
        self.inputs.send(Fed::Stop).ok();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

/// What to run: the program, its arguments, and the folder it works in.
pub(super) struct Launch {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub protocol: Box<dyn Protocol>,
}

/// Spawns `launch` in `start.dir` and runs its machine on a thread of its own.
pub(super) fn spawn(env: &Env, harness: Harness, start: &Start, launch: Launch, wake: Wake) -> Result<Session, String> {
    env.off_ui_thread(&format_args!("starting {}", harness.name()));
    let mut cmd = Command::new(&launch.program);
    cmd.args(&launch.args).current_dir(&start.dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    super::probe::child_env(env, &mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("couldn't start {}: {e}", launch.program.display()))?;
    let (inputs, feed) = mpsc::channel();
    let (emit, events) = mpsc::channel();
    let stdin = child.stdin.take();
    let out = child.stdout.take();
    let lines = inputs.clone();
    std::thread::spawn(move || {
        if let Some(out) = out {
            for line in BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                if lines.send(Fed::Line(line)).is_err() {
                    return;
                }
            }
        }
        lines.send(Fed::Closed).ok();
    });
    let err = child.stderr.take();
    let stderr = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(err) = err {
            BufReader::new(err).read_to_string(&mut text).ok();
        }
        let cut = (0..=text.len().saturating_sub(STDERR_TAIL)).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
        text.split_off(cut)
    });
    let machine = Machine::new(launch.protocol);
    let ends = Ends { stderr, emit, wake };
    std::thread::spawn(move || drive(machine, child, stdin, ends, &feed));
    Ok(Session { harness, inputs, events })
}

/// What reaches the session's loop.
enum Fed {
    Line(String),
    /// The harness closed its stdout: it's exiting.
    Closed,
    Stop,
}

/// The session's loop: every input through the machine, every effect carried out, until the
/// session has ended.
fn drive(mut m: Machine, mut child: Child, mut stdin: Option<ChildStdin>, ends: Ends, feed: &Receiver<Fed>) {
    let Ends { stderr, emit, wake } = ends;
    let mut stderr = Some(stderr);
    let mut next = Some(Input::Spawned);
    loop {
        let input = match next.take() {
            Some(i) => i,
            None => match feed.recv() {
                Ok(Fed::Line(line)) => Input::Line(line),
                Ok(Fed::Stop) => Input::Stop,
                Ok(Fed::Closed) | Err(_) => {
                    let code = wait(&mut child);
                    let tail = stderr.take().and_then(|h| h.join().ok()).unwrap_or_default();
                    Input::Exited { code, stderr: tail }
                }
            },
        };
        for effect in session::step(&mut m, input) {
            match effect {
                Effect::Write(line) => {
                    let wrote = stdin.as_mut().is_some_and(|s| writeln!(s, "{line}").and_then(|()| s.flush()).is_ok());
                    if !wrote {
                        stdin = None;
                    }
                }
                Effect::Emit(e) => {
                    emit.send(e).ok();
                    wake();
                }
                Effect::Kill => {
                    stdin = None;
                    // Gone already, if it finished on its own.
                    child.kill().ok();
                }
            }
        }
        if let session::State::Ended(_) = m.state {
            child.wait().ok();
            return;
        }
    }
}

/// The session's other ends: the harness's stderr, and where its events go.
struct Ends {
    stderr: std::thread::JoinHandle<String>,
    emit: Sender<Event>,
    wake: Wake,
}

/// The exit code of a harness that closed its stdout, killing it if it lingers.
fn wait(child: &mut Child) -> Option<i32> {
    for _ in 0..100 {
        match child.try_wait() {
            Ok(Some(status)) => return status.code(),
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
    child.kill().ok();
    child.wait().ok().and_then(|s| s.code())
}
