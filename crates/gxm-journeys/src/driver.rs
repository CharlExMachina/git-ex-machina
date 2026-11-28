//! The windows a journey drives, as the runner holds them: the main window, and the tag and
//! resolve windows that come in front of it; what moves their clocks, what's on screen, and how
//! a person's pointer and wheel reach them.

use gxm_app::resolve_window::{self, ResolveWindow};
use gxm_app::state::App;
use gxm_app::tag_window::{self, TagWindow};
use rune::{ElementInfo, Runtime};
use serde_json::{Value, json};

use crate::runner::Mark;
use crate::window::Window;

/// Frames advance at 60 Hz on the runtime's virtual clock, as `--script` renders do.
pub(crate) const DT: f64 = 1.0 / 60.0;
/// How long the runner sleeps between frames while a background read is in flight, so the read
/// gets real time to finish.
pub(crate) const POLL: std::time::Duration = std::time::Duration::from_millis(1);
/// The longest a mark waits for the screen to come to rest before its screenshot.
pub(crate) const MARK_SETTLE: f64 = 1.0;
/// One wheel step of a [`crate::Step::ScrollTo`], in points.
pub(crate) const WHEEL: f32 = 60.0;

/// Which window is in front, with the keyboard.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Front {
    Main,
    Tag,
    Resolve,
}

pub(crate) struct Driver {
    pub(crate) rt: Runtime<App>,
    /// The tag window, in front of the main one while it shows.
    pub(crate) tag: Runtime<TagWindow>,
    /// The resolve-conflicts window, in front of the main one while it shows.
    pub(crate) resolve: Runtime<ResolveWindow>,
    pub(crate) painter: crate::paint::Painter,
    pub(crate) tag_painter: Option<crate::paint::Painter>,
    pub(crate) resolve_painter: Option<crate::paint::Painter>,
    /// Another window has shown: the video records every window, as a screen recording would.
    pub(crate) windows: bool,
    pub(crate) clipboard: rune::clipboard::MemoryClipboard,
    /// Where a [`Step::Down`] pressed the mouse button, until [`Step::Up`] lets go.
    pub(crate) held: Option<(f32, f32)>,
    /// URLs the app asked to open, since the last `Opened` step looked.
    pub(crate) opened: Vec<String>,
    pub(crate) now: f64,
    /// Whether the last frame was still animating.
    pub(crate) animating: bool,
    pub(crate) dark: bool,
    pub(crate) events: Vec<Value>,
    pub(crate) marks: Vec<Mark>,
}

impl Driver {
    /// The window in front, with the keyboard: the tag or resolve window while one shows.
    pub(crate) fn front(&self) -> Front {
        if self.tag.state.shown {
            Front::Tag
        } else if self.resolve.state.shown {
            Front::Resolve
        } else {
            Front::Main
        }
    }

    /// The window in front: where input goes and what expectations look at.
    pub(crate) fn win(&self) -> &dyn Window {
        match self.front() {
            Front::Tag => &self.tag,
            Front::Resolve => &self.resolve,
            Front::Main => &self.rt,
        }
    }

    pub(crate) fn win_mut(&mut self) -> &mut dyn Window {
        match self.front() {
            Front::Tag => &mut self.tag,
            Front::Resolve => &mut self.resolve,
            Front::Main => &mut self.rt,
        }
    }

    /// Where the front window's top left corner is in the main window's points, as a desktop
    /// recording places it: another window centred over the main one.
    pub(crate) fn origin(&self) -> (f32, f32) {
        let (w, h) = match self.front() {
            Front::Main => return (0.0, 0.0),
            Front::Tag => self.tag.viewport,
            Front::Resolve => self.resolve.viewport,
        };
        (((gxm_app::SIZE.0 - w) / 2.0).round(), ((gxm_app::SIZE.1 - h) / 2.0).round())
    }

    /// Any window is waiting on work off the UI thread, or on another.
    pub(crate) fn busy(&self) -> bool {
        self.rt.state.busy()
            || self.tag.state.busy()
            || self.resolve.state.busy()
            || self.rt.state.tag_link.done.pending()
            || self.rt.state.resolve_link.notes.pending()
    }

    /// The screen: the main window, with the window in front over it while one shows.
    pub(crate) fn paint(&mut self) -> crate::paint::Pixels {
        let mut pixels = self.painter.paint(&mut self.rt);
        let origin = self.origin();
        match self.front() {
            Front::Main => {}
            Front::Tag => {
                let tag = &mut self.tag;
                let painter = self.tag_painter.get_or_insert_with(|| crate::paint::Painter::new(tag, tag_window::SIZE));
                pixels.composite(&painter.paint(tag), origin);
            }
            Front::Resolve => {
                let resolve = &mut self.resolve;
                let painter = self
                    .resolve_painter
                    .get_or_insert_with(|| crate::paint::Painter::new(resolve, resolve_window::SIZE));
                pixels.composite(&painter.paint(resolve), origin);
            }
        }
        pixels
    }

    pub(crate) fn advance_to(&mut self, t: f64) {
        // Every step acts, then advances: its first frame takes the input, so it's one frame.
        let mut first = true;
        while self.now + DT / 2.0 < t {
            let frames = if std::mem::take(&mut first) { 1 } else { self.frames_until(t) };
            let dt = frames as f64 * DT;
            let tag = self.tag.frame(dt);
            let resolve = self.resolve.frame(dt);
            self.animating = self.rt.frame(dt) | tag | resolve;
            self.now += dt;
            self.opened.extend(self.rt.take_url_requests());
            self.opened.extend(self.tag.take_url_requests());
            self.opened.extend(self.resolve.take_url_requests());
            self.windows |= self.tag.state.shown || self.resolve.state.shown;
        }
    }

    /// How many 60 Hz frames the next frame covers. While something moves or a background read
    /// runs, one. At rest a window draws nothing until input, a timer or a read's answer wakes
    /// it, so neither does the runner: one frame spans the rest, up to the runtime's next wake
    /// (`Runtime::wake_deadline`) or `t`. That's most of a journey's time, and the screen is the
    /// same either way.
    pub(crate) fn frames_until(&self, t: f64) -> usize {
        if self.animating || self.busy() {
            return 1;
        }
        let left = ((t - self.now) / DT).round();
        let wake = [
            self.rt.wake_deadline().map(|at| at - self.rt.clock),
            self.tag.wake_deadline().map(|at| at - self.tag.clock),
            self.resolve.wake_deadline().map(|at| at - self.resolve.clock),
        ]
        .into_iter()
        .flatten()
        .fold(f64::INFINITY, |soonest, left| soonest.min((left / DT).ceil()));
        left.min(wake).max(1.0) as usize
    }

    /// Runs frames until nothing animates, for at most [`MARK_SETTLE`] (a caret blinks forever).
    pub(crate) fn settle_motion(&mut self) {
        let until = self.now + MARK_SETTLE;
        while self.animating && self.now + DT / 2.0 < until {
            self.advance_to(self.now + DT);
        }
    }

    /// Runs frames until no background read is in flight and none has an answer waiting. Reads
    /// run in real time, so a replay keeps pace with the wall clock (`--realtime`) to land them
    /// at the same moments.
    pub(crate) fn settle(&mut self) -> Result<(), String> {
        let started = std::time::Instant::now();
        // Not pending can still mean landed but not yet taken: only a frame that takes every
        // answer without asking for more, after a check that found nothing running, is settled.
        let mut quiet = 0;
        while quiet < 2 {
            if started.elapsed() > crate::WAIT_LIMIT {
                return Err(format!("a background read was still running after {:?}", crate::WAIT_LIMIT));
            }
            if self.busy() {
                quiet = 0;
                std::thread::sleep(POLL);
            } else {
                quiet += 1;
            }
            if quiet < 2 {
                self.advance_to(self.now + DT);
            }
        }
        Ok(())
    }

    /// Whether the element keyed `key` is on screen in full.
    pub(crate) fn wholly_visible(&self, key: &str) -> bool {
        self.win()
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
            .is_none_or(|e| e.visible[3] >= e.rect[3] - 0.5)
    }

    /// Wheels, as a person would, until the element keyed `key` is wholly in view.
    pub(crate) fn scroll_into_view(&mut self, key: &str) -> Result<(), String> {
        for _ in 0..80 {
            let all = self.win().elements();
            let i = all
                .iter()
                .position(|e| e.key.as_deref() == Some(key) && !e.exiting)
                .ok_or_else(|| format!("no element is keyed {key:?}"))?;
            let e = &all[i];
            if e.visible[3] >= e.rect[3] - 0.5 {
                break;
            }
            // Positive moves the content down, showing what's above.
            let dy = if e.rect[1] < e.visible[1] { WHEEL } else { -WHEEL };
            // The wheel turns over what's showing of the element, or of the nearest thing
            // holding it that shows (its scroll view, inside a dialog, say).
            let mut shown = Some(i);
            while let Some(j) = shown.filter(|j| all[*j].visible[3] <= 1.0) {
                shown = all[j].parent;
            }
            let (x, y) = match shown {
                Some(j) => {
                    let v = all[j].visible;
                    let edge = if dy < 0.0 { v[1] + v[3] - 4.0 } else { v[1] + 4.0 };
                    (v[0] + v[2] / 2.0, edge.clamp(v[1], v[1] + v[3]))
                }
                None => (e.rect[0] + e.rect[2] / 2.0, (e.rect[1]).clamp(80.0, gxm_app::SIZE.1 - 40.0)),
            };
            self.win_mut().pointer_move(x, y);
            self.win_mut().wheel(x, y, dy);
            let (ox, oy) = self.origin();
            self.events.push(json!({ "t": self.now, "type": "wheel", "x": x + ox, "y": y + oy, "dy": dy }));
            self.advance_to(self.now + DT * 2.0);
        }
        Ok(())
    }

    pub(crate) fn tap(&mut self, x: f32, y: f32) {
        let win = self.win_mut();
        win.pointer_move(x, y);
        win.pointer_down(x, y);
        win.pointer_up(x, y);
    }

    /// Elements a person could see: on screen, not clipped away, not faded out or leaving.
    pub(crate) fn visible(&self) -> Vec<ElementInfo> {
        self.win()
            .elements()
            .into_iter()
            .filter(|e| !e.exiting && e.opacity > 0.01 && e.visible[2] > 0.5 && e.visible[3] > 0.5)
            .collect()
    }

    /// The centre of the smallest visible element whose text contains `text`.
    pub(crate) fn find_text(&self, text: &str) -> Option<(f32, f32)> {
        self.visible()
            .into_iter()
            .filter(|e| e.text.as_deref().is_some_and(|s| s.contains(text)))
            .min_by(|a, b| (a.rect[2] * a.rect[3]).total_cmp(&(b.rect[2] * b.rect[3])))
            .map(|e| (e.visible[0] + e.visible[2] / 2.0, e.visible[1] + e.visible[3] / 2.0))
    }

    /// The element keyed `key`'s label, then every visible text inside it, joined by spaces.
    pub(crate) fn text_in(&self, key: &str) -> Option<String> {
        let all = self.win().elements();
        let root = all.iter().position(|e| e.key.as_deref() == Some(key) && !e.exiting)?;
        let inside = |mut i: usize| loop {
            if i == root {
                return true;
            }
            match all[i].parent {
                Some(p) => i = p,
                None => return false,
            }
        };
        // An icon button's label is what it says (its tooltip, and what a screen reader reads).
        let label = all[root].label.as_deref().filter(|l| !l.is_empty());
        let texts: Vec<&str> = label
            .into_iter()
            .chain(
                (0..all.len())
                    .filter(|&i| inside(i) && all[i].opacity > 0.01 && all[i].visible[2] > 0.5)
                    .filter_map(|i| all[i].text.as_deref()),
            )
            .collect();
        Some(texts.join(" "))
    }

    /// The screen as text, for failure messages: each visible element with a key or text.
    pub(crate) fn screen(&self) -> String {
        let lines: Vec<String> = self
            .visible()
            .iter()
            .filter(|e| e.text.is_some() || e.key.is_some())
            .filter(|e| e.text.is_some() || e.role != "group")
            .map(|e| {
                let name = e.key.as_deref().unwrap_or(e.role);
                match &e.text {
                    Some(t) => format!("  [{name}] {t:?}"),
                    None => format!("  [{name}] ({})", e.role),
                }
            })
            .collect();
        const MAX: usize = 120;
        let more = lines.len().saturating_sub(MAX);
        let mut s = lines.into_iter().take(MAX).collect::<Vec<_>>().join("\n");
        if more > 0 {
            s.push_str(&format!("\n  … and {more} more"));
        }
        s
    }
}
