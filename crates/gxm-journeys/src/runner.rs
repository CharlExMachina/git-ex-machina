//! Drives the real app through a journey, headless, and records it as a Rune input script.

use std::fmt;

use gxm_app::resolve_window::{self, ResolveLink, ResolveWindow};
use gxm_app::state::App;
use gxm_app::tag_window::{self, TagLink, TagWindow};
use gxm_world::World;
use rune::{Key, KeyEvent, Mods, PhysicalInput, TitlebarAction};

use crate::driver::{DT, Driver, Front, POLL};
use serde_json::{Value, json};

use crate::{Journey, Step};

/// When the first step happens: after the workspace loads and the first frame settles.
const START: f64 = 0.8;
/// Time after each kind of step, so animations land and a viewer can follow the video.
const AFTER_TAP: f64 = 0.7;
const PER_CHAR: f64 = 0.07;
const AFTER_TYPE: f64 = 0.5;
const AFTER_PRESS: f64 = 0.5;
/// A mark's screenshot is taken at its time; the next step waits this long so it can't leak in.
const AFTER_MARK: f64 = 0.3;
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
    let resolving = ResolveLink::default();
    rt.state.resolve_link = resolving.clone();
    let mut resolve = resolve_window::app(resolving.clone()).headless_runtime(resolve_window::SIZE, env);
    resolve.set_font_smoothing(gxm_app::FONT_SMOOTHING);
    resolve.state = ResolveWindow::new(world.env().clone(), resolving);
    // The person's own clipboard stays theirs: copies land here, for `Copied` to read.
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    tag.use_clipboard(&clipboard);
    resolve.use_clipboard(&clipboard);
    let painter = crate::paint::Painter::new(&mut rt, gxm_app::SIZE);
    let mut d = Driver {
        rt,
        tag,
        resolve,
        painter,
        tag_painter: None,
        resolve_painter: None,
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
    let mut outside = 0;
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
                let key = p.rune_key().ok_or_else(|| fail(format!("journeys can't press {:?} yet", p.name), &d))?;
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
                let tree = match d.front() {
                    Front::Tag => crate::tree::tree(&d.tag, t, d.dark),
                    Front::Resolve => crate::tree::tree(&d.resolve, t, d.dark),
                    Front::Main => crate::tree::tree(&d.rt, t, d.dark),
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
            Step::Outside { act, .. } => {
                act(world);
                d.events.push(json!({ "t": t, "type": "mark", "name": crate::outside_mark(outside) }));
                outside += 1;
                // Time for the replay to hear the mark and act before the next step.
                d.advance_to(t + AFTER_PRESS);
            }
        }
    }
    let end = d.now + TAIL;
    d.events.push(json!({ "t": end, "type": "mark", "name": "end" }));
    Ok(Recording { events: d.events, marks: d.marks, end, windows: d.windows })
}
