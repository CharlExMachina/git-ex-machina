//! The window: the repository sidebar (248 + its border), the history, and the inspector
//! (360 + its border), each under its own 48-point bar.

mod account;
mod cockpit;
mod cockpit_side;
mod graph;
mod history;
mod inspector;
mod on_graph;
mod pull;
mod pull_parts;
mod settings;
mod sheet;
mod sidebar;
mod toolbar;
mod welcome;

use rune::{Cx, El, row, text};

use crate::state::{App, Load};
use crate::theme::{Theme, size, ty};

/// The sidebar's and inspector's widths with their 1-point borders (the CSS sizes the content
/// box, so the border adds to it).
const SIDEBAR_W: f32 = size::SIDEBAR + 1.0;
const INSPECTOR_W: f32 = size::INSPECTOR + 1.0;

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let t = Theme::new(cx.system_dark());
    if s.load == Load::Loaded && s.repos.is_empty() {
        return welcome::screen(s, &t).children(sheet::layer(s, &t));
    }
    let cockpit = s.cockpit().is_some();
    let main = match &s.load {
        Load::Failed(e) => message(&t, e),
        Load::NotStarted | Load::Loading | Load::Loaded if cockpit => cockpit::pane(s, &t),
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
            Some(_) if cockpit => cockpit_side::review_panel(s, &t),
            Some(v) => pull::inspector(s, v, &t),
            None => inspector::inspector(s, &t),
        };
        shell = shell.child(right.w(INSPECTOR_W).h_full());
    }
    shell.children(settings::layer(s, &t)).children(sheet::layer(s, &t))
}

fn message(t: &Theme, e: &str) -> El {
    rune::col().center().fill(t.canvas).child(text(e.to_owned()).font(ty::BODY).color(t.removed).wrap())
}

/// A 48-point bar with a bottom rule, as each column's top bar is drawn.
fn bar(t: &Theme) -> El {
    row()
        .w_full()
        .h(size::TOOLBAR + 1.0)
        .center_y()
        .gap(8.0)
        .fill(t.chrome)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout()
}
