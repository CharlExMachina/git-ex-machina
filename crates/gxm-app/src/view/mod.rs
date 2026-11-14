//! The window, laid out as the design's AppShell (`design/system/appshell-markup.html`): the
//! repository sidebar (248 + its border), the history, and the inspector (360 + its border),
//! each under its own 48-point bar.

mod graph;
mod history;
mod inspector;
mod sidebar;
mod toolbar;

use rune::{Cx, El, row, text};

use crate::state::{App, Load};
use crate::theme::{Theme, size, ty};

/// The sidebar's and inspector's widths with their 1-point borders (the CSS sizes the content
/// box, so the border adds to it).
const SIDEBAR_W: f32 = size::SIDEBAR + 1.0;
const INSPECTOR_W: f32 = size::INSPECTOR + 1.0;

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let t = Theme::new(cx.system_dark());
    let main = match &s.load {
        Load::Failed(e) => message(&t, e),
        Load::NotStarted | Load::Loaded => history::pane(s, &t),
    };
    let mut shell = row().w_full().h_full().fill(t.chrome);
    if s.sidebar_open {
        shell = shell.child(sidebar::sidebar(s, &t).w(SIDEBAR_W).h_full());
    }
    shell = shell.child(main.grow(1.0).min_w(0.0).h_full());
    if s.inspector_open {
        shell = shell.child(inspector::inspector(s, &t).w(INSPECTOR_W).h_full());
    }
    shell
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
