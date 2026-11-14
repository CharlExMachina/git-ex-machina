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
/// One wheel step of a [`Step::ScrollTo`], in points.
const WHEEL: f32 = 60.0;
/// The video runs this long past the last step.
const TAIL: f64 = 1.2;

/// A journey as Rune input: replay it with `--script` to get the same run on screen.
#[derive(Clone, Debug)]
pub struct Recording {
    pub events: Vec<Value>,
    pub marks: Vec<(&'static str, f64)>,
    pub end: f64,
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
    let env = rune::Env { system_dark: j.dark, scale: 1.0 };
    let mut rt = gxm_app::app().headless_runtime(gxm_app::SIZE, env);
    rt.state = App::new(world.env().clone());
    let mut d = Driver { rt, now: 0.0, events: Vec::new(), marks: Vec::new() };
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
                for _ in 0..80 {
                    let e = d
                        .rt
                        .elements()
                        .into_iter()
                        .find(|e| e.key.as_deref() == Some(key.as_str()) && !e.exiting)
                        .ok_or_else(|| fail(format!("no element is keyed {key:?}"), &d))?;
                    if e.visible[3] >= e.rect[3] - 0.5 {
                        break;
                    }
                    // Positive moves the content down, showing what's above.
                    let dy = if e.rect[1] < e.visible[1] { WHEEL } else { -WHEEL };
                    let (x, y) = (e.rect[0] + e.rect[2] / 2.0, (e.rect[1]).clamp(80.0, gxm_app::SIZE.1 - 40.0));
                    d.rt.pointer_move(x, y);
                    d.rt.wheel(x, y, dy);
                    d.events.push(json!({ "t": d.now, "type": "wheel", "x": x, "y": y, "dy": dy }));
                    d.advance_to(d.now + DT * 2.0);
                }
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
                d.events.push(json!({ "t": t, "type": "mark", "name": name }));
                d.marks.push((name, t));
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
            Step::World { check, .. } => check(world).map_err(|why| fail(why, &d))?,
        }
    }
    let end = d.now + TAIL;
    d.events.push(json!({ "t": end, "type": "mark", "name": "end" }));
    Ok(Recording { events: d.events, marks: d.marks, end })
}

struct Driver {
    rt: Runtime<App>,
    now: f64,
    events: Vec<Value>,
    marks: Vec<(&'static str, f64)>,
}

impl Driver {
    fn advance_to(&mut self, t: f64) {
        while self.now + DT / 2.0 < t {
            self.rt.frame(DT);
            self.now += DT;
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
            if self.rt.state.loads.busy() {
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

    /// Every visible text inside the element keyed `key`, joined by spaces.
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
        let texts: Vec<&str> = (0..all.len())
            .filter(|&i| inside(i) && all[i].opacity > 0.01 && all[i].visible[2] > 0.5)
            .filter_map(|i| all[i].text.as_deref())
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
