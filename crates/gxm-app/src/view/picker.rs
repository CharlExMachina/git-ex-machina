//! The popup `@` and `#` open in the release notes: people to mention (who's signed in and who
//! wrote the repository's pull requests) and pull requests to refer to, as the main window knew
//! them when it opened the tag, narrowed by what's typed after the `@` or `#`. ↑ and ↓ choose, Return or Tab puts the chip in, Escape closes it.

use gxm_core::notes::Chip;
use rune::{El, Key, KeyEvent, Tx, col, row, text};

use crate::icons;
use crate::keys;
use crate::notes::PickKind;
use crate::tag_window::TagWindow;
use crate::theme::{Theme, size, ty};

/// How many rows the popup shows.
const ROWS: usize = 6;

/// A row of the popup: the chip it puts in, and what it says.
pub struct Candidate {
    pub chip: Chip,
    pub label: String,
    pub detail: String,
}

/// What the open popup lists, as narrowed by what's typed.
pub fn candidates(s: &TagWindow) -> Vec<Candidate> {
    let (Some(pick), Some(tag)) = (&s.tag_fields.notes.pick, &s.sheet) else { return Vec::new() };
    let q = pick.query.to_lowercase();
    match pick.kind {
        PickKind::People => tag
            .opening
            .people
            .iter()
            .filter(|(login, name)| login.to_lowercase().contains(&q) || name.to_lowercase().contains(&q))
            .take(ROWS)
            .map(|(login, name)| Candidate {
                label: format!("@{login}"),
                chip: Chip::Mention(login.clone()),
                detail: name.clone(),
            })
            .collect(),
        PickKind::Pulls => tag
            .opening
            .pulls
            .iter()
            .filter(|(number, title)| number.to_string().starts_with(&q) || title.to_lowercase().contains(&q))
            .take(ROWS)
            .map(|(number, title)| Candidate {
                chip: Chip::Pull(*number),
                label: format!("#{number}"),
                detail: title.clone(),
            })
            .collect(),
    }
}

/// A key in part `id` while the popup is open there: true when the popup took it.
pub fn key(s: &mut TagWindow, id: u64, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if s.tag_fields.notes.pick.as_ref().is_none_or(|p| p.part != id) {
        return false;
    }
    let found = candidates(s);
    match ev.key {
        Key::Up | Key::Down if !found.is_empty() => {
            s.tag_fields.notes.move_pick(if ev.key == Key::Up { -1 } else { 1 }, found.len());
            true
        }
        Key::Enter | Key::Tab if !found.is_empty() => {
            let active = s.tag_fields.notes.pick.as_ref().map_or(0, |p| p.active.min(found.len() - 1));
            s.tag_fields.notes.choose(&found[active].chip);
            super::notes::refocus(s, tx);
            true
        }
        Key::Escape => {
            s.tag_fields.notes.pick = None;
            true
        }
        _ => false,
    }
}

/// The popup, under the caret (`caret`, in window points); nothing while it's closed or nothing
/// matches.
pub fn layer(s: &TagWindow, t: &Theme, caret: Option<[f32; 4]>) -> Option<El> {
    let caret = caret?;
    let pick = s.tag_fields.notes.pick.as_ref()?;
    let found = candidates(s);
    if found.is_empty() {
        return None;
    }
    let glyph = if pick.kind == PickKind::People { &icons::PERSON } else { &icons::PULL_REQUEST };
    let mut list = col()
        .key(keys::NOTES_PICKER)
        .role("listbox")
        .absolute(caret[0], caret[1] + caret[3] + 4.0)
        .w(300.0)
        .padding(4.0)
        .gap(1.0)
        .radius(size::RADIUS_MD)
        .fill(t.raised)
        .preserve_focus_on_press(true);
    for sh in t.shadow_overlay() {
        list = list.box_shadow(sh);
    }
    for (i, c) in found.into_iter().enumerate() {
        let active = i == pick.active;
        let chip = c.chip.clone();
        let mut line = row()
            .key(keys::notes_pick(i))
            .role("option")
            .w_full()
            .h(28.0)
            .px(8.0)
            .gap(8.0)
            .center_y()
            .radius(size::RADIUS_SM)
            .child(rune::icon(glyph, 14.0).color(if active { t.accent } else { t.ink_faint }))
            .child(text(c.label).font(ty::BODY_STRONG).color(t.ink))
            .child(text(c.detail).font(ty::CAPTION).color(t.ink_muted).truncate().grow(1.0).min_w(0.0))
            .on_press_tx(move |s: &mut TagWindow, tx| {
                s.tag_fields.notes.choose(&chip);
                super::notes::refocus(s, tx);
            });
        if active {
            line = line.fill(t.accent_soft);
        }
        list = list.child(line);
    }
    Some(list)
}
