//! A pull request on the graph (`design/pr-flow/flow2`): the line under the toolbar naming it,
//! and its row above its newest commit, with its checks and approvals.

use gxm_core::forge::model::Verdict;
use gxm_core::graph::{GraphRow, Lit};
use rune::{El, icon, row, text};

use super::{graph, pull_parts};
use crate::icons;
use crate::keys;
use crate::pulls::Sending;
use crate::review::PullView;
use crate::state::App;
use crate::theme::{Theme, mix, ty};
use crate::ui;

/// The pull request's colour: its lane's, when its commits are in the history.
fn lane_color(s: &App, t: &Theme) -> rune::Color {
    let lane = s.pull_tip().and_then(|i| s.history.as_ref()?.graph.get(i)).map(|g| g.color);
    lane.map_or(t.lane(4), |c| graph::color(t, c))
}

/// `⑂ Pull request #131 · fix/avatar-cache → main`, and what became of a review just sent.
pub fn summary(s: &App, v: &PullView, t: &Theme) -> El {
    let p = s.pulls.index.as_ref().and_then(|i| i.pulls.iter().find(|p| p.number == v.number));
    let mut what = p.map_or_else(String::new, |p| format!("· {} → {}", p.head_ref, p.base_ref));
    let mut line = row()
        .key(keys::PULL_SUMMARY)
        .w_full()
        .pt(10.0)
        .pb(10.0)
        .pl(24.0)
        .pr(16.0)
        .gap(6.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout()
        .child(icon(&icons::PULL_REQUEST, 12.0).color(lane_color(s, t)))
        .child(text(format!("Pull request #{}", v.number)).font(ty::CAPTION_STRONG).color(t.ink));
    let provider = match &s.pulls.link {
        crate::pulls::Link::Connected { forge, .. } => forge.provider().name(),
        _ => "the provider",
    };
    let note = match &s.pulls.machine.sending {
        Sending::Sent { number, verdict } if *number == v.number => {
            let what = match verdict {
                Verdict::Approve => "Your approval",
                Verdict::RequestChanges => "Your request for changes",
                Verdict::Comment => "Your review",
            };
            Some(ui::signal(&icons::CHECK, t.added, format!("{what} was sent to {provider}")))
        }
        Sending::Failed { number, message } if *number == v.number => Some(
            text(format!("Your review wasn't sent: {message}"))
                .font(ty::CAPTION)
                .color(t.removed)
                .truncate()
                .help(message.clone()),
        ),
        _ => match &s.pulls.machine.merging {
            crate::pulls::Merging::Merged { number } if *number == v.number => {
                let base = p.map_or("its base", |p| p.base_ref.as_str());
                Some(ui::signal(&icons::CHECK, t.added, format!("Squashed and merged into {base}")))
            }
            _ => None,
        },
    };
    if note.is_none() {
        what.push_str(" · its commits are lit, the rest of the history dims");
    }
    line = line.child(text(what).font(ty::CAPTION).color(t.ink_muted).truncate());
    match note {
        Some(n) => line.child(rune::el().grow(1.0)).child(n),
        None => line,
    }
}

/// The `#131` row: 30 tall with 2 above and below, radius 6, lifted like a selected row (its
/// top highlight, edge and drop) and tinted 10% with the pull request's colour. Like a selected
/// row, it sits beside the graph, and the lanes run past it uncovered.
pub fn banner(s: &App, g: &GraphRow, cols: usize, light: Option<&Lit>, t: &Theme) -> El {
    let ink = lane_color(s, t);
    let Some(v) = s.pull_view() else { return rune::el() };
    let summary = s.pulls.index.as_ref().and_then(|i| i.pulls.iter().find(|p| p.number == v.number));
    let full = s.open_pull_full();
    let commits = full.map(|p| p.commits as usize).or_else(|| s.pull_changes().map(|c| c.commits.len()));
    let mut about: Vec<String> = Vec::new();
    if let Some(n) = commits {
        about.push(pull_parts::plural(n, "commit"));
    }
    if let Some(p) = summary {
        about.push(p.author.name.clone());
    }
    let me = s.pulls.viewer().map(|u| u.login.clone()).unwrap_or_default();
    if let Some(part) = pull_parts::your_part(full.and_then(|p| p.review_of(&me))) {
        about.push(part.to_owned());
    }
    let mut content = row()
        .grow(1.0)
        .min_w(0.0)
        .h(30.0)
        .pr(12.0)
        .gap(8.0)
        .center_y()
        .child(icon(&icons::PULL_REQUEST, 14.0).color(ink))
        .child(text(format!("#{}", v.number)).font(ty::BODY_SEMI).color(t.ink))
        .child(text(about.join(" · ")).font(ty::BODY).color(t.ink_muted).truncate())
        .child(rune::el().grow(1.0));
    if let Some(p) = full {
        let failing = p.failing().count();
        if failing > 0 {
            content = content.child(ui::signal(&icons::X, t.removed, format!("{failing} failing")));
        }
        let approvals = p.approvals();
        if approvals > 0 {
            let approved = ui::signal(&icons::CHECK, t.added, pull_parts::plural(approvals, "approval"));
            content = content.child(approved.ml(10.0));
        }
    }
    // The text starts where the rows' subjects do.
    let mut card = row()
        .key(keys::PULL_BANNER)
        .grow(1.0)
        .min_w(0.0)
        .h(30.0)
        .my(2.0)
        .pl(6.0)
        .radius(crate::theme::size::RADIUS_SM)
        .fill(mix(ink, 0.10, t.raised));
    for sh in t.shadow_lifted() {
        card = card.box_shadow(sh);
    }
    row().w_full().h(34.0).child(graph::passing(t, g, cols, light, 34.0)).child(card.child(content))
}
