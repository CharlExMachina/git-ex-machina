//! GitHub sign-in on screen: the sidebar's account control, and the sheet it opens (the one-time
//! code while signing in; who's signed in, with Sign out, after).

use gxm_core::forge::model::User as Account;
use rune::{El, col, icon, row, text};

use super::history::author_color;
use crate::account::{AccountEvent, SignIn};
use crate::icons;
use crate::keys;
use crate::sheet::Event;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// Where the token is kept, as the person knows the place.
const KEYCHAIN: &str = if cfg!(target_os = "macos") { "your Keychain" } else { "your keyring" };

/// The sidebar's account control: who's signed in, or Sign in to GitHub.
pub fn control(s: &App, t: &Theme) -> El {
    let open = |b: El| b.on_press_tx(|s: &mut App, tx| s.account_event(AccountEvent::Open, Some(tx)));
    match &s.account {
        None => open(ui::button(t, Variant::Ghost, Some(&icons::PERSON), "Sign in to GitHub", None))
            .key(keys::ACCOUNT)
            .help("Sign in with a one-time code, as with gh"),
        Some(a) => open(
            row()
                .key(keys::ACCOUNT)
                .role("button")
                .h(24.0)
                .px(8.0)
                .gap(6.0)
                .center_y()
                .radius(size::RADIUS_SM)
                .hover(t.control_hover)
                .child(ui::avatar(t, display_name(a), author_color(t, &a.login)))
                .child(text(a.login.clone()).font(ty::CONTROL).color(t.ink_muted))
                .help(format!("Signed in to GitHub as {}", a.login)),
        ),
    }
}

fn display_name(a: &Account) -> &str {
    &a.name
}

/// The account sheet's title, what it's for, body and buttons.
pub fn sheet(t: &Theme, state: &SignIn) -> (String, String, Vec<El>, El) {
    let cancel = || {
        ui::button(t, Variant::Secondary, None, "Cancel", None)
            .key(keys::SHEET_CANCEL)
            .on_press(|s: &mut App| s.sheet_event(Event::Dismiss, None))
    };
    let foot = |buttons: Vec<El>| row().w_full().gap(8.0).justify_end().children(buttons);
    let muted = |words: &str| text(words.to_owned()).font(ty::BODY).color(t.ink_muted).wrap();
    let title = "Sign in to GitHub".to_owned();
    let about = format!(
        "GitHub gives a one-time code; you enter it on github.com and approve. The sign-in is kept in {KEYCHAIN}."
    );
    match state {
        SignIn::Asking => (title, about, vec![muted("Asking GitHub for a code…")], foot(vec![cancel()])),
        SignIn::Code(code) => {
            let page = code.verification_uri.trim_start_matches("https://").to_owned();
            let mut well = row().w_full().py(14.0).center().radius(size::RADIUS_MD).fill(t.well);
            for sh in t.shadow_well() {
                well = well.box_shadow(sh);
            }
            let well = well.child(text(code.user_code.clone()).key(keys::SIGNIN_CODE).font(ty::CODE).color(t.ink));
            let user_code = code.user_code.clone();
            let uri = code.verification_uri.clone();
            let copy = ui::button(t, Variant::Secondary, Some(&icons::COPY), "Copy code", None)
                .key(keys::SIGNIN_COPY)
                .on_press_tx(move |_: &mut App, tx| tx.copy(user_code.clone()));
            let open = ui::button(t, Variant::Primary, None, "Approve on GitHub", None)
                .key(keys::SIGNIN_OPEN)
                .on_press_tx(move |_: &mut App, tx| {
                    // A refused URL is GitHub's mistake, not the person's; the code still works.
                    tx.open_url(&uri).ok();
                });
            let body = vec![
                well,
                muted(&format!("Enter it at {page}. It's copied.")),
                row()
                    .gap(6.0)
                    .center_y()
                    .child(icon(&icons::SEARCH, 12.0).color(t.ink_faint))
                    .child(text("Waiting for you to approve on GitHub…").font(ty::CAPTION).color(t.ink_faint)),
            ];
            (title, about, body, foot(vec![cancel(), copy, open]))
        }
        SignIn::Failed(why) => {
            let retry = ui::button(t, Variant::Primary, None, "Try again", None)
                .key(keys::SIGNIN_RETRY)
                .on_press(|s: &mut App| s.account_event(AccountEvent::Retry, None));
            (title, about, vec![super::sheet::error(t, why)], foot(vec![cancel(), retry]))
        }
        SignIn::Account(a) | SignIn::SigningOut(a) => {
            let signing_out = matches!(state, SignIn::SigningOut(_));
            let who = row()
                .w_full()
                .gap(10.0)
                .center_y()
                .child(ui::avatar(t, display_name(a), author_color(t, &a.login)))
                .child(
                    col()
                        .gap(2.0)
                        .child(text(format!("Signed in to GitHub as {}", a.login)).font(ty::BODY_STRONG).color(t.ink))
                        .child(text(display_name(a).to_owned()).font(ty::CAPTION).color(t.ink_muted)),
                );
            let out = ui::button_in(t, Variant::Secondary, None, "Sign out", None, signing_out).key(keys::SIGN_OUT);
            let out = if signing_out {
                out.disabled(true)
            } else {
                out.on_press(|s: &mut App| s.account_event(AccountEvent::SignOut, None))
            };
            let done = ui::button(t, Variant::Primary, None, "Done", None)
                .key(keys::SHEET_CANCEL)
                .on_press(|s: &mut App| s.sheet_event(Event::Dismiss, None));
            let about = format!(
                "The sign-in is kept in {KEYCHAIN}. Signing out forgets it here; GitHub keeps the authorisation until you revoke it in its settings."
            );
            ("GitHub".to_owned(), about, vec![who], foot(vec![out, done]))
        }
    }
}
