//! A session with a harness, as a machine: [`State`], and one [`step`] that turns what happened
//! (a line the harness printed, a stop, the process ending) into [`Effect`]s, which the runner
//! (`run.rs`) carries out: lines written to the harness, events handed to whoever listens, the
//! process killed.
//!
//! What each harness says is its own [`Protocol`]'s business: Claude Code prints stream-json,
//! Codex speaks JSON-RPC. The machine only knows the phases they share.

use super::event::{Ending, Event};

/// Where a session is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// The harness is starting up: handshake, then the prompt.
    Starting,
    /// The agent is at work on the prompt.
    Working,
    /// Asked to stop: waiting for the process to end.
    Stopping,
    /// Over, and how.
    Ended(Ending),
}

/// What happens to a session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// The process is up: say hello and give it the prompt.
    Spawned,
    /// A line the harness printed on stdout.
    Line(String),
    /// The person stops the agent.
    Stop,
    /// The process ended, with its exit code (None for a signal) and the tail of its stderr.
    Exited { code: Option<i32>, stderr: String },
}

/// What the runner does for the machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Writes this line (a newline follows) to the harness's stdin.
    Write(String),
    /// Hands this event to whoever is listening.
    Emit(Event),
    /// Kills the process.
    Kill,
}

/// What a protocol makes of the harness's output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Said {
    /// A line to send back (a handshake step, the prompt, an answer to a request).
    Reply(String),
    Event(Event),
    /// The turn is over: the agent finished, or failed.
    TurnEnded(Ending),
}

/// One harness's wire format: what to say first, and what its lines mean.
pub trait Protocol: Send {
    /// What to write once the process is up.
    fn open(&mut self) -> Vec<Said>;
    /// What a line from its stdout means. A line it can't read is ignored, not an error:
    /// harnesses add message types faster than clients learn them.
    fn line(&mut self, line: &str) -> Vec<Said>;
    /// What to write to stop the turn politely before the process is killed.
    fn interrupt(&mut self) -> Vec<Said> {
        Vec::new()
    }
}

/// A session's machine: its state, and the protocol that reads its harness.
pub struct Machine {
    pub state: State,
    protocol: Box<dyn Protocol>,
}

impl std::fmt::Debug for Machine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Machine").field("state", &self.state).finish_non_exhaustive()
    }
}

impl Machine {
    pub fn new(protocol: Box<dyn Protocol>) -> Machine {
        Machine { state: State::Starting, protocol }
    }
}

/// One transition.
pub fn step(m: &mut Machine, input: Input) -> Vec<Effect> {
    if matches!(m.state, State::Ended(_)) {
        return Vec::new();
    }
    match input {
        Input::Spawned => {
            let said = m.protocol.open();
            settle(m, said)
        }
        Input::Line(line) => {
            let said = m.protocol.line(&line);
            settle(m, said)
        }
        Input::Stop if m.state == State::Stopping => Vec::new(),
        Input::Stop => {
            m.state = State::Stopping;
            let said = m.protocol.interrupt();
            let mut out: Vec<Effect> = said
                .into_iter()
                .filter_map(|s| if let Said::Reply(l) = s { Some(Effect::Write(l)) } else { None })
                .collect();
            out.push(Effect::Kill);
            out
        }
        Input::Exited { code, stderr } => {
            let ending = match (&m.state, code) {
                (State::Stopping, _) => Ending::Stopped,
                (_, Some(0)) => Ending::Failed("the agent stopped before finishing its turn".to_owned()),
                (_, code) => Ending::Failed(exited(code, &stderr)),
            };
            end(m, ending)
        }
    }
}

/// Carries out what the protocol said: replies go out, events go on, an ended turn ends the
/// session (one prompt, one turn).
fn settle(m: &mut Machine, said: Vec<Said>) -> Vec<Effect> {
    let mut out = Vec::new();
    for s in said {
        match s {
            Said::Reply(line) => out.push(Effect::Write(line)),
            Said::Event(e) => {
                if m.state == State::Starting && matches!(e, Event::Started { .. }) {
                    m.state = State::Working;
                }
                out.push(Effect::Emit(e));
            }
            Said::TurnEnded(ending) => {
                // A turn the person stopped is stopped, whatever the harness calls it.
                let ending = if m.state == State::Stopping { Ending::Stopped } else { ending };
                out.extend(end(m, ending));
                // Its process has nothing more to do.
                out.push(Effect::Kill);
                break;
            }
        }
    }
    out
}

fn end(m: &mut Machine, ending: Ending) -> Vec<Effect> {
    m.state = State::Ended(ending.clone());
    vec![Effect::Emit(Event::Ended(ending))]
}

/// Why a harness that exited on its own failed, in its own words when it left any.
fn exited(code: Option<i32>, stderr: &str) -> String {
    let said = stderr.lines().rev().find(|l| !l.trim().is_empty()).map(str::trim);
    match (code, said) {
        (_, Some(said)) => said.to_owned(),
        (Some(c), None) => format!("the agent exited with {c}"),
        (None, None) => "the agent was killed".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::event::Event;

    /// A protocol that echoes: each line is assistant text, `done` ends the turn.
    struct Echo;

    impl Protocol for Echo {
        fn open(&mut self) -> Vec<Said> {
            vec![Said::Reply("hello".to_owned()), Said::Event(Event::Started { session: "s1".to_owned() })]
        }
        fn line(&mut self, line: &str) -> Vec<Said> {
            if line == "done" {
                vec![Said::TurnEnded(Ending::Completed)]
            } else {
                vec![Said::Event(Event::Text(line.to_owned()))]
            }
        }
        fn interrupt(&mut self) -> Vec<Said> {
            vec![Said::Reply("interrupt".to_owned())]
        }
    }

    #[test]
    fn a_session_starts_works_and_ends_once() {
        let mut m = Machine::new(Box::new(Echo));
        let opened = step(&mut m, Input::Spawned);
        assert_eq!(opened[0], Effect::Write("hello".to_owned()));
        assert_eq!(m.state, State::Working);
        assert_eq!(step(&mut m, Input::Line("hi".to_owned())), [Effect::Emit(Event::Text("hi".to_owned()))]);
        let done = step(&mut m, Input::Line("done".to_owned()));
        assert_eq!(done, [Effect::Emit(Event::Ended(Ending::Completed)), Effect::Kill]);
        assert!(step(&mut m, Input::Exited { code: None, stderr: String::new() }).is_empty(), "ended once");
    }

    #[test]
    fn a_stop_interrupts_then_kills_and_the_end_says_stopped() {
        let mut m = Machine::new(Box::new(Echo));
        step(&mut m, Input::Spawned);
        assert_eq!(step(&mut m, Input::Stop), [Effect::Write("interrupt".to_owned()), Effect::Kill]);
        assert!(step(&mut m, Input::Stop).is_empty(), "a second stop changes nothing");
        let ended = step(&mut m, Input::Exited { code: None, stderr: String::new() });
        assert_eq!(ended, [Effect::Emit(Event::Ended(Ending::Stopped))]);
    }

    #[test]
    fn a_harness_that_dies_says_why_in_its_own_words() {
        let mut m = Machine::new(Box::new(Echo));
        step(&mut m, Input::Spawned);
        let ended = step(&mut m, Input::Exited { code: Some(1), stderr: "boom\nInvalid API key\n".to_owned() });
        assert_eq!(ended, [Effect::Emit(Event::Ended(Ending::Failed("Invalid API key".to_owned())))]);
    }
}
