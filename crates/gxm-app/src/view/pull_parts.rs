//! Pieces of a pull request the graph, the inspector and the review cockpit share: people,
//! the description, reviewers, the merge box (`design/pr-flow/pages.py`).

use gxm_core::forge::model::{CheckState, Mergeable, Pull, PullState, ReviewStatus, User};
use rune::text::Span;
use rune::{Color, El, col, icon, rich, row, text};

use super::history::author_color;
use crate::icons;
use crate::keys;
use crate::state::{App, Row};
use crate::theme::{Theme, ty};
use crate::ui;

/// A person's colour: their commits' colour when they've committed here, so a name keeps its
/// colour between the history and the pull request; else one chosen by their login.
pub fn person_color(s: &App, t: &Theme, who: &User) -> Color {
    let email = s.history.as_ref().and_then(|h| {
        h.rows.iter().find_map(|r| match r {
            Row::Commit(c) if c.author == who.name => Some(c.email.as_str()),
            _ => None,
        })
    });
    author_color(t, email.unwrap_or(&who.login))
}

/// `person()`: a 16 avatar and the name, 6 apart.
pub fn person(s: &App, t: &Theme, who: &User, font: rune::TextStyle) -> El {
    row()
        .gap(6.0)
        .center_y()
        .child(ui::avatar(t, &who.name, person_color(s, t, who)))
        .child(text(who.name.clone()).font(font).color(t.ink))
}

/// Inline markdown: `code` set in mono on a well, `#123` in the accent. Returns the text with
/// the backticks gone, and its spans.
pub fn inline(t: &Theme, source: &str, mono: bool) -> (String, Vec<Span>) {
    let (mut out, mut spans) = (String::new(), Vec::new());
    for (k, part) in source.split('`').enumerate() {
        let start = out.len();
        out.push_str(part);
        if k % 2 == 1 {
            let code = Span::new(start, out.len()).mono().size(11.5).color(t.ink);
            spans.push(if mono { code } else { Span { bg: Some(t.well), bg_pad: 2.0, ..code } });
        } else {
            let mut rest = part;
            while let Some(at) = rest.find('#') {
                let digits = rest[at + 1..].chars().take_while(char::is_ascii_digit).count();
                let from = start + (part.len() - rest.len()) + at;
                if digits > 0 {
                    spans.push(Span::new(from, from + 1 + digits).color(t.accent));
                }
                rest = &rest[at + 1..];
            }
        }
    }
    (out, spans)
}

/// A description as GitHub writes it, in the `.md` style: paragraphs and `-` lists, 8 apart,
/// 12.5px muted.
pub fn markdown(t: &Theme, body: &str) -> El {
    let mut c = col().w_full().gap(8.0);
    for block in body.split("\n\n").map(str::trim).filter(|b| !b.is_empty()) {
        let lines: Vec<&str> = block.lines().map(str::trim).collect();
        if lines.iter().all(|l| l.starts_with("- ") || l.starts_with("* ")) {
            let mut list = col().w_full();
            for l in lines {
                let (s, spans) = inline(t, &l[2..], false);
                list = list.child(
                    row()
                        .w_full()
                        .items_start()
                        .child(row().w(18.0).pl(6.0).child(text("•").font(ty::DESCRIPTION).color(t.ink_muted)))
                        .child(rich(s, spans).font(ty::DESCRIPTION).color(t.ink_muted).grow(1.0).min_w(0.0)),
                );
            }
            c = c.child(list);
        } else {
            let (s, spans) = inline(t, &lines.join(" "), false);
            c = c.child(rich(s, spans).font(ty::DESCRIPTION).color(t.ink_muted).w_full());
        }
    }
    c
}

/// How the viewer stands with the pull request, for its row on the graph.
pub fn your_part(status: Option<ReviewStatus>) -> Option<&'static str> {
    Some(match status? {
        ReviewStatus::Requested => "review requested from you",
        ReviewStatus::Approved => "you approved",
        ReviewStatus::ChangesRequested => "you requested changes",
        ReviewStatus::Commented => "you commented",
    })
}

pub fn plural(n: usize, one: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") }
}

/// A check's run time: `38s`, `2m 03s`.
pub fn duration(seconds: u32) -> String {
    if seconds < 60 { format!("{seconds}s") } else { format!("{}m {:02}s", seconds / 60, seconds % 60) }
}

/// A reviewer's state, as a signal.
pub fn review_signal(t: &Theme, status: ReviewStatus) -> El {
    match status {
        ReviewStatus::Approved => ui::signal(&icons::CHECK, t.added, "Approved"),
        ReviewStatus::Requested => ui::signal(&icons::CLOCK, t.accent, "Requested"),
        ReviewStatus::ChangesRequested => ui::signal(&icons::X, t.removed, "Changes requested"),
        ReviewStatus::Commented => ui::signal(&icons::COMMENT, t.ink_muted, "Commented"),
    }
}

/// `Reviewers`: the label, then a 28-tall row per reviewer: who, `(you)`, their state.
pub fn reviewers(s: &App, p: &Pull, t: &Theme) -> El {
    let me = s.pulls.viewer().map(|v| v.login.as_str());
    let mut c = col().w_full().gap(2.0).child(row().px(8.0).pb(6.0).child(ui::label(t, "Reviewers")));
    for r in &p.reviewers {
        let mut line = row().key(keys::reviewer(&r.user.login)).w_full().h(28.0).px(8.0).gap(8.0).center_y();
        line = line.child(person(s, t, &r.user, ty::BODY));
        if me == Some(r.user.login.as_str()) {
            line = line.child(text("(you)").font(ty::CAPTION).color(t.ink_faint));
        }
        c = c.child(line.child(rune::el().grow(1.0)).child(review_signal(t, r.status)));
    }
    c
}

/// The merge box: the checks, conflicts and approvals, then Squash and merge, which waits.
pub fn merge_box(s: &App, p: &Pull, t: &Theme) -> El {
    let fact = |glyph, color, title: String, detail: String| {
        row()
            .w_full()
            .gap(10.0)
            .items_start()
            .child(row().mt(1.0).child(icon(glyph, 16.0).color(color)))
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .child(text(title).font(ty::BODY_SEMI).color(t.ink))
                    .child(text(detail).font(ty::CAPTION).color(t.ink_muted).wrap()),
            )
    };
    let base = p.summary.base_ref.clone();
    let mut card = ui::card(t).key(keys::MERGE_BOX).w_full().pt(14.0).px(14.0).pb(12.0).gap(10.0);
    if p.summary.state == PullState::Merged {
        let title = format!("Merged into {base}");
        return card.child(fact(&icons::MERGE, t.lane(6), title, plural(p.commits as usize, "commit")));
    }
    let failing: Vec<_> = p.failing().collect();
    let running = p.checks.iter().filter(|c| c.state == CheckState::Pending).count();
    if let Some(first) = failing.first() {
        let detail = format!("{} {}", first.name, first.title).trim().to_owned();
        let more = if failing.len() > 1 { format!(" and {} more", failing.len() - 1) } else { String::new() };
        card = card.child(fact(&icons::X, t.removed, format!("{} failing", plural(failing.len(), "check")), detail + &more));
    } else if running > 0 {
        card = card.child(fact(&icons::CLOCK, t.accent, format!("{} running", plural(running, "check")), String::new()));
    } else if !p.checks.is_empty() {
        card = card.child(fact(&icons::CHECK, t.added, "All checks passed".to_owned(), plural(p.checks.len(), "check")));
    }
    let approvals = p.approvals();
    let me = s.pulls.viewer().map(|v| v.login.as_str()).unwrap_or_default();
    let ready = match (p.review_of(me), failing.is_empty() && running == 0) {
        (Some(ReviewStatus::Requested), _) => "your review is requested",
        (_, false) => "ready once checks pass",
        (_, true) => "ready to merge",
    };
    let approved = if approvals == 0 { "No approvals yet".to_owned() } else { plural(approvals, "approval") };
    let detail = format!("{approved} · {ready}");
    card = card.child(match p.mergeable {
        Mergeable::Clean => fact(&icons::CHECK, t.added, format!("No conflicts with {base}"), detail),
        Mergeable::Conflicts => fact(&icons::X, t.removed, format!("Conflicts with {base}"), detail),
        Mergeable::Unknown => fact(&icons::CLOCK, t.accent, "Checking for conflicts".to_owned(), detail),
    });
    let why = if failing.is_empty() { "Coming next: merge from here" } else { "Waits until every check passes" };
    card.child(
        row()
            .mt(2.0)
            .gap(8.0)
            .child(ui::button_disabled(t, Some(&icons::MERGE), "Squash and merge").help(why))
            .child(ui::button_disabled(t, Some(&icons::CHEVRON), "").help("Coming next: merge or rebase instead")),
    )
}
