//! The element tree at a mark, as JSON beside its screenshot: the screen the journey checked.
//!
//! It mirrors the element fields of Rune's `--tree` (same names, rects in points), so the
//! drive-app skill reads either one. Rune's own tree also carries the accessibility tree and
//! key warnings; for those at a mark, replay it with `--tree` (see the drive-app skill).

use rune::{ElementInfo, Runtime};
use serde_json::{Value, json};

pub(crate) fn tree<S>(rt: &Runtime<S>, t: f64, dark: bool) -> Value {
    let elements: Vec<Value> = rt.elements().iter().map(element).collect();
    json!({
        "t": t,
        "viewport": [gxm_app::SIZE.0, gxm_app::SIZE.1],
        "dark_system": dark,
        "elements": elements,
    })
}

fn element(e: &ElementInfo) -> Value {
    let mut v = json!({
        "path": e.path,
        "role": e.role,
        "rect": e.rect.map(r2),
        "visible": e.visible.map(r2),
        "opacity": (e.opacity * 1000.0).round() / 1000.0,
        "focusable": e.focusable,
        "focused": e.focused,
        "fill": e.fill.hex(),
    });
    if let Some(k) = &e.key {
        v["key"] = k.clone().into();
    }
    if let Some(l) = &e.label {
        v["label"] = l.clone().into();
    }
    if let Some(t) = &e.text {
        v["text"] = t.clone().into();
    }
    if let Some(c) = e.text_color {
        v["color"] = c.hex().into();
    }
    if e.rect.iter().zip(e.target.iter()).any(|(a, b)| (a - b).abs() > 0.01) {
        v["target"] = e.target.map(r2).to_vec().into();
    }
    if e.moving {
        v["moving"] = true.into();
    }
    if e.exiting {
        v["exiting"] = true.into();
    }
    v
}

fn r2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}
