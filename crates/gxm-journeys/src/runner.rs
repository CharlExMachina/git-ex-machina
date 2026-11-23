//! Drives the real app through a journey, headless, and records it as a Rune input script.

use std::fmt;

use gxm_app::state::App;
use gxm_app::tag_window::{self, TagLink, TagWindow};
use gxm_world::World;
use rune::{ElementInfo, Key, KeyEvent, Mods, PhysicalInput, Runtime, TitlebarAction};

use crate::window::Window;
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
    /// A window besides the main one showed: the replay records the desktop, every window on it.
    pub windows: bool,
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
    // The main window, and the tag window it opens: each its own runtime, as each is its own
    // window, linked as the app links them.
    let link = TagLink::default();
    let mut rt = gxm_app::main_app().headless_runtime(gxm_app::SIZE, env);
    rt.set_font_smoothing(gxm_app::FONT_SMOOTHING);
    rt.state = App::new(world.env().clone());
    rt.state.tag_link = link.clone();
    let mut tag = tag_window::app(link.clone()).headless_runtime(tag_window::SIZE, env);
    tag.set_font_smoothing(gxm_app::FONT_SMOOTHING);
    tag.state = TagWindow::new(world.env().clone(), link);
    // The person's own clipboard stays theirs: copies land here, for `Copied` to read.
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    tag.use_clipboard(&clipboard);
    let painter = crate::paint::Painter::new(&mut rt, gxm_app::SIZE);
    let mut d = Driver {
        rt,
        tag,
        painter,
        tag_painter: None,
        windows: false,
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
                let (x, y) = d.win().find(key).ok_or_else(|| fail(format!("no element is keyed {key:?}"), &d))?;
                d.tap(x, y);
                d.events.push(json!({ "t": t, "type": "tap", "key": key }));
                d.advance_to(t + AFTER_TAP);
            }
            Step::TapText(text) => {
                let (x, y) = d.find_text(text).ok_or_else(|| fail(format!("nothing on screen shows {text:?}"), &d))?;
                d.tap(x, y);
                let (ox, oy) = d.origin();
                d.events.push(json!({ "t": t, "type": "tap", "x": x + ox, "y": y + oy }));
                d.advance_to(t + AFTER_TAP);
            }
            Step::Down(key) => {
                if !d.wholly_visible(key) {
                    d.scroll_into_view(key).map_err(|why| fail(why, &d))?;
                }
                let t = d.now;
                let (x, y) = d.win().find(key).ok_or_else(|| fail(format!("no element is keyed {key:?}"), &d))?;
                d.win_mut().pointer_move(x, y);
                d.win_mut().pointer_down(x, y);
                d.held = Some((x, y));
                d.events.push(json!({ "t": t, "type": "down", "key": key }));
                d.advance_to(t + AFTER_PRESS);
            }
            Step::Up => {
                let (x, y) = d.held.take().ok_or_else(|| fail("no mouse button is held".to_owned(), &d))?;
                d.win_mut().pointer_up(x, y);
                d.events.push(json!({ "t": t, "type": "up" }));
                d.advance_to(t + AFTER_TAP);
            }
            Step::Type(text) => {
                d.events.push(json!({ "t": t, "type": "type", "text": text }));
                for ch in text.chars() {
                    d.win_mut().key(KeyEvent { key: Key::Text(ch.to_string()), mods: Mods::default() });
                    d.advance_to(d.now + PER_CHAR);
                }
                d.advance_to(t + PER_CHAR * text.chars().count() as f64 + AFTER_TYPE);
            }
            Step::Drop { key, path } => {
                let (x, y) = d.win().find(key).ok_or_else(|| fail(format!("no element is keyed {key:?}"), &d))?;
                d.win_mut().drop_files(vec![world.code().join(path)], Some((x, y)));
                // A replay runs in its own world's `~/code`, where the same file is.
                d.events.push(json!({ "t": t, "drop_files": { "paths": [path], "key": key } }));
                d.advance_to(t + AFTER_TAP);
            }
            Step::Chooser { title, starts_in } => {
                let (shown, start) = d.win().folder_chooser().map_err(|why| fail(why, &d))?;
                let home = &world.env().home;
                let start = start.map(|dir| world.env().tilde(&dir));
                let want = world.env().tilde(&home.join(starts_in));
                if shown != *title || start.as_deref() != Some(want.as_str()) {
                    return Err(fail(format!("the chooser is titled {shown:?}, starting in {start:?}"), &d));
                }
            }
            Step::Choose(folder) => {
                d.win().folder_chooser().map_err(|why| fail(why, &d))?;
                // The app gets the full path, as a real chooser gives it; the replay's script names
                // it from the home folder, which the app reads in its own world's home.
                let picked = folder.map(|f| world.env().home.join(f));
                d.win_mut().answer_dialog(picked).map_err(|why| fail(why, &d))?;
                d.events.push(match folder {
                    Some(f) => json!({ "t": t, "type": "file-dialog", "path": f }),
                    None => json!({ "t": t, "type": "file-dialog-cancel" }),
                });
                d.advance_to(t + AFTER_TAP);
            }
            Step::Press(p) => {
                let key = key_of(p).ok_or_else(|| fail(format!("journeys can't press {:?} yet", p.name), &d))?;
                d.win_mut()
                    .key(KeyEvent { key, mods: Mods { cmd: p.cmd, shift: p.shift, alt: p.alt, ..Mods::default() } });
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
                d.win_mut().physical_input(PhysicalInput::Modifiers(mods));
                d.events.push(json!({ "t": t, "hold": names }));
                d.advance_to(t + DT);
            }
            Step::Wait(s) => d.advance_to(t + s),
            Step::ScrollTo(key) => {
                d.scroll_into_view(key).map_err(|why| fail(why, &d))?;
                d.advance_to(d.now + AFTER_PRESS);
            }
            Step::MovesWindow(key) => {
                let (x, y) = crate::window::empty_spot(d.win(), key)
                    .ok_or_else(|| fail(format!("[{key}] has no empty stretch to grab"), &d))?;
                // Left out of the replay's script: a double press there would zoom the very
                // window the video records.
                let mut asked = Vec::new();
                for _ in 0..2 {
                    d.win_mut().pointer_move(x, y);
                    d.win_mut().pointer_down(x, y);
                    asked.push(d.win_mut().take_titlebar_action());
                    d.win_mut().pointer_up(x, y);
                }
                if asked != [Some(TitlebarAction::Drag), Some(TitlebarAction::DoubleClick)] {
                    return Err(fail(format!("pressing it twice at ({x}, {y}) asked the window for {asked:?}"), &d));
                }
                d.advance_to(t + AFTER_TAP);
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
                let pixels = d.paint();
                if pixels.is_blank() {
                    return Err(fail("its screenshot is blank: the renderer drew nothing".to_owned(), &d));
                }
                let tree = if d.tag_front() {
                    crate::tree::tree(&d.tag, t, d.dark)
                } else {
                    crate::tree::tree(&d.rt, t, d.dark)
                };
                d.marks.push(Mark { name, t, tree, pixels });
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
                if d.win().find(key).is_some() {
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
    Ok(Recording { events: d.events, marks: d.marks, end, windows: d.windows })
}

struct Driver {
    rt: Runtime<App>,
    /// The tag window, in front of the main one while it shows.
    tag: Runtime<TagWindow>,
    painter: crate::paint::Painter,
    tag_painter: Option<crate::paint::Painter>,
    /// The tag window has shown: the video records every window, as a screen recording would.
    windows: bool,
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
    /// Whether the tag window is in front, with the keyboard.
    fn tag_front(&self) -> bool {
        self.tag.state.shown
    }

    /// The window in front: where input goes and what expectations look at.
    fn win(&self) -> &dyn Window {
        if self.tag_front() { &self.tag } else { &self.rt }
    }

    fn win_mut(&mut self) -> &mut dyn Window {
        if self.tag_front() { &mut self.tag } else { &mut self.rt }
    }

    /// Where the front window's top left corner is in the main window's points, as a desktop
    /// recording places it: the tag window centred over the main one.
    fn origin(&self) -> (f32, f32) {
        if !self.tag_front() {
            return (0.0, 0.0);
        }
        let (w, h) = self.tag.viewport;
        (((gxm_app::SIZE.0 - w) / 2.0).round(), ((gxm_app::SIZE.1 - h) / 2.0).round())
    }

    /// Either window is waiting on work off the UI thread, or on the other.
    fn busy(&self) -> bool {
        self.rt.state.busy() || self.tag.state.busy() || self.rt.state.tag_link.done.pending()
    }

    /// The screen: the main window, with the tag window over it while it shows.
    fn paint(&mut self) -> crate::paint::Pixels {
        let mut pixels = self.painter.paint(&mut self.rt);
        if self.tag_front() {
            let tag = &mut self.tag;
            let painter = self.tag_painter.get_or_insert_with(|| crate::paint::Painter::new(tag, tag_window::SIZE));
            let window = painter.paint(tag);
            pixels.composite(&window, self.origin());
        }
        pixels
    }

    fn advance_to(&mut self, t: f64) {
        // Every step acts, then advances: its first frame takes the input, so it's one frame.
        let mut first = true;
        while self.now + DT / 2.0 < t {
            let frames = if std::mem::take(&mut first) { 1 } else { self.frames_until(t) };
            let dt = frames as f64 * DT;
            let tag = self.tag.frame(dt);
            self.animating = self.rt.frame(dt) | tag;
            self.now += dt;
            self.opened.extend(self.rt.take_url_requests());
            self.opened.extend(self.tag.take_url_requests());
            self.windows |= self.tag.state.shown;
        }
    }

    /// How many 60 Hz frames the next frame covers. While something moves or a background read
    /// runs, one. At rest a window draws nothing until input, a timer or a read's answer wakes
    /// it, so neither does the runner: one frame spans the rest, up to the runtime's next wake
    /// (`Runtime::wake_deadline`) or `t`. That's most of a journey's time, and the screen is the
    /// same either way.
    fn frames_until(&self, t: f64) -> usize {
        if self.animating || self.busy() {
            return 1;
        }
        let left = ((t - self.now) / DT).round();
        let wake = [
            self.rt.wake_deadline().map(|at| at - self.rt.clock),
            self.tag.wake_deadline().map(|at| at - self.tag.clock),
        ]
        .into_iter()
        .flatten()
        .fold(f64::INFINITY, |soonest, left| soonest.min((left / DT).ceil()));
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
    fn wholly_visible(&self, key: &str) -> bool {
        self.win()
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
            .is_none_or(|e| e.visible[3] >= e.rect[3] - 0.5)
    }

    /// Wheels, as a person would, until the element keyed `key` is wholly in view.
    fn scroll_into_view(&mut self, key: &str) -> Result<(), String> {
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

    fn tap(&mut self, x: f32, y: f32) {
        let win = self.win_mut();
        win.pointer_move(x, y);
        win.pointer_down(x, y);
        win.pointer_up(x, y);
    }

    /// Elements a person could see: on screen, not clipped away, not faded out or leaving.
    fn visible(&self) -> Vec<ElementInfo> {
        self.win()
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
