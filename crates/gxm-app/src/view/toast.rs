//! The toasts in the window's lower right corner, over everything (a sheet too), rising in from
//! the bottom edge and sinking back out.

use rune::{Edge, El, Spring, Transition, col, icon, row, text};

use crate::icons;
use crate::keys;
use crate::state::App;
use crate::theme::{Theme, size, ty};

/// Points from the window's right and bottom edges to the toasts.
const INSET: f32 = 16.0;

/// The corner, always there so a toast that closes can sink out of it.
pub fn layer(s: &App, t: &Theme) -> El {
    col()
        .key(keys::TOASTS)
        .role("region")
        .label("Notifications")
        .pin(None, None, Some(INSET), Some(INSET))
        .gap(8.0)
        .items_end()
        .children(s.toasts.shown().map(|(id, words)| toast(t, id, words)))
}

/// A check and the words, on a raised card that floats as a sheet does.
fn toast(t: &Theme, id: u64, words: &str) -> El {
    let mut card = row()
        .key(keys::toast(id))
        .role("status")
        .label(words.to_owned())
        .h(36.0)
        .pl(10.0)
        .pr(14.0)
        .gap(8.0)
        .center_y()
        .radius(size::RADIUS_MD)
        .fill(t.raised)
        .child(icon(&icons::CHECK, 14.0).color(t.added))
        .child(text(words.to_owned()).font(ty::BODY_STRONG).color(t.ink));
    for sh in t.shadow_overlay() {
        card = card.box_shadow(sh);
    }
    card.transition(Transition::move_edge(Edge::Bottom).combined(Transition::opacity())).motion(Spring::new(0.28, 0.0))
}
