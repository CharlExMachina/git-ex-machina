//! git-ex-machina, the native app.
//!
//! `cargo run -p gxm-app` opens the window on your workspace. `cargo xtask snap` renders it
//! headless inside a fixture world; `cargo xtask journey <name>` records a journey to video
//! (journeys live in `gxm-journeys`, so this crate never links test scaffolding).

pub mod account;
pub mod icons;
pub mod keys;
pub mod load;
pub mod notes;
pub mod ops;
pub mod pull_disk;
pub mod pulls;
pub mod review;
pub mod settings;
pub mod sheet;
pub mod state;
pub mod tag;
pub mod theme;
pub mod toast;
pub mod ui;
pub mod update;
pub mod view;

use gxm_core::env::Env;
use rune::{AppSpec, Chrome, Key, KeyEvent, Tx};
use rune_kit::shell;

use crate::state::{App, Load};

pub const TITLE: &str = "git-ex-machina";
/// The window's application id, which is also the name of its desktop entry and icon. Reverse
/// DNS with no dash: KDE shortens an icon name it can't find in the person's theme at each dash,
/// before it looks in hicolor, and `git-ex-machina` came up as the theme's `git` icon.
pub const APP_ID: &str = "com.charlexmachina.GitExMachina";
/// The window's size, and the size headless renders and journeys use.
pub const SIZE: (f32, f32) = (1552.0, 852.0);
/// The traffic lights sit 12 in from the left, centred in the 48-point bar.
/// CoreText without outline dilation (what browsers call `antialiased`): lighter, crisper text.
/// Journeys draw their screenshots with it too.
pub const FONT_SMOOTHING: rune::FontSmoothing = rune::FontSmoothing::Off;
pub const CHROME: Chrome = Chrome { full_size_content: true, traffic_lights: Some((12.0, 18.0)), drag_regions: true };

fn init() -> App {
    App::new(Env::from_process())
}

/// Nothing is read before the first frame; the workspace starts loading on the first poll, off
/// the UI thread, and every read lands here.
fn poll(s: &mut App, t: f64, tx: &mut Tx) -> bool {
    if s.load == Load::NotStarted {
        s.load_workspace();
        s.resume_attachments();
        s.update_event(update::UpdateEvent::Boot, Some(tx));
        return true;
    }
    let landed = s.land_loads(tx);
    let pulled = s.land_pulls();
    if pulled {
        // A pull request's index can arrive after its commit was selected: read it now.
        s.prefetch_pull();
    }
    let sheet = s.land_sheet(tx) | s.land_tag(tx);
    let account = s.land_account(tx);
    let update = s.land_update(tx);
    let settings = s.land_settings();
    let toasts = s.toasts.tick(t, tx);
    s.land() | landed | pulled | sheet | account | update | settings | toasts
}

fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    // A sheet, a menu or the settings have the keyboard: their fields take the keys, Escape
    // dismisses.
    if !matches!(s.sheet, sheet::Sheet::Closed) || s.add_menu || s.settings.open {
        return false;
    }
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
    if s.cockpit().is_some() {
        // ⌘↩ sends the review; j and k move between the file's changes.
        if ev.key == Key::Enter && ev.mods.cmd {
            s.submit_review();
            return true;
        }
        let by = match &ev.key {
            Key::Text(t) if plain && t == "j" => 1,
            Key::Text(t) if plain && t == "k" => -1,
            _ => 0,
        };
        if by != 0 {
            if let Some(key) = s.step_change(by) {
                tx.scroll_into_view_in(keys::DIFF, key);
            }
            return true;
        }
    }
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
        setup: Some(|s: &mut App, waker| {
            s.ops.set_waker(waker.clone());
            s.pulls.set_waker(&waker);
            s.sheet_jobs.set_waker(waker.clone());
            s.tag_jobs.set_waker(waker.clone());
            s.account_jobs.set_waker(waker.clone());
            s.update_jobs.set_waker(waker.clone());
            s.settings_jobs.set_waker(&waker);
            s.loads.set_waker(waker);
        }),
        on_key: Some(on_key),
        min_size: Some((900.0, 520.0)),
        fonts: theme::FONTS,
        families: (theme::SANS, theme::MONO),
        ..AppSpec::new(TITLE, SIZE, init, view::view)
    })
    .chrome(CHROME)
    .font_smoothing(FONT_SMOOTHING)
}
