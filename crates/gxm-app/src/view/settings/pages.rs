//! The site the repository publishes with GitHub Pages: whether, from where, and on which domain.

use gxm_core::forge::admin::{Page, PagesSource, Reading, Write};
use rune::{El, row, text};

use super::parts;
use crate::keys::setting_item;
use crate::settings::{Action, What};
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::Variant;

fn forge(w: Write) -> Action {
    Action::Forge(w)
}

pub fn pages(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Reading::Pages(p)) = m.forge(Page::Pages) else { return super::waiting(s, t, What::Forge(Page::Pages)) };
    let can = m.admin();
    let default = m.repo().map_or_else(|| "main".to_owned(), |r| r.default_branch.clone());
    let current = match p {
        None => 0,
        Some(p) if matches!(p.source, PagesSource::Branch { .. }) => 1,
        Some(_) => 2,
    };
    let opts = [
        ("off", "Off", current == 0),
        ("branch", "From a branch", current == 1),
        ("workflow", "With a workflow", current == 2),
    ];
    let mut out = vec![
        parts::note(
            t,
            &match p {
                Some(p) => format!("Published at {}", p.url),
                None => "Not published.".to_owned(),
            },
        ),
        parts::choice(t, "pages", &opts, can, move |s: &mut App, i| {
            let source = match i {
                0 => None,
                1 => Some(PagesSource::Branch { branch: default.clone(), docs: false }),
                _ => Some(PagesSource::Workflow),
            };
            s.settings_do(forge(Write::PagesSource(source)));
        }),
    ];
    let Some(p) = p else { return out };
    if let PagesSource::Branch { branch, docs } = &p.source {
        let mut chips = row().w_full().gap(6.0).center_y().flex_wrap();
        let mut names: Vec<String> = m.remote_branches().to_vec();
        if !names.contains(branch) {
            names.insert(0, branch.clone());
        }
        for b in names {
            let selected = b == *branch;
            let (to, docs_now) = (b.clone(), *docs);
            let mut c = row()
                .key(setting_item("pages-branch", &b))
                .role("button")
                .label(b.clone())
                .h(24.0)
                .px(10.0)
                .center_y()
                .radius(size::PILL)
                .fill(if selected { t.accent_soft } else { t.control_hover })
                .child(text(b.clone()).font(ty::CONTROL).color(if selected { t.accent } else { t.ink }));
            if can && !selected {
                c = c.on_press(move |s: &mut App| {
                    s.settings_do(forge(Write::PagesSource(Some(PagesSource::Branch {
                        branch: to.clone(),
                        docs: docs_now,
                    }))));
                });
            }
            chips = chips.child(c);
        }
        let b = branch.clone();
        let folders = [("root", "/ (its root)", !docs), ("docs", "/docs", *docs)];
        out.push(parts::group(
            t,
            "From",
            vec![
                chips,
                parts::choice(t, "pages", &folders, can, move |s: &mut App, i| {
                    s.settings_do(forge(Write::PagesSource(Some(PagesSource::Branch {
                        branch: b.clone(),
                        docs: i == 1,
                    }))));
                }),
            ],
        ));
    }
    let cname = p.cname.clone();
    out.push(parts::group(
        t,
        "Custom domain",
        vec![
            parts::controls(vec![
                row().grow(1.0).child(parts::field(
                    t,
                    "cname",
                    &m.drafts.cname,
                    "docs.example.com",
                    can,
                    false,
                    |s: &mut App| &mut s.settings.drafts.cname,
                    parts::on(save_domain),
                )),
                parts::button(t, "save-domain", "Save", Variant::Secondary, can, save_domain),
            ]),
            parts::toggle(
                t,
                "https",
                p.https,
                "Enforce HTTPS",
                Some("Visitors who come by http:// are sent to https://."),
                None,
                can,
                move |v| forge(Write::PagesDomain { cname: cname.clone(), https: v }),
            ),
        ],
    ));
    out
}

fn save_domain(s: &mut App) {
    let cname = s.settings.drafts.cname.text.trim().to_owned();
    let https = matches!(s.settings.forge(Page::Pages), Some(Reading::Pages(Some(p))) if p.https);
    s.settings_do(forge(Write::PagesDomain { cname: (!cname.is_empty()).then_some(cname), https }));
}
