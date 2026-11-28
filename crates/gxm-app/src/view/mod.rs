//! The window: the repository sidebar (248 + its border), the history, and the inspector
//! (360 + its border), each under its own 48-point bar.

mod account;
mod checkout;
mod cockpit;
mod cockpit_side;
mod graph;
mod history;
mod inspector;
mod notes;
mod on_graph;
mod picker;
mod pull;
mod pull_parts;
mod settings;
mod sheet;
mod sidebar;
mod tag;
mod toast;
mod toolbar;
mod update;
mod welcome;
mod work;
mod work_inspector;

use rune::{Cx, El, row, text};

use crate::state::{App, Load, View};
use crate::theme::{Theme, size, ty};

/// The sidebar's and inspector's widths with their 1-point borders (the CSS sizes the content
/// box, so the border adds to it).
const SIDEBAR_W: f32 = size::SIDEBAR + 1.0;
const INSPECTOR_W: f32 = size::INSPECTOR + 1.0;

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let t = Theme::new(cx.system_dark());
    if s.load == Load::Loaded && s.repos.is_empty() {
        return welcome::screen(s, &t).children(sheet::layer(s, &t)).child(toast::layer(s, &t));
    }
    let cockpit = s.cockpit().is_some();
    let work = s.view == View::Work && !cockpit;
    let main = match &s.load {
        Load::Failed(e) => message(&t, e),
        Load::NotStarted | Load::Loading | Load::Loaded if cockpit => cockpit::pane(s, &t),
        Load::NotStarted | Load::Loading | Load::Loaded if work => work::pane(s, &t),
        Load::NotStarted | Load::Loading | Load::Loaded => history::pane(s, &t),
    };
    let mut shell = row().w_full().h_full().fill(t.chrome);
    if s.sidebar_open {
        let side = if cockpit { cockpit_side::sidebar(s, &t) } else { sidebar::sidebar(s, &t) };
        shell = shell.child(side.w(SIDEBAR_W).h_full());
    }
    shell = shell.child(main.grow(1.0).min_w(0.0).h_full());
    if s.inspector_open {
        let right = match s.pull_view() {
            _ if work => work_inspector::inspector(s, &t),
            Some(_) if cockpit => cockpit_side::review_panel(s, &t),
            Some(v) => pull::inspector(s, v, &t),
            None => inspector::inspector(s, &t),
        };
        shell = shell.child(right.w(INSPECTOR_W).h_full());
    }
    if s.add_menu && s.sidebar_open && !cockpit {
        shell = shell.children(sidebar::add_menu(&t));
    }
    if s.checkout_menu && !cockpit {
        shell = shell.children(checkout::menu(s, &t));
    }
    shell.children(settings::layer(s, &t)).children(sheet::layer(s, &t)).child(toast::layer(s, &t))
}

/// The tag window (`crate::tag_window`).
pub fn tag_window(s: &crate::tag_window::TagWindow, cx: &mut Cx<crate::tag_window::TagWindow>) -> El {
    tag::window(s, cx)
}

fn message(t: &Theme, e: &str) -> El {
    rune::col().center().fill(t.canvas).child(text(e.to_owned()).font(ty::BODY).color(t.removed).wrap())
}

/// A 48-point bar with a bottom rule, as each column's top bar is drawn. The window draws no
/// title bar of its own over its content (`CHROME`), so these bars are its title bar: a press on
/// any part without a control of its own moves the window, and a double click zooms it.
fn bar(t: &Theme) -> El {
    row()
        .window_drag()
        .w_full()
        .h(size::TOOLBAR + 1.0)
        .center_y()
        .gap(8.0)
        .fill(t.chrome)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout()
}
