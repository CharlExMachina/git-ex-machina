//! The app's components, one function each: buttons, fields, segmented controls, badges, tiles.
//! New UI composes these rather than styling elements inline, so the app keeps one look.

use rune::{Color, El, Icon, TextEdit, icon, row, text, text_input};

use crate::icons;
use crate::theme::{Theme, size, ty};

/// `.gu-btn` variants.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Secondary,
    Primary,
    Ghost,
}

/// `Button` at `size="sm"`: 24 tall, 8 a side, radius 6, 12px medium, icon 14, gap 6.
pub fn button(t: &Theme, variant: Variant, glyph: Option<&'static Icon>, label: &str, badge: Option<String>) -> El {
    button_in(t, variant, glyph, label, badge, false)
}

/// [`button`], held down (`.gu-btn:active`): the pressed shadow, half a point lower.
pub fn button_in(
    t: &Theme,
    variant: Variant,
    glyph: Option<&'static Icon>,
    label: &str,
    badge: Option<String>,
    pressed: bool,
) -> El {
    let ink = match variant {
        Variant::Primary => t.on_accent,
        Variant::Secondary => t.ink,
        Variant::Ghost => t.ink_muted,
    };
    let only_icon = label.is_empty();
    let mut b = row().h(24.0).gap(6.0).center_y().radius(size::RADIUS_SM);
    b = if only_icon { b.w(24.0).center() } else { b.px(8.0) };
    b = match variant {
        Variant::Primary => b.fill(t.accent).gradient(t.accent_top, t.accent).hover(t.accent_top),
        Variant::Secondary => b.fill(t.control).gradient(t.control, t.control_bottom).hover(t.control_hover),
        Variant::Ghost => b.hover(t.control_hover),
    };
    if pressed {
        b = b.mt(0.5);
        for s in t.shadow_pressed() {
            b = b.box_shadow(s);
        }
    } else if variant == Variant::Primary {
        for s in t.shadow_primary() {
            b = b.box_shadow(s);
        }
    } else if variant == Variant::Secondary {
        for s in t.shadow_raised() {
            b = b.box_shadow(s);
        }
    }
    if let Some(g) = glyph {
        b = b.child(icon(g, 14.0).color(ink));
    }
    if !only_icon {
        b = b.child(text(label.to_owned()).font(ty::CONTROL).color(ink));
    }
    if let Some(n) = badge {
        // `.gu-btn-badge`, and its `.gu-btn-primary` colours.
        let (bg, fg) =
            if variant == Variant::Primary { (t.on_accent, t.accent_top) } else { (t.accent_soft, t.accent) };
        let mut pill = row().h(16.0).px(5.0).radius(size::PILL).center().fill(bg);
        if variant == Variant::Primary {
            pill = pill.box_shadow(t.shadow_badge());
        }
        b = b.child(pill.child(text(n).font(ty::MONO_11).color(fg)));
    }
    b.role("button")
}

/// `.gu-kbd`: mono 11/16, 5 a side, radius 4, on a raised control.
pub fn kbd(t: &Theme, keys: &str) -> El {
    let mut k = row().h(16.0).px(5.0).radius(size::RADIUS_XS).center().fill(t.control);
    for s in t.shadow_raised() {
        k = k.box_shadow(s);
    }
    k.child(text(keys.to_owned()).font(ty::MONO_11).color(t.ink_muted))
}

/// `SearchField`: a 28-tall well, radius 6, 8 a side, a 14 search icon, the field (keyed `key`,
/// editing what `access` returns, with whatever `handlers` attaches), a shortcut.
pub fn search_field<S: 'static>(
    t: &Theme,
    edit: &TextEdit,
    placeholder: &str,
    shortcut: &str,
    key: &str,
    access: impl for<'a> Fn(&'a mut S) -> &'a mut TextEdit + 'static,
    handlers: impl FnOnce(El) -> El,
) -> El {
    let field = handlers(text_input(edit, ty::BODY))
        .key(key)
        .on_edit(access)
        .single_line()
        .placeholder(placeholder.to_owned(), t.ink_faint)
        .color(t.ink)
        .grow(1.0)
        .min_w(0.0);
    let mut w = row().h(28.0).px(8.0).gap(8.0).center_y().radius(size::RADIUS_SM).fill(t.well);
    for s in t.shadow_well() {
        w = w.box_shadow(s);
    }
    w.role("textbox").child(icon(&icons::SEARCH, 14.0).color(t.ink_faint)).child(field).child(kbd(t, shortcut))
}

/// `SegmentedControl`: a well with 2 of padding and gap, radius 8; options 24 tall, 12 a side,
/// radius 6, 12px medium; the selected one raised.
pub fn segmented(t: &Theme, options: Vec<(El, bool)>) -> El {
    let mut w = row().padding(2.0).gap(2.0).radius(size::RADIUS_MD).fill(t.well);
    for s in t.shadow_well() {
        w = w.box_shadow(s);
    }
    for (opt, selected) in options {
        let mut o = opt.h(24.0).px(12.0).center_y().radius(size::RADIUS_SM);
        if selected {
            o = o.fill(t.control);
            for s in t.shadow_raised() {
                o = o.box_shadow(s);
            }
        }
        w = w.child(o);
    }
    w
}

/// A segment's label, in the colour its selection gives it.
pub fn segment_label(t: &Theme, label: &str, selected: bool) -> El {
    row().child(text(label.to_owned()).font(ty::CONTROL).color(if selected { t.ink } else { t.ink_muted }))
}

/// `.label`: an uppercase section label.
pub fn label(t: &Theme, s: &str) -> El {
    text(s.to_uppercase()).font(ty::LABEL).color(t.ink_muted)
}

/// How many files have uncommitted changes: a ± and the count in an 18-tall pill, accent on
/// accent-soft, so it reads as "changes" before the number is read. Says it in full on hover.
pub fn changes(t: &Theme, n: u32) -> El {
    row()
        .h(18.0)
        .pl(5.0)
        .pr(6.0)
        .gap(3.0)
        .radius(size::PILL)
        .center()
        .fill(t.accent_soft)
        .child(icon(&icons::CHANGES, 11.0).color(t.accent))
        .child(text(n.to_string()).font(ty::MONO_11).color(t.accent))
        .help(files_with_changes(n))
}

/// "1 file with uncommitted changes", "12 files with uncommitted changes".
fn files_with_changes(n: u32) -> String {
    if n == 1 { "1 file with uncommitted changes".to_owned() } else { format!("{n} files with uncommitted changes") }
}

/// `.gu-avatar`: a 16 circle in the person's lane colour, initial 600 9px.
pub fn avatar(t: &Theme, name: &str, lane: Color) -> El {
    row().size(16.0, 16.0).radius(8.0).center().fill(lane).child(text(initial(name)).font(ty::AVATAR).color(t.on_lane))
}

/// `.gu-repo-tile`: 22 square, radius 6, lit from above and shaded below.
pub fn tile(t: &Theme, name: &str, lane: Color) -> El {
    row()
        .size(22.0, 22.0)
        .radius(size::RADIUS_SM)
        .center()
        .fill(lane)
        .box_shadow(rune::Shadow {
            x: 0.0,
            y: 1.0,
            blur: 0.0,
            spread: 0.0,
            color: crate::theme::rgba(0xffffff, 0.25),
            inset: true,
        })
        .box_shadow(rune::Shadow {
            x: 0.0,
            y: -1.0,
            blur: 0.0,
            spread: 0.0,
            color: crate::theme::rgba(0x000000, 0.15),
            inset: true,
        })
        .child(text(initial(name)).font(ty::TILE).color(t.on_lane))
}

fn initial(name: &str) -> String {
    name.chars().next().unwrap_or('?').to_uppercase().to_string()
}

/// `.gu-status`: 16 square, radius 4, a 1-point ring and the letter in the status colour.
pub fn status(t: &Theme, letter: char) -> El {
    let c = status_color(t, letter);
    row()
        .size(16.0, 16.0)
        .radius(size::RADIUS_XS)
        .center()
        .box_shadow(Theme::ring(1.0, c))
        .child(text(letter.to_string()).font(ty::STATUS).color(c))
}

/// `.gu-status-A/M/D/R`.
pub fn status_color(t: &Theme, letter: char) -> Color {
    match letter {
        'A' => t.added,
        'D' => t.removed,
        'R' => t.lane(3),
        _ => t.modified(),
    }
}

/// Five 5-point squares (radius 1.5, gap 2), split between added and removed lines in
/// proportion.
pub fn pips(t: &Theme, added: u32, removed: u32) -> El {
    let total = added + removed;
    let (green, red) = if total == 0 {
        (0, 0)
    } else {
        let green = (5.0 * added as f32 / total as f32).round() as u32;
        (green, (5 - green).min((5.0 * removed as f32 / total as f32).round() as u32))
    };
    let mut r = row().gap(2.0);
    for i in 0..5 {
        let c = if i < green {
            t.added
        } else if i < green + red {
            t.removed
        } else {
            t.line
        };
        r = r.child(rune::el().size(5.0, 5.0).radius(1.5).fill(c));
    }
    r
}
