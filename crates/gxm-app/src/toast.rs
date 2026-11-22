//! Toasts: a few words in the window's lower right corner saying something just happened (the
//! code is copied), gone on their own a moment later. rune-kit's `ToastStack` keeps them and
//! their timers: a toast shown again while it's in front stays one toast, and its time starts
//! over.

use rune::Tx;
use rune::machine::Effects;
use rune_kit::{ToastEffect, ToastEvent, ToastStack};

/// Seconds a toast shows.
const SECS: f64 = 2.5;

#[derive(Default)]
pub struct Toasts(ToastStack<String>);

impl Toasts {
    /// Each toast's id and what it says, oldest first.
    pub fn shown(&self) -> impl Iterator<Item = (u64, &str)> {
        self.0.entries().iter().map(|e| (e.id, e.options.as_str()))
    }

    /// Shows `words` for a moment.
    pub fn show(&mut self, words: &str, tx: &mut Tx) {
        let now = tx.clock();
        let shown = ToastEvent::Shown { key: words.to_owned(), options: words.to_owned(), secs: Some(SECS), now };
        wake(self.0.step(shown), tx);
    }

    /// Closes the toasts whose time is up at `now`. True when one closed.
    pub fn tick(&mut self, now: f64, tx: &mut Tx) -> bool {
        if self.0.next_due().is_none_or(|due| now + 1e-3 < due) {
            return false;
        }
        let before = self.0.entries().len();
        wake(self.0.step(ToastEvent::Tick { now }), tx);
        self.0.entries().len() != before
    }
}

/// Asks the window to wake when the next toast is due. A toast shown again would pulse, but
/// the press on the control that showed it already says the click landed.
fn wake(effects: Effects<ToastEffect>, tx: &mut Tx) {
    for effect in effects {
        if let ToastEffect::WakeAt(due) = effect {
            tx.wake_at(due);
        }
    }
}
