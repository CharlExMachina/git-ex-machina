//! The repository's own settings: General (its name, description, topics and features), Pull
//! requests (how they merge) and the danger zone (who sees it, who owns it, whether it exists).

use gxm_core::forge::admin::{Change, Message, Page, Repo, Visibility, Write};
use rune::{El, TextEdit, col, row, text};

use super::parts::{self, Place};
use crate::keys::setting;
use crate::settings::{Action, Confirm, Event, Section, StepUp, What};
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::Variant;

fn change(c: Change) -> Action {
    Action::Forge(Write::Change(c))
}

pub fn general(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(r) = m.repo() else { return super::waiting(s, t, What::Forge(Page::Repo)) };
    let can = m.admin();
    let d = &m.drafts;
    let dirty = d.name.text.trim() != r.name
        || d.description.text.trim() != r.description
        || d.homepage.text.trim() != r.homepage;
    let mut about = vec![
        parts::labelled(
            t,
            "Name",
            parts::field(
                t,
                "name",
                &d.name,
                "Its name",
                can,
                false,
                |s: &mut App| &mut s.settings.drafts.name,
                parts::on(save_about),
            ),
        ),
        row().pl(108.0).child(parts::note(
            t,
            &format!("github.com/{}/{}. A new name keeps the old address working.", r.owner, r.name),
        )),
        parts::labelled(
            t,
            "Description",
            parts::field(
                t,
                "description",
                &d.description,
                "What it is, in a line",
                can,
                false,
                |s: &mut App| &mut s.settings.drafts.description,
                parts::on(save_about),
            ),
        ),
        parts::labelled(
            t,
            "Website",
            parts::field(
                t,
                "homepage",
                &d.homepage,
                "https://",
                can,
                false,
                |s: &mut App| &mut s.settings.drafts.homepage,
                parts::on(save_about),
            ),
        ),
    ];
    if dirty && can {
        about.push(row().pl(108.0).child(parts::controls(vec![
            parts::button(t, "save-about", "Save", Variant::Primary, true, save_about),
            parts::button(t, "cancel-about", "Cancel", Variant::Ghost, true, |s: &mut App| {
                let d = &mut s.settings.drafts;
                (d.name, d.description, d.homepage) = (TextEdit::new(""), TextEdit::new(""), TextEdit::new(""));
                s.settings_event(Event::Pick(Section::General));
            }),
            parts::note(t, "Text saves when you say so; a box saves the moment you tick it."),
        ])));
    }
    let mut topics = row().w_full().gap(6.0).center_y().flex_wrap();
    for topic in &r.topics {
        let gone = topic.clone();
        let remove: parts::Remove = can.then(|| {
            (
                format!("remove-topic-{topic}"),
                Box::new(move |s: &mut App| set_topics(s, |t| t.retain(|x| *x != gone))) as parts::Handler,
            )
        });
        topics = topics.child(parts::chip(t, "topic", topic, remove));
    }
    if can {
        topics = topics.child(row().w(160.0).child(parts::field(
            t,
            "topic",
            &d.topic,
            "Add a topic",
            true,
            false,
            |s: &mut App| &mut s.settings.drafts.topic,
            parts::on(add_topic),
        )));
    }
    about.push(parts::labelled(t, "Topics", topics));
    let gh = Place::Provider(s.settings_jobs.provider());
    let feature = |id: &str, on: bool, title: &str, hint: &str, make: fn(bool) -> Change| {
        parts::toggle(t, id, on, title, Some(hint), None, can, move |v| change(make(v)))
    };
    vec![
        parts::group(t, "About", about),
        parts::group(
            t,
            "Features",
            vec![
                feature("issues", r.issues, "Issues", "Track bugs and ideas.", Change::Issues),
                feature("projects", r.projects, "Projects", "Boards that plan the work.", Change::Projects),
                feature("wiki", r.wiki, "Wiki", "Pages anyone with access can edit.", Change::Wiki),
                feature(
                    "discussions",
                    r.discussions,
                    "Discussions",
                    "Questions and announcements, apart from issues.",
                    Change::Discussions,
                ),
                feature(
                    "template",
                    r.template,
                    "Template repository",
                    "People start new repositories from this one.",
                    Change::Template,
                ),
            ],
        ),
        parts::group(
            t,
            "Commits",
            vec![parts::toggle(
                t,
                "signoff",
                r.signoff,
                "Require sign-off on commits made on the website",
                Some("Each gets a Signed-off-by line, as a promise the author may contribute it."),
                Some(gh),
                can,
                |v| change(Change::Signoff(v)),
            )],
        ),
    ]
}

/// Sends what the About fields changed; a new name last, since the rest are asked of the old one.
fn save_about(s: &mut App) {
    let Some(r) = s.settings.repo().cloned() else { return };
    let d = &s.settings.drafts;
    let (name, description, homepage) =
        (d.name.text.trim().to_owned(), d.description.text.trim().to_owned(), d.homepage.text.trim().to_owned());
    if description != r.description {
        s.settings_do(change(Change::Description(description)));
    }
    if homepage != r.homepage {
        s.settings_do(change(Change::Homepage(homepage)));
    }
    if name != r.name && !name.is_empty() {
        s.settings_do(change(Change::Name(name)));
    }
}

fn set_topics(s: &mut App, edit: impl FnOnce(&mut Vec<String>)) {
    let Some(mut topics) = s.settings.repo().map(|r| r.topics.clone()) else { return };
    edit(&mut topics);
    s.settings_do(Action::Forge(Write::Topics(topics)));
}

/// A topic as GitHub takes one: lower case, words joined by hyphens.
fn add_topic(s: &mut App) {
    let topic = s.settings.drafts.topic.text.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join("-");
    s.settings.drafts.topic = TextEdit::new("");
    if topic.is_empty() || s.settings.repo().is_some_and(|r| r.topics.contains(&topic)) {
        return;
    }
    set_topics(s, |t| t.push(topic));
}

pub fn pull_requests(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(r) = m.repo() else { return super::waiting(s, t, What::Forge(Page::Repo)) };
    let can = m.admin();
    let methods = [r.merge_commit, r.squash, r.rebase].iter().filter(|on| **on).count();
    // GitHub needs at least one way to merge: the last one stays on.
    let can_turn = |on: bool| can && !(on && methods == 1);
    let message = |id: &str, current: Message, make: fn(Message) -> Change| {
        let opts: Vec<(&str, &str, bool)> = [
            ("default", Message::Default),
            ("title", Message::Title),
            ("title-and-description", Message::TitleAndDescription),
        ]
        .iter()
        .map(|(opt, msg)| (*opt, msg.word(), *msg == current))
        .collect();
        row().pl(26.0).gap(10.0).center_y().child(parts::note(t, "Its message")).child(parts::choice(
            t,
            id,
            &opts,
            can,
            move |s: &mut App, i| {
                s.settings_do(change(make(Message::ALL[i])));
            },
        ))
    };
    let mut how = vec![parts::toggle(
        t,
        "merge-commit",
        r.merge_commit,
        "Merge commits",
        Some("Every commit lands, joined by a merge commit."),
        None,
        can_turn(r.merge_commit),
        |v| change(Change::MergeCommit(v)),
    )];
    if r.merge_commit {
        how.push(message("merge-message", r.merge_message, Change::MergeMessage));
    }
    how.push(parts::toggle(
        t,
        "squash",
        r.squash,
        "Squash",
        Some("The pull request lands as one commit."),
        None,
        can_turn(r.squash),
        |v| change(Change::Squash(v)),
    ));
    if r.squash {
        how.push(message("squash-message", r.squash_message, Change::SquashMessage));
    }
    how.push(parts::toggle(
        t,
        "rebase",
        r.rebase,
        "Rebase",
        Some("Each commit lands on top of the base, one by one, with no merge commit."),
        None,
        can_turn(r.rebase),
        |v| change(Change::Rebase(v)),
    ));
    let delete = if r.delete_branch_on_merge { "On" } else { "Off" };
    vec![
        parts::group(t, "How pull requests merge", how),
        parts::group(
            t,
            "Before merging",
            vec![
                parts::toggle(
                    t,
                    "update-branch",
                    r.update_branch,
                    "Suggest updating a branch that's behind its base",
                    Some("An Update branch button on pull requests the base has moved past."),
                    None,
                    can,
                    |v| change(Change::UpdateBranch(v)),
                ),
                parts::toggle(
                    t,
                    "auto-merge",
                    r.auto_merge,
                    "Allow auto-merge",
                    Some("A pull request merges itself once its checks pass and its reviews are in."),
                    None,
                    can,
                    |v| change(Change::AutoMerge(v)),
                ),
            ],
        ),
        parts::list(
            t,
            vec![
                row()
                    .gap(12.0)
                    .center_y()
                    .child(parts::named(
                        t,
                        "Deleting branches after a merge",
                        &format!("{delete} for {}. It lives with the rest of the branch settings.", r.name),
                        false,
                    ))
                    .child(parts::button(t, "open-branches", "Branches", Variant::Secondary, true, |s: &mut App| {
                        s.settings_event(Event::Pick(Section::Branches));
                    })),
            ],
        ),
    ]
}

pub fn danger_zone(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let full = s.settings_jobs.admin.as_ref().map(|a| a.full_name()).unwrap_or_default();
    if m.deleted {
        return vec![parts::list(
            t,
            vec![parts::named(
                t,
                &format!("Deleted {full} on GitHub"),
                "Your copy on this computer stays, with its history and its branches.",
                false,
            )],
        )];
    }
    let Some(r) = m.repo() else { return super::waiting(s, t, What::Forge(Page::Repo)) };
    if !m.admin() {
        return Vec::new();
    }
    let rows = vec![
        visibility(t, r, &full, m.confirm),
        transfer(s, t, m.confirm),
        archive(t, r, &full, m.confirm),
        delete(s, t, &full),
    ];
    let mut card = col().w_full().radius(size::RADIUS_LG).fill(t.canvas).box_shadow(Theme::ring(1.0, t.removed_soft));
    for (i, r) in rows.into_iter().enumerate() {
        let r = r.w_full().px(16.0).py(14.0);
        card = card.child(if i == 0 { r } else { r.border_sides([1.0, 0.0, 0.0, 0.0], t.removed_soft) });
    }
    vec![
        parts::note(t, "These change who can find it, who owns it, or whether it exists. Each asks you to confirm."),
        card,
    ]
}

/// A danger row: what it is, what it means, and its control (or its confirmation).
fn danger_row(t: &Theme, title: &str, about: &str, control: El) -> El {
    row().gap(14.0).center_y().child(parts::named(t, title, about, false)).child(control)
}

/// Controls at the end of a danger row, as wide as they need.
fn inline(children: Vec<El>) -> El {
    row().gap(8.0).center_y().children(children)
}

fn confirming(t: &Theme, id: &str, label: &str, then: impl Fn(&mut App) + 'static) -> El {
    inline(vec![
        parts::button(t, "cancel-confirm", "Cancel", Variant::Ghost, true, |s: &mut App| {
            s.settings.confirm = Confirm::None;
        }),
        parts::button(t, id, label, Variant::Danger, true, move |s: &mut App| {
            s.settings.confirm = Confirm::None;
            then(s);
        }),
    ])
}

fn visibility(t: &Theme, r: &Repo, full: &str, confirm: Confirm) -> El {
    let public = r.visibility == Visibility::Public;
    let (title, about, label, to) = if public {
        ("Make it private", "Only the people and teams with access will see it.", "Make private", Visibility::Private)
    } else {
        ("Make it public", "Anyone could see its code, its history and its issues.", "Make public", Visibility::Public)
    };
    let control = if confirm == Confirm::Visibility {
        let words = if public { format!("Make {full} private") } else { format!("Make {full} public") };
        confirming(t, "confirm-visibility", &words, move |s: &mut App| {
            s.settings_do(change(Change::Visibility(to)));
        })
    } else {
        parts::button(t, "visibility", label, Variant::Danger, true, |s: &mut App| {
            s.settings.confirm = Confirm::Visibility;
        })
    };
    danger_row(t, title, about, control)
}

fn transfer(s: &App, t: &Theme, confirm: Confirm) -> El {
    let to = s.settings.drafts.transfer_to.text.trim().to_owned();
    let control = if confirm == Confirm::Transfer {
        let owner = to.clone();
        confirming(t, "confirm-transfer", &format!("Transfer to {to}"), move |s: &mut App| {
            s.settings_do(Action::Forge(Write::Transfer(owner.clone())));
        })
    } else {
        inline(vec![
            row().w(160.0).child(parts::field(
                t,
                "transfer-to",
                &s.settings.drafts.transfer_to,
                "New owner",
                true,
                false,
                |s: &mut App| &mut s.settings.drafts.transfer_to,
                None,
            )),
            parts::button(t, "transfer", "Transfer", Variant::Danger, !to.is_empty(), |s: &mut App| {
                s.settings.confirm = Confirm::Transfer;
            }),
        ])
    };
    danger_row(t, "Transfer it", "To another account or organisation. The old address redirects.", control)
}

fn archive(t: &Theme, r: &Repo, full: &str, confirm: Confirm) -> El {
    let archived = r.archived;
    let (title, about, label) = if archived {
        ("Unarchive it", "It takes pushes, issues and pull requests again.", "Unarchive")
    } else {
        ("Archive it", "Everything becomes read-only. You can unarchive it later.", "Archive")
    };
    let control = if confirm == Confirm::Archive {
        confirming(t, "confirm-archive", &format!("{label} {full}"), move |s: &mut App| {
            s.settings_do(change(Change::Archived(!archived)));
        })
    } else {
        parts::button(t, "archive", label, Variant::Danger, true, |s: &mut App| s.settings.confirm = Confirm::Archive)
    };
    danger_row(t, title, about, control)
}

/// Deleting: type the full name to confirm. GitHub wants a permission the app doesn't ask for
/// at sign-in, so the first try asks for it with a one-time code, then deletes.
fn delete(s: &App, t: &Theme, full: &str) -> El {
    let typed = s.settings.drafts.confirm_delete.text.trim() == full;
    let mut body = col().w_full().gap(10.0).child(parts::named(
        t,
        "Delete it",
        "The repository, its issues, pull requests, wiki and settings, for good. Your copy on this computer stays.",
        false,
    ));
    body = body.child(parts::controls(vec![
        row().grow(1.0).child(parts::field(
            t,
            "confirm-delete",
            &s.settings.drafts.confirm_delete,
            &format!("Type {full} to confirm"),
            true,
            false,
            |s: &mut App| &mut s.settings.drafts.confirm_delete,
            None,
        )),
        parts::button(
            t,
            "delete-repository",
            &format!("Delete {full}"),
            Variant::Danger,
            typed && s.settings.step_up == StepUp::None,
            |s: &mut App| {
                s.settings_do(Action::Forge(Write::Delete));
            },
        ),
    ]));
    match &s.settings.step_up {
        StepUp::None => {}
        StepUp::Asking => {
            body = body.child(parts::note(t, "GitHub wants one more permission for this. Asking it for a code…"));
        }
        StepUp::Code(code) => {
            let uri = code.verification_uri.clone();
            body = body
                .child(parts::note(
                    t,
                    "GitHub wants one more permission for this, delete_repo, which the app doesn't ask for when you sign in. Approve it with this code; the repository goes once you do.",
                ))
                .child(
                    row()
                        .gap(12.0)
                        .center_y()
                        .child(text(code.user_code.clone()).key(setting("step-up-code")).font(ty::CODE).color(t.ink))
                        .child(crate::ui::button(t, Variant::Primary, None, "Approve on GitHub", None).key(setting("step-up-open")).on_press_tx(
                            move |_: &mut App, tx| {
                                // A refused URL is the system's to report; the code still works.
                                tx.open_url(&uri).ok();
                            },
                        ))
                        .child(parts::button(t, "step-up-cancel", "Cancel", Variant::Ghost, true, App::cancel_step_up)),
                );
        }
        StepUp::Failed(why) => {
            body = body.child(text(why.clone()).font(ty::CAPTION).color(t.removed).wrap()).child(parts::button(
                t,
                "step-up-retry",
                "Try again",
                Variant::Secondary,
                true,
                App::step_up,
            ));
        }
    }
    body
}
