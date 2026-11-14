//! The inspector: what the history has selected (HEAD when nothing is), its message, its
//! changed files (`FileChangeRow`) and the selected file's first hunk (`DiffHunk`).

use gxm_core::diff::{Hunk, LineKind};
use gxm_core::history::{ChangeStatus, Commit, CommitInfo, FileChange};
use gxm_core::time;
use rune::{El, ScrollSpec, col, row, text};

use super::bar;
use super::history::author_color;
use crate::icons;
use crate::keys;
use crate::state::{App, Details, InspectorTab, Subject};
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

pub fn inspector(s: &App, t: &Theme) -> El {
    let details = s.history.as_ref().and_then(|h| h.details.as_ref());
    let copy_id = match details.map(|d| &d.subject) {
        Some(Subject::Head(id) | Subject::Commit(id)) => Some(id.clone()),
        _ => None,
    };
    // On the uncommitted row the first tab is where the commit message is written.
    let first_tab = if matches!(details.map(|d| &d.subject), Some(Subject::Worktree)) { "Message" } else { "Overview" };
    let tab = |label: &str, key: &str, which: InspectorTab| {
        let on = s.tab == which;
        (ui::segment_label(t, label, on).key(key).on_press(move |s: &mut App| s.tab = which), on)
    };
    let mut copy = ui::button(t, Variant::Ghost, Some(&icons::COPY), "", None).key(keys::COPY_ID);
    copy = match copy_id {
        Some(id) => copy.help("Copy commit ID").on_press_tx(move |_: &mut App, tx| tx.copy(id.clone())),
        None => copy.disabled(true).opacity(0.6),
    };
    let top = bar(t)
        .pl(16.0)
        .pr(12.0)
        .child(ui::segmented(
            t,
            vec![
                tab(first_tab, keys::TAB_OVERVIEW, InspectorTab::Overview),
                tab("Files", keys::TAB_FILES, InspectorTab::Files),
            ],
        ))
        .child(rune::el().grow(1.0))
        .child(copy);
    let body = match details {
        Some(d) => content(s, d, t),
        None => col().padding(20.0).child(text("Nothing to inspect").font(ty::BODY).color(t.ink_faint)),
    };
    col()
        .key(keys::INSPECTOR)
        .fill(t.raised)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.line)
        .border_layout()
        .child(top)
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}

fn content(s: &App, d: &Details, t: &Theme) -> El {
    // A commit's Overview tab is for reading it: its whole message, then who, when and where.
    // Its Files tab is the changes, under a short header.
    let reading = s.tab == InspectorTab::Overview && matches!(d.subject, Subject::Head(_) | Subject::Commit(_));
    let mut c = col().w_full().padding(20.0).gap(18.0).child(heading(s, d, t, !reading));
    if let Some(e) = &d.error {
        c = c.child(text(e.clone()).font(ty::CAPTION).color(t.removed).wrap());
    }
    if let (true, Subject::Head(id) | Subject::Commit(id), Some(info)) = (reading, &d.subject, &d.info) {
        if !info.body.is_empty() {
            c = c.child(text(info.body.clone()).font(ty::BODY.lh(20.0)).color(t.ink).wrap());
        }
        if let Some(commit) = s.commit(id) {
            c = c.child(message_details(s, commit, info, t));
        }
    }
    if let Subject::Commits(ids) = &d.subject {
        c = c.child(commit_list(s, ids, t));
    }
    if s.tab == InspectorTab::Overview && d.subject == Subject::Worktree {
        c = c.child(message_editor(s, t));
    }
    if s.tab == InspectorTab::Files {
        c = c.child(files(s, d, t));
        if let Some(h) = &d.hunk {
            c = c.child(hunk(h, t));
        }
    }
    c
}

/// The kicker, the title, then who and when (when `byline`; the Overview tab lists those in
/// its details instead), or a note.
fn heading(s: &App, d: &Details, t: &Theme, byline: bool) -> El {
    let title = |s: String| text(s).key(keys::INSPECTOR_TITLE).font(ty::DISPLAY_SM).color(t.ink).wrap();
    let mut h = col().w_full().gap(8.0);
    match &d.subject {
        Subject::Head(id) | Subject::Commit(id) => {
            let kicker = match (&d.subject, s.repo().and_then(|r| r.summary.status.branch.as_deref())) {
                (Subject::Head(_), Some(branch)) => format!("Head of {branch}"),
                (Subject::Head(_), None) => "Head".to_owned(),
                _ => "Commit".to_owned(),
            };
            h = h.child(ui::label(t, &kicker));
            let Some(c) = s.commit(id) else { return h };
            h = h.child(title(c.subject.clone()));
            if !byline {
                return h;
            }
            h = h.child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(ui::avatar(t, &c.author, author_color(t, &c.email)))
                    .child(text(c.author.clone()).font(ty::CAPTION_STRONG).color(t.ink))
                    .child(
                        text(time::long(c.time, s.env.now(), &s.env.tz))
                            .font(ty::CAPTION)
                            .color(t.ink_muted)
                            .help(time::absolute(c.time, &s.env.tz)),
                    ),
            );
            let short = |id: &str| id.get(..7).unwrap_or(id).to_owned();
            let chip = row()
                .px(6.0)
                .py(1.0)
                .radius(size::RADIUS_XS)
                .fill(t.canvas)
                .box_shadow(Theme::ring(1.0, t.line))
                .child(text(c.id.get(..8).unwrap_or(&c.id).to_owned()).font(ty::MONO_SM).color(t.ink));
            let mut ids = row().gap(6.0).center_y().child(chip);
            if !c.parents.is_empty() {
                let word = if c.parents.len() == 1 { "parent" } else { "parents" };
                let parents: Vec<String> = c.parents.iter().map(|p| short(p)).collect();
                ids = ids
                    .child(text(word).font(ty::CAPTION).color(t.ink_muted))
                    .child(text(parents.join(" · ")).font(ty::MONO_SM).color(t.ink_muted));
            }
            h.child(ids)
        }
        Subject::Worktree => {
            let n = d.files.len();
            let staged = s.repo().map_or(0, |r| r.summary.status.staged as usize);
            let files = if n == 1 { "1 file".to_owned() } else { format!("{n} files") };
            let note = if s.all_staged() {
                if n == 1 {
                    "1 file staged, ready to commit".to_owned()
                } else {
                    format!("All {n} files staged, ready to commit")
                }
            } else if staged == 0 {
                format!("{files} changed, nothing staged")
            } else {
                format!("{files} changed, {staged} staged")
            };
            h.child(ui::label(t, "Working tree"))
                .child(title("Uncommitted changes".to_owned()))
                .child(text(note).font(ty::CAPTION).color(t.ink_muted))
        }
        Subject::Commits(ids) => h
            .child(ui::label(t, "Selection"))
            .child(title(format!("{} commits", ids.len())))
            .child(text(selection_note(s, ids)).font(ty::CAPTION).color(t.ink_muted)),
    }
}

/// A commit's details, as labelled rows: author, committer when it's someone else or another
/// time, the message's trailers, the commit and its parents, and the refs that contain it.
fn message_details(s: &App, c: &Commit, info: &CommitInfo, t: &Theme) -> El {
    let tz = &s.env.tz;
    let when = |at| format!("{} · {}", time::date_time(at, tz), time::relative(at, s.env.now(), tz));
    let person = |name: &str, email: &str| {
        row()
            .gap(8.0)
            .center_y()
            .child(ui::avatar(t, name, author_color(t, email)))
            .child(text(name.to_owned()).font(ty::CAPTION_STRONG).color(t.ink).truncate())
    };
    let dated =
        |who: El, at| col().gap(2.0).child(who).child(text(when(at)).font(ty::CAPTION).color(t.ink_muted).wrap());
    let mut rows = vec![("Author".to_owned(), dated(person(&c.author, &c.email), c.time))];
    // The committer shows when it's someone else, and their date only when it's another time.
    let same_person = info.committer == c.author && info.committer_email == c.email;
    let same_time = (info.committed.as_second() - c.time.as_second()).abs() <= 60;
    if !(same_person && same_time) {
        let who = person(&info.committer, &info.committer_email);
        rows.push(("Committer".to_owned(), if same_time { who } else { dated(who, info.committed) }));
    }
    for (key, value) in &info.trailers {
        // `Name <email>` reads as a person; anything else as written.
        let el = match value.rsplit_once(" <").and_then(|(n, e)| e.strip_suffix('>').map(|e| (n, e))) {
            Some((name, email)) => person(name, email),
            None => row().child(text(value.clone()).font(ty::CAPTION).color(t.ink).wrap()),
        };
        rows.push((key.clone(), el));
    }
    let short = |id: &str| id.get(..7).unwrap_or(id).to_owned();
    rows.push((
        "Commit".to_owned(),
        row().child(text(c.id.get(..8).unwrap_or(&c.id).to_owned()).font(ty::MONO_SM).color(t.ink)),
    ));
    if !c.parents.is_empty() {
        let label = if c.parents.len() == 1 { "Parent" } else { "Parents" };
        let ids: Vec<String> = c.parents.iter().map(|p| short(p)).collect();
        rows.push((label.to_owned(), row().child(text(ids.join(" · ")).font(ty::MONO_SM).color(t.ink_muted))));
    }
    if !info.contained_in.is_empty() {
        // A long-lived commit can be in hundreds of tags: show the first few.
        const SHOWN: usize = 8;
        let mut refs = row().gap(6.0).flex_wrap();
        for r in info.contained_in.iter().take(SHOWN) {
            refs = refs.child(super::history::ref_badge(t, r.kind, &r.name, t.ink_faint));
        }
        if info.contained_in.len() > SHOWN {
            let more = format!("+{} more", info.contained_in.len() - SHOWN);
            refs = refs.child(row().h(20.0).center_y().child(text(more).font(ty::CAPTION).color(t.ink_muted)));
        }
        rows.push(("On".to_owned(), refs));
    }
    let mut grid = col().w_full().gap(12.0).child(ui::label(t, "Details"));
    for (label, value) in rows {
        grid = grid.child(
            row()
                .w_full()
                .gap(12.0)
                .items_start()
                .child(row().w(88.0).h(18.0).center_y().child(text(label).font(ty::CAPTION).color(t.ink_faint)))
                .child(value.grow(1.0).min_w(0.0)),
        );
    }
    grid
}

/// What a multi-selection allows.
fn selection_note(s: &App, ids: &[String]) -> &'static str {
    if s.can_bisect(ids) {
        "A linear range: bisect or squash"
    } else if s.can_squash(ids) {
        "Consecutive on one branch: can be squashed"
    } else {
        "Not consecutive on one branch: cherry-pick only"
    }
}

/// The selected commits, in a well: short id and subject, 26 tall.
fn commit_list(s: &App, ids: &[String], t: &Theme) -> El {
    let mut list = col().w_full().py(4.0).radius(size::RADIUS_LG).fill(t.canvas).box_shadow(Theme::ring(1.0, t.line));
    for id in ids {
        let Some(c) = s.commit(id) else { continue };
        list = list.child(
            row()
                .h(26.0)
                .px(12.0)
                .gap(10.0)
                .center_y()
                .child(row().w(56.0).child(text(c.short_id().to_owned()).font(ty::MONO_SM).color(t.ink_faint)))
                .child(text(c.subject.clone()).font(ty::BODY).color(t.ink).truncate()),
        );
    }
    list
}

/// `Changed files` with its stat, then a row per file.
fn files(s: &App, d: &Details, t: &Theme) -> El {
    let (added, removed) =
        d.files.iter().fold((0, 0), |(a, r), f| (a + f.added.unwrap_or(0), r + f.removed.unwrap_or(0)));
    let n = d.files.len();
    let stat = format!("{n} {} · +{added} −{removed}", if n == 1 { "file" } else { "files" });
    let staged = s.repo().map_or(0, |r| r.summary.status.staged);
    let label = match &d.subject {
        Subject::Worktree if s.all_staged() => "Staged",
        Subject::Worktree if staged == 0 => "Not staged",
        _ => "Changed files",
    };
    let head = row()
        .w_full()
        .px(8.0)
        .pb(6.0)
        .justify_between()
        .child(ui::label(t, label))
        .child(text(stat).font(ty::MONO_SM).color(t.ink_muted));
    let mut list = col().w_full();
    for f in &d.files {
        list = list.child(file_row(f, d.file.as_deref() == Some(f.path.as_str()), t));
    }
    col().key(keys::INSPECTOR_FILES).w_full().gap(2.0).child(head).child(list)
}

/// `FileChangeRow`: 28 tall, 8 a side, gap 8, radius 6; the folder muted, the name medium.
fn file_row(f: &FileChange, selected: bool, t: &Theme) -> El {
    let (dir, name) = f.path.rsplit_once('/').map_or(("", f.path.as_str()), |(d, n)| (d, n));
    let mut path = row().grow(1.0).min_w(0.0);
    if !dir.is_empty() {
        path = path.child(text(format!("{dir}/")).font(ty::BODY).color(t.ink_muted));
    }
    path = path.child(text(name.to_owned()).font(ty::BODY_STRONG).color(t.ink).truncate());
    let (a, r) = (f.added.unwrap_or(0), f.removed.unwrap_or(0));
    let mut stat = row().gap(6.0).center_y();
    if a > 0 {
        stat = stat.child(text(format!("+{a}")).font(ty::MONO_11).color(t.added));
    }
    if r > 0 {
        stat = stat.child(text(format!("−{r}")).font(ty::MONO_11).color(t.removed));
    }
    stat = stat.child(ui::pips(t, a, r));
    let letter = match f.status {
        ChangeStatus::Copied => 'A',
        ChangeStatus::TypeChanged => 'M',
        other => other.letter(),
    };
    let target = f.path.clone();
    let mut el = row()
        .key(keys::changed_file(&f.path))
        .w_full()
        .h(size::ROW_FILE)
        .px(8.0)
        .gap(8.0)
        .center_y()
        .radius(size::RADIUS_SM)
        .child(ui::status(t, letter))
        .child(path)
        .child(stat)
        .on_press(move |s: &mut App| s.select_file(&target));
    if selected {
        el = el.fill(t.raised);
        for sh in t.shadow_lifted() {
            el = el.box_shadow(sh);
        }
        el
    } else {
        el.hover(t.control_hover)
    }
}

/// `DiffHunk`: radius 12 on the canvas with a hairline ring; a chrome header; lines in a
/// 36 | 36 | 16 | rest grid, mono 12/18.
fn hunk(h: &Hunk, t: &Theme) -> El {
    let head = row()
        .w_full()
        .py(6.0)
        .px(12.0)
        .fill(t.chrome)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.line)
        .border_layout()
        .child(text(h.header.clone()).font(ty::MONO_11.lh(18.0)).color(t.ink_muted).truncate());
    let mut card = col()
        .key(keys::HUNK)
        .w_full()
        .radius(size::RADIUS_LG)
        .clip_rounded(size::RADIUS_LG)
        .fill(t.canvas)
        .box_shadow(Theme::ring(1.0, t.line))
        .child(head);
    for l in &h.lines {
        let (bg, mark, sign) = match l.kind {
            LineKind::Added => (Some(t.added_soft), t.added, "+"),
            LineKind::Removed => (Some(t.removed_soft), t.removed, "−"),
            LineKind::Context => (None, t.ink_faint, " "),
        };
        let no = |n: Option<u32>| {
            row()
                .w(36.0)
                .pr(8.0)
                .justify_end()
                .child(text(n.map(|n| n.to_string()).unwrap_or_default()).font(ty::MONO_11.lh(18.0)).color(mark))
        };
        let mut line = row()
            .w_full()
            .h(18.0)
            .center_y()
            .child(no(l.old))
            .child(no(l.new))
            .child(row().w(16.0).center().child(text(sign).font(ty::MONO).color(mark)))
            .child(
                row().grow(1.0).min_w(0.0).pr(12.0).child(text(l.text.clone()).font(ty::MONO).color(t.ink).truncate()),
            );
        if let Some(bg) = bg {
            line = line.fill(bg);
        }
        card = card.child(line);
    }
    card
}

/// The commit message, in a well like the search fields': the subject on the first line.
fn message_editor(s: &App, t: &Theme) -> El {
    let field = rune::text_input(&s.message, ty::BODY)
        .key(keys::COMMIT_MESSAGE)
        .on_edit(|s: &mut App| &mut s.message)
        .placeholder("Commit message".to_owned(), t.ink_faint)
        .color(t.ink)
        .w_full()
        .min_h(90.0);
    let mut well = col().w_full().padding(10.0).radius(size::RADIUS_SM).fill(t.well).role("textbox").child(field);
    for sh in t.shadow_well() {
        well = well.box_shadow(sh);
    }
    well
}
