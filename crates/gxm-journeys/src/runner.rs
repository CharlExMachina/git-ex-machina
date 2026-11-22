//! Drives the real app through a journey, headless, and records it as a Rune input script.

use std::fmt;

use gxm_app::state::App;
use gxm_world::World;
use rune::{ElementInfo, Key, KeyEvent, Mods, PhysicalInput, Runtime};
use serde_json::{Value, json};

use crate::{Journey, Press, Step};

/// Frames advance at 60 Hz on the runtime's virtual clock, as `--script` renders do.
const DT: f64 = 1.0 / 60.0;
/// When the first step happens: after the workspace loads and the first frame settles.
const START: f64 = 0.8;
/// How long the runner sleeps between frames while a background read is in flight, so the read
/// gets real time to finish.
const POLL: std::time::Duration = std::time::Duration::from_millis(1);
/// Time after each kind of step, so animations land and a viewer can follow the video.
const AFTER_TAP: f64 = 0.7;
const PER_CHAR: f64 = 0.07;
const AFTER_TYPE: f64 = 0.5;
const AFTER_PRESS: f64 = 0.5;
/// A mark's screenshot is taken at its time; the next step waits this long so it can't leak in.
const AFTER_MARK: f64 = 0.3;
/// The longest a mark waits for the screen to come to rest before its screenshot.
const MARK_SETTLE: f64 = 1.0;
/// One wheel step of a [`Step::ScrollTo`], in points.
const WHEEL: f32 = 60.0;
/// The video runs this long past the last step.
const TAIL: f64 = 1.2;

/// A journey as Rune input (replay it with `--script` to get the same run on screen), and what
/// the screen showed at each mark.
#[derive(Clone, Debug)]
pub struct Recording {
    pub events: Vec<Value>,
    pub marks: Vec<Mark>,
    pub end: f64,
}

/// A moment worth a screenshot, as the journey saw it.
#[derive(Clone, Debug)]
pub struct Mark {
    pub name: &'static str,
    pub t: f64,
    /// The element tree, as JSON.
    pub tree: Value,
    /// The screen, drawn from the journey's own runtime.
    pub pixels: crate::paint::Pixels,
}

impl Recording {
    /// The `--script` JSON.
    pub fn script(&self) -> Value {
        json!({ "events": self.events })
    }
}

/// The first step that didn't hold.
#[derive(Debug)]
pub struct Failure {
    pub journey: &'static str,
    pub step: usize,
    pub what: String,
    pub why: String,
    /// What was on screen, one element per line.
    pub screen: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "journey {:?} failed at step {} ({}): {}", self.journey, self.step + 1, self.what, self.why)?;
        writeln!(f, "On screen:\n{}", self.screen)?;
        write!(f, "See it: cargo xtask journey {} (renders every mark up to the failure)", self.journey)
    }
}

impl std::error::Error for Failure {}

/// Runs `j` against the real app inside `world` (already built with `j.world`).
pub fn run(j: &Journey, world: &World) -> Result<Recording, Failure> {
    // At the screenshots' scale, so layout snaps to the pixels the screenshots show.
    let env = rune::Env { system_dark: j.dark, scale: crate::paint::SCALE };
    let mut rt = gxm_app::app().headless_runtime(gxm_app::SIZE, env);
    rt.set_font_smoothing(gxm_app::FONT_SMOOTHING);
    rt.state = App::new(world.env().clone());
    // The person's own clipboard stays theirs: copies land here, for `Copied` to read.
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    let painter = crate::paint::Painter::new(&mut rt);
    let mut d = Driver {
        rt,
        painter,
        clipboard,
        held: None,
        opened: Vec::new(),
        now: 0.0,
        animating: true,
        dark: j.dark,
        events: Vec::new(),
        marks: Vec::new(),
    };
    d.advance_to(START);
    for (i, step) in j.steps.iter().enumerate() {
        let fail = |why: String, d: &Driver| Failure {
            journey: j.name,
            step: i,
            what: step.to_string(),
            why,
            screen: d.screen(),
        };
        // Whatever the last step asked to read lands before this one acts or looks.
        d.settle().map_err(|why| fail(why, &d))?;
        let t = d.now;
        match step {
            Step::Tap(key) => {
                // A person scrolls to what they click when it's below the fold.
                if !d.wholly_visible(key) {
                    d.scroll_into_view(key).map_err(|why| fail(why, &d))?;
                }
                let t = d.now;
                let (x, y) = d.rt.find(key).ok_or_else(|| fail(format!("no element is keyed {key:?}"), &d))?;
                d.tap(x, y);
                d.events.push(json!({ "t": t, "type": "tap", "key": key }));
                d.advance_to(t + AFTER_TAP);
            }
            Step::TapText(text) => {
                let (x, y) = d.find_text(text).ok_or_else(|| fail(format!("nothing on screen shows {text:?}"), &d))?;
                d.tap(x, y);
                d.events.push(json!({ "t": t, "type": "tap", "x": x, "y": y }));
                d.advance_to(t + AFTER_TAP);
            }
            Step::Down(key) => {
                if !d.wholly_visible(key) {
                    d.scroll_into_view(key).map_err(|why| fail(why, &d))?;
                }
                let t = d.now;
                let (x, y) = d.rt.find(key).ok_or_else(|| fail(format!("no element is keyed {key:?}"), &d))?;
                d.rt.pointer_move(x, y);
                d.rt.pointer_down(x, y);
                d.held = Some((x, y));
                d.events.push(json!({ "t": t, "type": "down", "key": key }));
                d.advance_to(t + AFTER_PRESS);
            }
            Step::Up => {
                let (x, y) = d.held.take().ok_or_else(|| fail("no mouse button is held".to_owned(), &d))?;
                d.rt.pointer_up(x, y);
                d.events.push(json!({ "t": t, "type": "up" }));
                d.advance_to(t + AFTER_TAP);
            }
            Step::Type(text) => {
                d.events.push(json!({ "t": t, "type": "type", "text": text }));
                for ch in text.chars() {
                    d.rt.key(KeyEvent { key: Key::Text(ch.to_string()), mods: Mods::default() });
                    d.advance_to(d.now + PER_CHAR);
                }
                d.advance_to(t + PER_CHAR * text.chars().count() as f64 + AFTER_TYPE);
            }
            Step::Press(p) => {
                let key = key_of(p).ok_or_else(|| fail(format!("journeys can't press {:?} yet", p.name), &d))?;
                d.rt.key(KeyEvent { key, mods: Mods { cmd: p.cmd, shift: p.shift, alt: p.alt, ..Mods::default() } });
                d.events.push(
                    json!({ "t": t, "type": "key", "name": p.name, "cmd": p.cmd, "shift": p.shift, "alt": p.alt }),
                );
                d.advance_to(t + AFTER_PRESS);
            }
            Step::Hold(names) => {
                let mut mods = Mods::default();
                for part in names.split('+').filter(|p| !p.is_empty()) {
                    match part {
                        "cmd" => mods.cmd = true,
                        "shift" => mods.shift = true,
                        "alt" => mods.alt = true,
                        other => return Err(fail(format!("journeys can't hold {other:?}"), &d)),
                    }
                }
                d.rt.physical_input(PhysicalInput::Modifiers(mods));
                d.events.push(json!({ "t": t, "hold": names }));
                d.advance_to(t + DT);
            }
            Step::Wait(s) => d.advance_to(t + s),
            Step::ScrollTo(key) => {
                d.scroll_into_view(key).map_err(|why| fail(why, &d))?;
                d.advance_to(d.now + AFTER_PRESS);
            }
            Step::WaitFor { key, text } => {
                let started = std::time::Instant::now();
                while !d.text_in(key).is_some_and(|shown| shown.contains(text.as_str())) {
                    if started.elapsed() > crate::WAIT_LIMIT {
                        let shown = d.text_in(key).unwrap_or_else(|| "nothing (no such element)".to_owned());
                        return Err(fail(format!("after {:?}, [{key}] shows {shown:?}", crate::WAIT_LIMIT), &d));
                    }
                    d.advance_to(d.now + DT);
                    std::thread::sleep(POLL);
                }
                d.advance_to(d.now + AFTER_PRESS);
            }
            Step::Mark(name) => {
                // A screenshot shows the screen at rest, not an animation's last frames.
                d.settle_motion();
                let t = d.now;
                d.events.push(json!({ "t": t, "type": "mark", "name": name }));
                let pixels = d.painter.paint(&mut d.rt);
                if pixels.is_blank() {
                    return Err(fail("its screenshot is blank: the renderer drew nothing".to_owned(), &d));
                }
                d.marks.push(Mark { name, t, tree: crate::tree::tree(&d.rt, t, d.dark), pixels });
                d.advance_to(t + AFTER_MARK);
            }
            Step::See(text) => {
                if !d.visible().iter().any(|e| e.text.as_deref().is_some_and(|s| s.contains(text.as_str()))) {
                    return Err(fail(format!("{text:?} isn't on screen"), &d));
                }
            }
            Step::SeeIn { key, text } => {
                let shown = d.text_in(key).ok_or_else(|| fail(format!("no element is keyed {key:?}"), &d))?;
                if !shown.contains(text.as_str()) {
                    return Err(fail(format!("[{key}] shows {shown:?}"), &d));
                }
            }
            Step::NotSee(text) => {
                if let Some(e) =
                    d.visible().into_iter().find(|e| e.text.as_deref().is_some_and(|s| s.contains(text.as_str())))
                {
                    return Err(fail(format!("{text:?} is still on screen, at {}", e.path), &d));
                }
            }
            Step::Gone(key) => {
                if d.rt.find(key).is_some() {
                    return Err(fail(format!("[{key}] is still there"), &d));
                }
            }
            Step::Opened(url) => {
                let asked = std::mem::take(&mut d.opened);
                if !asked.iter().any(|u| u == url) {
                    return Err(fail(format!("the app asked to open {asked:?}"), &d));
                }
            }
            Step::Copied(text) => {
                let held = d.clipboard.text().map(|t| t.to_string());
                if held.as_deref() != Some(text.as_str()) {
                    return Err(fail(format!("the clipboard holds {held:?}"), &d));
                }
                // Cleared, so the next `Copied` sees only a copy made after this one.
                d.clipboard.set_text("");
            }
            Step::Quits => {
                if d.rt.shell_request != Some(rune::Shell::Quit) {
                    return Err(fail(format!("the app asked the window for {:?}", d.rt.shell_request), &d));
                }
            }
            Step::World { check, .. } => check(world).map_err(|why| fail(why, &d))?,
        }
    }
    let end = d.now + TAIL;
    d.events.push(json!({ "t": end, "type": "mark", "name": "end" }));
    Ok(Recording { events: d.events, marks: d.marks, end })
}

struct Driver {
    rt: Runtime<App>,
    painter: crate::paint::Painter,
    clipboard: rune::clipboard::MemoryClipboard,
    /// Where a [`Step::Down`] pressed the mouse button, until [`Step::Up`] lets go.
    held: Option<(f32, f32)>,
    /// URLs the app asked to open, since the last `Opened` step looked.
    opened: Vec<String>,
    now: f64,
    /// Whether the last frame was still animating.
    animating: bool,
    dark: bool,
    events: Vec<Value>,
    marks: Vec<Mark>,
}

impl Driver {
    fn advance_to(&mut self, t: f64) {
        // Every step acts, then advances: its first frame takes the input, so it's one frame.
        let mut first = true;
        while self.now + DT / 2.0 < t {
            let frames = if std::mem::take(&mut first) { 1 } else { self.frames_until(t) };
            let dt = frames as f64 * DT;
            self.animating = self.rt.frame(dt);
            self.now += dt;
            self.opened.extend(self.rt.take_url_requests());
        }
    }

    /// How many 60 Hz frames the next frame covers. While something moves or a background read
    /// runs, one. At rest a window draws nothing until input, a timer or a read's answer wakes
    /// it, so neither does the runner: one frame spans the rest, up to the runtime's next wake
    /// (`Runtime::wake_deadline`) or `t`. That's most of a journey's time, and the screen is the
    /// same either way.
    fn frames_until(&self, t: f64) -> usize {
        if self.animating || self.rt.state.busy() {
            return 1;
        }
        let left = ((t - self.now) / DT).round();
        let wake = self.rt.wake_deadline().map_or(f64::INFINITY, |at| ((at - self.rt.clock) / DT).ceil());
        left.min(wake).max(1.0) as usize
    }

    /// Runs frames until nothing animates, for at most [`MARK_SETTLE`] (a caret blinks forever).
    fn settle_motion(&mut self) {
        let until = self.now + MARK_SETTLE;
        while self.animating && self.now + DT / 2.0 < until {
            self.advance_to(self.now + DT);
        }
    }

    /// Runs frames until no background read is in flight and none has an answer waiting. Reads
    /// run in real time, so a replay keeps pace with the wall clock (`--realtime`) to land them
    /// at the same moments.
    fn settle(&mut self) -> Result<(), String> {
        let started = std::time::Instant::now();
        // Not pending can still mean landed but not yet taken: only a frame that takes every
        // answer without asking for more, after a check that found nothing running, is settled.
        let mut quiet = 0;
        while quiet < 2 {
            if started.elapsed() > crate::WAIT_LIMIT {
                return Err(format!("a background read was still running after {:?}", crate::WAIT_LIMIT));
            }
            if self.rt.state.busy() {
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
    fn wholly_visible(&self, key: &str) -> bool {
        self.rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
            .is_none_or(|e| e.visible[3] >= e.rect[3] - 0.5)
    }

    /// Wheels, as a person would, until the element keyed `key` is wholly in view.
    fn scroll_into_view(&mut self, key: &str) -> Result<(), String> {
        for _ in 0..80 {
            let all = self.rt.elements();
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
            self.rt.pointer_move(x, y);
            self.rt.wheel(x, y, dy);
            self.events.push(json!({ "t": self.now, "type": "wheel", "x": x, "y": y, "dy": dy }));
            self.advance_to(self.now + DT * 2.0);
        }
        Ok(())
    }

    fn tap(&mut self, x: f32, y: f32) {
        self.rt.pointer_move(x, y);
        self.rt.pointer_down(x, y);
        self.rt.pointer_up(x, y);
    }

    /// Elements a person could see: on screen, not clipped away, not faded out or leaving.
    fn visible(&self) -> Vec<ElementInfo> {
        self.rt
            .elements()
            .into_iter()
            .filter(|e| !e.exiting && e.opacity > 0.01 && e.visible[2] > 0.5 && e.visible[3] > 0.5)
            .collect()
    }

    /// The centre of the smallest visible element whose text contains `text`.
    fn find_text(&self, text: &str) -> Option<(f32, f32)> {
        self.visible()
            .into_iter()
            .filter(|e| e.text.as_deref().is_some_and(|s| s.contains(text)))
            .min_by(|a, b| (a.rect[2] * a.rect[3]).total_cmp(&(b.rect[2] * b.rect[3])))
            .map(|e| (e.visible[0] + e.visible[2] / 2.0, e.visible[1] + e.visible[3] / 2.0))
    }

    /// The element keyed `key`'s label, then every visible text inside it, joined by spaces.
    fn text_in(&self, key: &str) -> Option<String> {
        let all = self.rt.elements();
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
    fn screen(&self) -> String {
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

fn key_of(p: &Press) -> Option<Key> {
    Some(match p.name {
        "Down" => Key::Down,
        "Up" => Key::Up,
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Enter" => Key::Enter,
        "Escape" => Key::Escape,
        "Tab" => Key::Tab,
        "Home" => Key::Home,
        "End" => Key::End,
        name if p.cmd && name.chars().count() == 1 => Key::Command(name.chars().next()?.to_ascii_lowercase()),
        name if name.chars().count() == 1 => Key::Text(name.to_owned()),
        _ => return None,
    })
}
