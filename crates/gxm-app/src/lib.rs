//! git-ex-machina, the native app.
//!
//! `cargo run -p gxm-app` opens the window on your workspace. `cargo xtask snap` renders it
//! headless inside a fixture world; `cargo xtask journey <name>` records a journey to video
//! (journeys live in `gxm-journeys`, so this crate never links test scaffolding).

pub mod icons;
pub mod keys;
pub mod ops;
pub mod state;
pub mod theme;
pub mod ui;
pub mod view;

use gxm_core::env::Env;
use rune::{AppSpec, Chrome, Key, KeyEvent, Tx};
use rune_kit::shell;

use crate::state::{App, Load};

pub const TITLE: &str = "git-ex-machina";
/// The window's size, and the size headless renders and journeys use.
pub const SIZE: (f32, f32) = (1552.0, 852.0);
/// The traffic lights sit 12 in from the left, centred in the 48-point bar.
pub const CHROME: Chrome = Chrome { full_size_content: true, traffic_lights: Some((12.0, 18.0)), drag_regions: true };

fn init() -> App {
    App::new(Env::from_process())
}

/// Nothing is read before the first frame; the workspace loads on the first poll.
fn poll(s: &mut App, _t: f64, tx: &mut Tx) -> bool {
    if s.load == Load::NotStarted {
        s.load_workspace();
        return true;
    }
    s.land(tx)
}

fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if shell::is_sidebar_toggle(ev) {
        s.sidebar_open = !s.sidebar_open;
        return true;
    }
    if ev.is_command('i') && ev.mods.alt {
        s.inspector_open = !s.inspector_open;
        return true;
    }
    if ev.is_command('k') {
        tx.focus(keys::REPO_FILTER, true);
        return true;
    }
    if ev.is_command('f') {
        tx.focus(keys::COMMIT_SEARCH, true);
        return true;
    }
    let plain = !ev.mods.cmd && !ev.mods.alt && !ev.mods.ctrl && !ev.mods.shift;
    let by = match ev.key {
        Key::Down if plain => 1,
        Key::Up if plain => -1,
        _ => return false,
    };
    if let Some(id) = s.step(by) {
        tx.scroll_into_view_in(keys::HISTORY, id);
    }
    true
}

/// The app as both the window and headless runs build it.
pub fn app() -> rune::App<App> {
    rune::App::new(AppSpec {
        poll: Some(poll),
        setup: Some(|s: &mut App, waker| s.ops.set_waker(waker)),
        on_key: Some(on_key),
        min_size: Some((900.0, 520.0)),
        fonts: theme::FONTS,
        families: (theme::SANS, theme::MONO),
        ..AppSpec::new(TITLE, SIZE, init, view::view)
    })
    .chrome(CHROME)
    // CoreText without outline dilation (what browsers call `antialiased`): lighter, crisper text.
    .font_smoothing(rune::FontSmoothing::Off)
}
