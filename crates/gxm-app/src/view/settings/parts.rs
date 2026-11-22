//! The settings dialog's pieces, built from the app's components: a group under its label, a
//! setting that's a checkbox, a choice of a few, a field, a list of rows, a chip, and the tag
//! that says where a setting lives.

use rune::{El, Icon, TextEdit, col, icon, row, text, text_input};

use crate::icons;
use crate::keys::{setting, setting_field, setting_item};
use crate::settings::Action;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// What a control does when pressed (or Return in a field).
pub type Handler = Box<dyn Fn(&mut App)>;

/// A chip's remove button: its key and what it does.
pub type Remove = Option<(String, Handler)>;

/// Where a setting is kept.
#[derive(Clone, Copy)]
pub enum Place {
    /// On the provider (`GitHub`), for everyone.
    Provider(&'static str),
    /// In this repository's files on this computer.
    Here,
}

/// `☁ On GitHub` or `▭ This computer`, beside a setting.
pub fn tag(t: &Theme, place: Place) -> El {
    let (glyph, words): (&'static Icon, String) = match place {
        Place::Provider(p) => (&icons::CLOUD, format!("On {p}")),
        Place::Here => (&icons::LAPTOP, "This computer".to_owned()),
    };
    row()
        .gap(4.0)
        .center_y()
        .child(icon(glyph, 12.0).color(t.ink_muted))
        .child(text(words).font(ty::SMALL).color(t.ink_muted))
}

/// A group of settings under its label.
pub fn group(t: &Theme, label: &str, children: Vec<El>) -> El {
    col().w_full().gap(12.0).child(ui::label(t, label)).children(children)
}

/// What a setting is, and a line on what it does.
fn words(t: &Theme, title: &str, hint: Option<&str>) -> El {
    let mut c = col().grow(1.0).min_w(0.0).child(text(title.to_owned()).font(ty::BODY).color(t.ink).wrap());
    if let Some(h) = hint {
        c = c.child(text(h.to_owned()).font(ty::HINT).color(t.ink_muted).wrap());
    }
    c
}

/// A checkbox setting, keyed `setting-{id}`: pressed, it does what `act` makes of its new state.
/// Locked (no rights, no provider), it shows its state and does nothing.
#[expect(clippy::too_many_arguments, reason = "a setting row's every part, named at the call")]
pub fn toggle(
    t: &Theme,
    id: &str,
    on: bool,
    title: &str,
    hint: Option<&str>,
    place: Option<Place>,
    enabled: bool,
    act: impl Fn(bool) -> Action + 'static,
) -> El {
    let box_ = ui::check_box(t, on);
    let mut r = row()
        .key(setting(id))
        .w_full()
        .gap(12.0)
        .role("checkbox")
        .label(title.to_owned())
        .child(row().pt(1.0).child(box_))
        .child(words(t, title, hint));
    if let Some(p) = place {
        r = r.child(tag(t, p));
    }
    if enabled { r.on_press(move |s: &mut App| s.settings_do(act(!on))) } else { r.disabled(true).opacity(0.5) }
}

/// A checkbox that changes something in the dialog itself rather than a setting.
pub fn check(t: &Theme, id: &str, on: bool, title: &str, f: impl Fn(&mut App) + 'static) -> El {
    ui::check(t, on, title).key(setting(id)).on_press(f)
}

/// A choice of a few, as a segmented control: each option keyed `setting-{id}-{option}`.
pub fn choice<F>(t: &Theme, id: &str, options: &[(&str, &str, bool)], enabled: bool, f: F) -> El
where
    F: Fn(&mut App, usize) + Clone + 'static,
{
    let opts = options
        .iter()
        .enumerate()
        .map(|(i, (opt, label, selected))| {
            let mut o = ui::segment_label(t, label, *selected).key(setting(&format!("{id}-{opt}"))).role("button");
            if enabled && !*selected {
                let f = f.clone();
                o = o.on_press(move |s: &mut App| f(s, i));
            }
            (o, *selected)
        })
        .collect();
    let c = row().child(ui::segmented(t, opts));
    if enabled { c } else { c.disabled(true).opacity(0.5) }
}

/// A text field keyed `setting-field-{id}`, editing what `access` returns; Return does `submit`.
#[expect(clippy::too_many_arguments, reason = "a field's every part, named at the call")]
pub fn field(
    t: &Theme,
    id: &str,
    edit: &TextEdit,
    placeholder: &str,
    enabled: bool,
    secure: bool,
    access: impl for<'a> Fn(&'a mut App) -> &'a mut TextEdit + 'static,
    submit: Option<Handler>,
) -> El {
    let mut input = text_input(edit, ty::BODY)
        .key(setting_field(id))
        .label(placeholder.to_owned())
        .on_edit(access)
        .single_line()
        .placeholder(placeholder.to_owned(), t.ink_faint)
        .color(t.ink)
        .grow(1.0)
        .min_w(0.0);
    if secure {
        input = input.secure();
    }
    if let Some(f) = submit {
        input = input.on_submit(move |s: &mut App, _| f(s));
    }
    let mut w = row().h(28.0).px(8.0).center_y().radius(size::RADIUS_SM).fill(t.well).grow(1.0).min_w(0.0);
    for sh in t.shadow_well() {
        w = w.box_shadow(sh);
    }
    let w = w.role("textbox").child(input);
    if enabled { w } else { w.disabled(true).opacity(0.5) }
}

/// What Return does in a field.
pub fn on(f: impl Fn(&mut App) + 'static) -> Option<Handler> {
    Some(Box::new(f))
}

/// A field under its label, 96 wide on the left.
pub fn labelled(t: &Theme, label: &str, field: El) -> El {
    row()
        .w_full()
        .gap(12.0)
        .center_y()
        .child(text(label.to_owned()).font(ty::BODY).color(t.ink_muted).w(96.0))
        .child(field)
}

/// A button keyed `setting-{id}`; held off (dimmed, deaf) when not `enabled`.
pub fn button(t: &Theme, id: &str, label: &str, variant: Variant, enabled: bool, f: impl Fn(&mut App) + 'static) -> El {
    let b = ui::button(t, variant, None, label, None).key(setting(id));
    if enabled { b.on_press(f) } else { b.disabled(true).opacity(0.5) }
}

/// An icon-only button keyed `setting-{id}`, named for accessibility.
pub fn icon_button(
    t: &Theme,
    id: &str,
    glyph: &'static Icon,
    name: &str,
    enabled: bool,
    f: impl Fn(&mut App) + 'static,
) -> El {
    let b = ui::button(t, Variant::Ghost, Some(glyph), "", None)
        .key(setting(id))
        .label(name.to_owned())
        .help(name.to_owned());
    if enabled { b.on_press(f) } else { b.disabled(true).opacity(0.4) }
}

/// Rows in a card, a hairline between each.
pub fn list(t: &Theme, rows: Vec<El>) -> El {
    let mut card = col().w_full().radius(size::RADIUS_LG).fill(t.canvas).box_shadow(Theme::ring(1.0, t.line));
    for (i, r) in rows.into_iter().enumerate() {
        let r = r.w_full().px(14.0).py(10.0);
        card = card.child(if i == 0 { r } else { r.border_sides([1.0, 0.0, 0.0, 0.0], t.line) });
    }
    card
}

/// A list row keyed `setting-item-{kind}-{id}`.
pub fn item(kind: &str, id: &str) -> El {
    row().key(setting_item(kind, id)).gap(12.0).center_y()
}

/// A name in a row, with what it is underneath.
pub fn named(t: &Theme, name: &str, about: &str, mono: bool) -> El {
    col()
        .grow(1.0)
        .min_w(0.0)
        .child(text(name.to_owned()).font(if mono { ty::MONO_STRONG } else { ty::BODY_STRONG }).color(t.ink).truncate())
        .child(text(about.to_owned()).font(ty::HINT).color(t.ink_muted).truncate())
}

/// A small rounded label, keyed `setting-item-{kind}-{id}`, with a remove button keyed
/// `setting-{remove}` when it can go.
pub fn chip(t: &Theme, kind: &str, id: &str, remove: Remove) -> El {
    let mut c = row()
        .key(setting_item(kind, id))
        .h(24.0)
        .pl(10.0)
        .pr(if remove.is_some() { 4.0 } else { 10.0 })
        .gap(4.0)
        .center_y()
        .radius(size::PILL)
        .fill(t.control_hover)
        .child(text(id.to_owned()).font(ty::CONTROL).color(t.ink));
    if let Some((key, f)) = remove {
        c = c.child(
            row()
                .key(setting(&key))
                .role("button")
                .label(format!("Remove {id}"))
                .size(16.0, 16.0)
                .center()
                .radius(8.0)
                .hover(t.control)
                .child(icon(&icons::X, 10.0).color(t.ink_muted))
                .on_press(move |s: &mut App| f(s)),
        );
    }
    c
}

/// Words in the page's muted voice.
pub fn note(t: &Theme, s: &str) -> El {
    text(s.to_owned()).font(ty::CAPTION).color(t.ink_muted).wrap()
}

/// A row of controls, left to right.
pub fn controls(children: Vec<El>) -> El {
    row().w_full().gap(8.0).center_y().children(children)
}
