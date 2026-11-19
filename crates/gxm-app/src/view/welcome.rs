//! The window before there's a repository: what to do, and the two ways to do it. The
//! sidebar, toolbar and inspector stay away until there's something for them to act on.

use rune::{El, col, row, text};

use crate::icons;
use crate::keys;
use crate::sheet::Event;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

pub fn screen(t: &Theme) -> El {
    let add = ui::button(t, Variant::Secondary, Some(&icons::PLUS), "Add repository", None)
        .key(keys::EMPTY_ADD)
        .on_press_tx(|s: &mut App, tx| s.sheet_event(Event::OpenAdd, Some(tx)));
    let clone = ui::button(t, Variant::Primary, Some(&icons::FETCH), "Clone repository", None)
        .key(keys::EMPTY_CLONE)
        .on_press_tx(|s: &mut App, tx| s.sheet_event(Event::OpenClone, Some(tx)));
    let ask = col()
        .key(keys::EMPTY_STATE)
        .center()
        .gap(6.0)
        .child(text("Start with a repository").font(ty::DISPLAY_SM).color(t.ink))
        .child(text("Add one you have, or clone one.").font(ty::BODY).color(t.ink_muted))
        .child(row().mt(18.0).gap(8.0).child(add).child(clone));
    col()
        .w_full()
        .h_full()
        .fill(t.canvas)
        // The band the window's title bar and traffic lights sit in.
        .child(rune::el().w_full().h(size::TOOLBAR + 1.0))
        .child(col().w_full().grow(1.0).center().child(ask))
        // Balances the band above, so the question sits at the window's centre.
        .child(rune::el().w_full().h(size::TOOLBAR + 1.0))
}
