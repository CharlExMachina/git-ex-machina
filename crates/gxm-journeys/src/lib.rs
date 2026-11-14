//! Journeys: long, user-shaped workflows that are the app's tests, its demo videos and the proof
//! attached to every pull request, all from one definition.
//!
//! A [`Journey`] is what a person does, step by step, with what they should see after each step.
//! [`run`] drives the real app (the same `rune::App` the window runs) headless, on a fixed clock,
//! inside a hermetic [`World`], and fails on the first expectation that doesn't hold, naming the
//! step and listing what was on screen. It also returns a [`Recording`]: the same steps as a
//! Rune input script, so `cargo xtask journey NAME --video` replays exactly what the test did
//! into a keycapped video and a screenshot at every mark.
//!
//! Write fewer, longer journeys (see CLAUDE.md): one per workflow, with expectations at every
//! step, never one per assertion.

pub mod catalog;
mod review;
mod runner;

pub use runner::{Failure, Recording, run};

use gxm_world::World;

/// How long a [`Step::WaitFor`] waits, in real time, before failing the journey.
pub const WAIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(20);

/// A workflow a person goes through, with what they should see along the way.
pub struct Journey {
    /// `snake_case`; also the folder its proof renders into.
    pub name: &'static str,
    /// One sentence: who does what, and why.
    pub story: &'static str,
    /// Builds the world the journey starts in.
    pub world: fn(&World),
    pub dark: bool,
    pub steps: Vec<Step>,
}

/// One thing the person does or expects.
#[derive(Clone, Debug)]
pub enum Step {
    /// Click the element with this key.
    Tap(String),
    /// Click the element showing this text.
    TapText(String),
    /// Type into whatever has focus.
    Type(String),
    /// Press a key.
    Press(Press),
    /// Hold modifiers (`"cmd"`, `"shift"`, `"cmd+shift"`) until the next hold; `""` lets go.
    Hold(&'static str),
    /// Let time pass (animations settle, the person reads).
    Wait(f64),
    /// Scroll with the wheel until the element with this key is wholly in view.
    ScrollTo(String),
    /// Keep the app running until this text shows inside the element with this key: for work
    /// that runs in real time off the UI thread, such as a push.
    WaitFor { key: String, text: String },
    /// A moment worth a screenshot in the proof.
    Mark(&'static str),
    /// This text is on screen somewhere.
    See(String),
    /// This text is on screen inside the element with this key.
    SeeIn { key: String, text: String },
    /// This text is nowhere on screen.
    NotSee(String),
    /// No element has this key.
    Gone(String),
    /// Something about the world itself, such as what a remote now holds.
    World { what: &'static str, check: fn(&World) -> Result<(), String> },
}

/// A key press, as Rune's scripts name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    pub name: &'static str,
    pub cmd: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Press {
    pub const DOWN: Press = Press::key("Down");
    pub const UP: Press = Press::key("Up");
    pub const ENTER: Press = Press::key("Enter");
    pub const ESCAPE: Press = Press::key("Escape");
    pub const TAB: Press = Press::key("Tab");

    pub const fn key(name: &'static str) -> Press {
        Press { name, cmd: false, shift: false, alt: false }
    }

    /// ⌘ on macOS, Ctrl elsewhere, with a letter.
    pub const fn cmd(name: &'static str) -> Press {
        Press { name, cmd: true, shift: false, alt: false }
    }
}

impl Journey {
    pub fn new(name: &'static str, story: &'static str, world: fn(&World)) -> Journey {
        Journey { name, story, world, dark: true, steps: Vec::new() }
    }

    fn step(mut self, s: Step) -> Journey {
        self.steps.push(s);
        self
    }

    pub fn light(mut self) -> Journey {
        self.dark = false;
        self
    }

    pub fn tap(self, key: impl Into<String>) -> Journey {
        self.step(Step::Tap(key.into()))
    }

    pub fn tap_text(self, text: impl Into<String>) -> Journey {
        self.step(Step::TapText(text.into()))
    }

    pub fn type_text(self, text: impl Into<String>) -> Journey {
        self.step(Step::Type(text.into()))
    }

    pub fn press(self, p: Press) -> Journey {
        self.step(Step::Press(p))
    }

    /// Holds modifier keys down for the steps that follow, as for a ⌘-click; `""` lets go.
    pub fn hold(self, mods: &'static str) -> Journey {
        self.step(Step::Hold(mods))
    }

    /// Scrolls (with the wheel, as a person would) until the element keyed `key` is in view.
    pub fn scroll_to(self, key: impl Into<String>) -> Journey {
        self.step(Step::ScrollTo(key.into()))
    }

    pub fn wait(self, secs: f64) -> Journey {
        self.step(Step::Wait(secs))
    }

    /// Waits (up to [`WAIT_LIMIT`]) until `text` shows inside the element keyed `key`.
    pub fn wait_for(self, key: impl Into<String>, text: impl Into<String>) -> Journey {
        self.step(Step::WaitFor { key: key.into(), text: text.into() })
    }

    pub fn mark(self, name: &'static str) -> Journey {
        self.step(Step::Mark(name))
    }

    pub fn see(self, text: impl Into<String>) -> Journey {
        self.step(Step::See(text.into()))
    }

    pub fn see_in(self, key: impl Into<String>, text: impl Into<String>) -> Journey {
        self.step(Step::SeeIn { key: key.into(), text: text.into() })
    }

    pub fn not_see(self, text: impl Into<String>) -> Journey {
        self.step(Step::NotSee(text.into()))
    }

    pub fn gone(self, key: impl Into<String>) -> Journey {
        self.step(Step::Gone(key.into()))
    }

    pub fn world_check(self, what: &'static str, check: fn(&World) -> Result<(), String>) -> Journey {
        self.step(Step::World { what, check })
    }
}

impl std::fmt::Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Step::Tap(k) => write!(f, "tap [{k}]"),
            Step::TapText(t) => write!(f, "tap {t:?}"),
            Step::Type(t) => write!(f, "type {t:?}"),
            Step::Press(p) => {
                write!(f, "press {}{}{}", if p.cmd { "⌘" } else { "" }, if p.shift { "⇧" } else { "" }, p.name)
            }
            Step::Hold("") => write!(f, "let go of the modifiers"),
            Step::Hold(m) => write!(f, "hold {m}"),
            Step::Wait(s) => write!(f, "wait {s}s"),
            Step::ScrollTo(k) => write!(f, "scroll to [{k}]"),
            Step::WaitFor { key, text } => write!(f, "wait for {text:?} in [{key}]"),
            Step::Mark(m) => write!(f, "mark {m:?}"),
            Step::See(t) => write!(f, "see {t:?}"),
            Step::SeeIn { key, text } => write!(f, "see {text:?} in [{key}]"),
            Step::NotSee(t) => write!(f, "don't see {t:?}"),
            Step::Gone(k) => write!(f, "[{k}] is gone"),
            Step::World { what, .. } => write!(f, "world: {what}"),
        }
    }
}
