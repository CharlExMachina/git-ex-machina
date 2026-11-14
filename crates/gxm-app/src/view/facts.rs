//! A commit's facts in the inspector, as labelled rows (`.kv` in `design/pr-flow`): a 96-point
//! label column, 12 apart, rows 10 apart, 12px. Author, Date (absolute, then how long ago),
//! Commit, Parent or Parents, and the pull request it belongs to.

use gxm_core::history::Commit;
use gxm_core::time;
use rune::text::Span;
use rune::{El, col, rich, row, text};

use super::history::author_color;
use crate::keys;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui;

pub fn facts(s: &App, c: &Commit, t: &Theme) -> El {
    let fact = |label: &str, value: El| {
        row()
            .w_full()
            .gap(12.0)
            .center_y()
            .child(row().w(96.0).child(text(label.to_owned()).font(ty::CAPTION).color(t.ink_faint)))
            .child(value.grow(1.0).min_w(0.0))
    };
    let author = row()
        .gap(8.0)
        .center_y()
        .child(ui::avatar(t, &c.author, author_color(t, &c.email)))
        .child(text(c.author.clone()).font(ty::CAPTION_STRONG).color(t.ink));
    let (stamp, ago) = (time::stamp(c.time, &s.env.tz), time::ago(c.time, s.env.now()));
    let date = format!("{stamp} · {ago}");
    let date = rich(date.clone(), vec![Span::new(stamp.len() + 1, date.len()).color(t.ink_muted)])
        .font(ty::CAPTION)
        .color(t.ink)
        .help(time::absolute(c.time, &s.env.tz));
    let chip = row().child(
        row()
            .px(6.0)
            .py(1.0)
            .radius(size::RADIUS_XS)
            .fill(t.canvas)
            .box_shadow(Theme::ring(1.0, t.line))
            .child(text(c.id.get(..8).unwrap_or(&c.id).to_owned()).font(ty::MONO_SM).color(t.ink)),
    );
    let mut rows = col().w_full().gap(10.0).child(fact("Author", author)).child(fact("Date", date)).child(fact("Commit", chip));
    if !c.parents.is_empty() {
        let parents: Vec<&str> = c.parents.iter().map(|p| p.get(..7).unwrap_or(p)).collect();
        let label = if parents.len() == 1 { "Parent" } else { "Parents" };
        rows = rows.child(fact(label, text(parents.join(" · ")).font(ty::MONO_SM).color(t.ink_muted)));
    }
    if let Some(p) = s.pulls.of_commit(&c.id) {
        let number = format!("#{}", p.number);
        let line = format!("{number} {}", p.title);
        let title = rich(line, vec![Span::new(0, number.len()).mono().size(11.0).weight(500.0).color(t.ink_faint)])
            .font(ty::CAPTION)
            .color(t.ink)
            .truncate();
        let value = row().gap(8.0).center_y().child(ui::state_pill(t, p.state, t.raised)).child(title.grow(1.0).min_w(0.0));
        rows = rows.child(fact("Pull request", value).key(keys::PULL_REQUEST_ROW));
    } else if let Some(problem) = s.pulls.problem().filter(|_| s.pulls.index.is_none()) {
        rows = rows.child(
            fact("Pull request", text(problem.clone()).font(ty::CAPTION).color(t.ink_faint).truncate().help(problem))
                .key(keys::PULL_NOTICE),
        );
    }
    rows
}
