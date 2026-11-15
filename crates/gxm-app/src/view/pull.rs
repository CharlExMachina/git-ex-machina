//! The inspector as the open pull request: Overview (its header,
//! description, merge box and reviewers), Checks, and Conversation.

use gxm_core::forge::model::{CheckState, Event, Pull, ReviewStatus, Thread};
use gxm_core::time;
use rune::{El, ScrollSpec, col, icon, rich, row, text};

use super::history::ref_badge;
use super::{bar, pull_parts};
use crate::icons;
use crate::keys;
use crate::pulls::Fetch;
use crate::review::{PullTab, PullView};
use crate::state::App;
use crate::theme::{Theme, ty};
use crate::ui::{self, Variant};

pub fn inspector(s: &App, v: &PullView, t: &Theme) -> El {
    let p = s.open_pull_full();
    let tab = |label: &str, key: &str, which: PullTab, count: Option<usize>| {
        let on = v.tab == which;
        let mut l = ui::segment_label(t, label, on).gap(6.0);
        if let Some(n) = count {
            l = l.child(ui::tab_count(t, n));
        }
        (l.key(key).on_press(move |s: &mut App| s.set_pull_tab(which)), on)
    };
    let url = p.map(|p| p.summary.url.clone());
    let mut open = ui::button(t, Variant::Ghost, Some(&icons::EXTERNAL), "", None).key(keys::OPEN_ON_PROVIDER);
    open = match url {
        Some(url) => open.help("Open on GitHub").on_press_tx(move |_: &mut App, tx| {
            // A browser that won't open leaves the person where they are.
            tx.open_url(&url).ok();
        }),
        None => open.disabled(true).opacity(0.6),
    };
    let top = bar(t)
        .pl(16.0)
        .pr(12.0)
        .child(ui::segmented(
            t,
            vec![
                tab("Overview", keys::TAB_OVERVIEW, PullTab::Overview, None),
                tab("Checks", keys::TAB_CHECKS, PullTab::Checks, None),
                tab(
                    "Conversation",
                    keys::TAB_CONVERSATION,
                    PullTab::Conversation,
                    p.map(|p| p.conversation.len()).filter(|n| *n > 0),
                ),
            ],
        ))
        .child(rune::el().grow(1.0))
        .child(open);
    let body = match p {
        Some(p) => match v.tab {
            PullTab::Overview => overview(s, p, t),
            PullTab::Checks => checks(p, t),
            PullTab::Conversation => conversation(s, p, t),
        },
        None => {
            let note = match &s.pulls.machine.details {
                Fetch::Failed(e) => text(format!("Couldn't read the pull request: {e}")).color(t.removed),
                _ => text("Reading the pull request…").color(t.ink_faint),
            };
            col().padding(20.0).child(note.font(ty::BODY).wrap())
        }
    };
    col()
        .key(keys::PULL_INSPECTOR)
        .fill(t.raised)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.line)
        .border_layout()
        .child(top)
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}

/// The state, number, title, and who wants to merge what into what, 8 apart (16 around the
/// title, as `pr_header` sets them).
pub fn header(s: &App, p: &Pull, t: &Theme) -> El {
    let sum = &p.summary;
    let lane = s.pull_tip().and_then(|i| s.history.as_ref()?.graph.get(i)).map(|g| super::graph::color(t, g.color));
    let meta = row()
        .w_full()
        .flex_wrap()
        .gap(6.0)
        .center_y()
        .child(pull_parts::person(s, t, &sum.author, ty::CAPTION_STRONG))
        .child(
            text(format!(
                "{} {} into",
                if sum.state == gxm_core::forge::model::PullState::Merged { "merged" } else { "wants to merge" },
                pull_parts::plural(p.commits as usize, "commit")
            ))
            .font(ty::CAPTION)
            .color(t.ink_muted),
        )
        .child(ref_badge(t, gxm_core::history::RefKind::Head, &sum.base_ref, t.accent))
        .child(text("from").font(ty::CAPTION).color(t.ink_muted))
        .child(ref_badge(t, gxm_core::history::RefKind::LocalBranch, &sum.head_ref, lane.unwrap_or(t.lane(4))))
        .child(text(format!("· opened {}", time::ago(p.created, s.env.now()))).font(ty::CAPTION).color(t.ink_muted));
    col()
        .w_full()
        .gap(8.0)
        .child(ui::label(t, "Pull request"))
        .child(
            row()
                .gap(10.0)
                .center_y()
                .child(ui::state_pill(t, sum.state, t.raised))
                .child(text(format!("#{}", sum.number)).font(ty::MONO_SM).color(t.ink_faint)),
        )
        .child(text(sum.title.clone()).font(ty::DISPLAY_SM).color(t.ink).wrap().mt(8.0))
        .child(meta.mt(8.0))
}

fn overview(s: &App, p: &Pull, t: &Theme) -> El {
    col()
        .w_full()
        .padding(20.0)
        .gap(18.0)
        .child(header(s, p, t))
        .child(pull_parts::markdown(t, &p.body))
        .child(pull_parts::merge_box(s, p, t))
        .child(pull_parts::reviewers(s, p, t))
}

/// Every check on the head commit: 26-tall rows, the state, the name, how long it ran.
fn checks(p: &Pull, t: &Theme) -> El {
    let passed = p.checks.iter().filter(|c| c.state == CheckState::Passed).count();
    let mut list = col().w_full().gap(2.0).child(
        row()
            .w_full()
            .px(8.0)
            .pb(6.0)
            .justify_between()
            .child(ui::label(t, "Checks"))
            .child(text(format!("{passed} of {} passed", p.checks.len())).font(ty::MONO_SM).color(t.ink_muted)),
    );
    for c in &p.checks {
        let (glyph, color) = match c.state {
            CheckState::Passed => (&icons::CHECK, t.added),
            CheckState::Failed => (&icons::X, t.removed),
            CheckState::Pending => (&icons::CLOCK, t.accent),
            CheckState::Skipped => (&icons::CHEVRON, t.ink_faint),
        };
        let took = c.seconds.map(pull_parts::duration).unwrap_or_default();
        let mut name = text(c.name.clone()).font(ty::BODY).color(t.ink).truncate();
        if !c.title.is_empty() {
            name = name.help(c.title.clone());
        }
        list = list.child(
            row()
                .w_full()
                .h(26.0)
                .px(8.0)
                .gap(8.0)
                .center_y()
                .child(icon(glyph, 14.0).color(color))
                .child(name.grow(1.0).min_w(0.0))
                .child(text(took).font(ty::MONO_SM).color(t.ink_faint)),
        );
    }
    col().w_full().padding(20.0).child(list)
}

/// Reviews, comments and threads, oldest first, as a timeline.
fn conversation(s: &App, p: &Pull, t: &Theme) -> El {
    enum Item<'a> {
        Event(&'a Event),
        Thread(&'a Thread),
    }
    let mut items: Vec<(_, Item<'_>)> = p.conversation.iter().map(|e| (e.at(), Item::Event(e))).collect();
    items.extend(p.threads.iter().filter_map(|th| Some((th.comments.first()?.at, Item::Thread(th)))));
    items.sort_by_key(|(at, _)| *at);
    let now = s.env.now();
    let mut c = col().w_full().padding(20.0).gap(14.0);
    for (_, item) in items {
        c = c.child(match item {
            Item::Event(Event::Review { author, status, body, at }) => {
                let did = match status {
                    ReviewStatus::Approved => "approved",
                    ReviewStatus::ChangesRequested => "requested changes",
                    _ => "reviewed",
                };
                entry(s, t, author, format!("{did} · {}", time::relative(*at, now, &s.env.tz)), body)
            }
            Item::Event(Event::Comment(cm)) => {
                entry(s, t, &cm.author, format!("commented · {}", time::relative(cm.at, now, &s.env.tz)), &cm.body)
            }
            Item::Thread(th) => {
                let place = th.line.map_or_else(|| th.path.clone(), |l| format!("{}:{l}", th.path));
                let mut card = ui::card(t)
                    .w_full()
                    .padding(12.0)
                    .gap(10.0)
                    .child(text(place).font(ty::MONO_11).color(t.ink_muted).truncate());
                for cm in &th.comments {
                    card = card.child(entry(s, t, &cm.author, time::relative(cm.at, now, &s.env.tz), &cm.body));
                }
                card
            }
        });
    }
    c
}

/// Someone did something, and what they wrote.
fn entry(s: &App, t: &Theme, who: &gxm_core::forge::model::User, did: String, body: &str) -> El {
    let mut c = col().w_full().gap(2.0).child(
        row()
            .gap(6.0)
            .center_y()
            .child(pull_parts::person(s, t, who, ty::CAPTION_STRONG))
            .child(text(did).font(ty::CAPTION).color(t.ink_muted)),
    );
    if !body.trim().is_empty() {
        let (body, spans) = pull_parts::inline(t, body.trim(), false);
        c = c.child(rich(body, spans).font(ty::BODY).color(t.ink).pl(22.0).w_full());
    }
    c
}
