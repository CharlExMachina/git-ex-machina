//! Who and what may change the repository: its rule sets (what protects its branches and tags)
//! and its people (collaborators, teams, invitations).

use gxm_core::forge::admin::{DEFAULT_BRANCH, Enforcement, Grantee, Page, Reading, Role, Ruleset, Target, Write};
use gxm_core::time;
use rune::{El, TextEdit, icon, row, text};

use super::parts;
use crate::icons;
use crate::settings::Action;
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

fn save(r: Ruleset) -> Action {
    Action::Forge(Write::SaveRuleset(Box::new(r)))
}

pub fn rules(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Reading::Rules(list)) = m.forge(Page::Rules) else {
        return super::waiting(s, t, crate::settings::What::Forge(Page::Rules));
    };
    let can = m.admin();
    let mut out = vec![parts::controls(vec![
        row().grow(1.0).min_w(0.0).child(parts::note(
            t,
            "Rules keep force-pushes, deletions and unreviewed merges away from the branches and tags that matter.",
        )),
        parts::button(t, "new-rule", "New rule", Variant::Primary, can, |s: &mut App| {
            let r = Ruleset::new_for_default_branch();
            s.settings.rule_open = Some(r.name.clone());
            s.settings_do(save(r));
        }),
    ])];
    if list.is_empty() {
        out.push(parts::note(t, "No rules yet: anyone who can push can force-push or delete any branch."));
    }
    for r in list {
        let open = m.rule_open.as_deref() == Some(r.name.as_str());
        let name = r.name.clone();
        let targets: Vec<String> = r.include.iter().map(|i| target_word(r.target, i)).collect();
        let head = parts::item("rule", &r.name)
            .role("button")
            .label(r.name.clone())
            .child(icon(if open { &icons::CHEVRON } else { &icons::CHEVRON_RIGHT }, 12.0).color(t.ink_muted))
            .child(parts::named(t, &r.name, &format!("{} · {}", targets.join(", "), r.summary().join(" · ")), false))
            .child(enforcement_pill(t, r.enforcement))
            .on_press(move |s: &mut App| {
                let m = &mut s.settings;
                m.rule_open = if m.rule_open.as_deref() == Some(name.as_str()) { None } else { Some(name.clone()) };
            });
        let mut rows = vec![head];
        if open {
            rows.extend(rule_body(s, t, r, can));
        }
        out.push(parts::list(t, rows));
    }
    out
}

/// A rule set's target as people say it: `main`, `release/*`, `the default branch`, `tags v*`.
fn target_word(target: Target, include: &str) -> String {
    let word = match include {
        DEFAULT_BRANCH => "the default branch".to_owned(),
        "~ALL" => "everything".to_owned(),
        other => other.to_owned(),
    };
    if target == Target::Tag { format!("tags {word}") } else { word }
}

fn enforcement_pill(t: &Theme, e: Enforcement) -> El {
    let (fg, bg) = match e {
        Enforcement::Active => (t.added, t.added_soft),
        Enforcement::Evaluate => (t.accent, t.accent_soft),
        Enforcement::Disabled => (t.ink_muted, t.control_hover),
    };
    row()
        .h(20.0)
        .px(8.0)
        .center()
        .radius(size::PILL)
        .fill(bg)
        .child(text(e.word().to_owned()).font(ty::STATE).color(fg))
}

/// An open rule set's controls: each change saves the whole set.
fn rule_body(s: &App, t: &Theme, r: &Ruleset, can: bool) -> Vec<El> {
    let with = |edit: fn(&mut Ruleset, bool)| {
        let r = r.clone();
        move |v: bool| {
            let mut r = r.clone();
            edit(&mut r, v);
            save(r)
        }
    };
    let x = &r.rules;
    let enforcement: Vec<(&str, &str, bool)> =
        [("active", Enforcement::Active), ("trial", Enforcement::Evaluate), ("off", Enforcement::Disabled)]
            .iter()
            .map(|(id, e)| (*id, e.word(), r.enforcement == *e))
            .collect();
    let set = r.clone();
    let mut out = vec![row().gap(12.0).center_y().child(parts::note(t, "It's")).child(parts::choice(
        t,
        "rule-enforcement",
        &enforcement,
        can,
        move |s: &mut App, i| {
            let mut r = set.clone();
            r.enforcement = Enforcement::ALL[i];
            s.settings_do(save(r));
        },
    ))];
    let approvals = x.pull_request.unwrap_or(0);
    let mut pr = row().gap(12.0).center_y().child(
        parts::toggle(
            t,
            "rule-pull-request",
            x.pull_request.is_some(),
            "Require a pull request",
            Some("Nothing reaches these branches without one."),
            None,
            can,
            with(|r, v| {
                r.rules.pull_request = v.then_some(1);
            }),
        )
        .grow(1.0),
    );
    if x.pull_request.is_some() {
        let (less, more) = (r.clone(), r.clone());
        pr = pr
            .child(parts::icon_button(
                t,
                "rule-approvals-less",
                &icons::BACK,
                "Fewer approvals",
                can && approvals > 0,
                move |s: &mut App| {
                    let mut r = less.clone();
                    r.rules.pull_request = Some(approvals.saturating_sub(1));
                    s.settings_do(save(r));
                },
            ))
            .child(
                text(if approvals == 1 { "1 approval".to_owned() } else { format!("{approvals} approvals") })
                    .font(ty::CONTROL)
                    .color(t.ink)
                    .w(76.0),
            )
            .child(parts::icon_button(
                t,
                "rule-approvals-more",
                &icons::CHEVRON_RIGHT,
                "More approvals",
                can && approvals < 6,
                move |s: &mut App| {
                    let mut r = more.clone();
                    r.rules.pull_request = Some(approvals + 1);
                    s.settings_do(save(r));
                },
            ));
    }
    out.push(pr);
    out.push(parts::toggle(
        t,
        "rule-checks",
        x.checks.is_some(),
        "Require checks to pass",
        Some("These checks pass before anything reaches these branches."),
        None,
        can,
        with(|r, v| {
            r.rules.checks = v.then(Vec::new);
        }),
    ));
    if let Some(checks) = &x.checks {
        let mut chips = row().pl(28.0).gap(6.0).center_y().flex_wrap();
        for c in checks {
            let (gone, r2) = (c.clone(), r.clone());
            let remove: parts::Remove = can.then(|| {
                (
                    format!("remove-check-{c}"),
                    Box::new(move |s: &mut App| {
                        let mut r = r2.clone();
                        if let Some(cs) = &mut r.rules.checks {
                            cs.retain(|x| *x != gone);
                        }
                        s.settings_do(save(r));
                    }) as parts::Handler,
                )
            });
            chips = chips.child(parts::chip(t, "check", c, remove));
        }
        if can {
            chips = chips.child(row().w(160.0).child(parts::field(
                t,
                "rule-check",
                &s.settings.drafts.rule_check,
                "Add a check",
                true,
                false,
                |s: &mut App| &mut s.settings.drafts.rule_check,
                parts::on(add_check),
            )));
        }
        out.push(chips);
    }
    for (id, on, title, edit) in [
        (
            "rule-force-push",
            x.block_force_push,
            "Block force-pushes",
            (|r: &mut Ruleset, v| r.rules.block_force_push = v) as fn(&mut Ruleset, bool),
        ),
        ("rule-deletion", x.block_deletion, "Block deleting them", |r, v| r.rules.block_deletion = v),
        ("rule-signed", x.signed_commits, "Require signed commits", |r, v| r.rules.signed_commits = v),
        ("rule-linear", x.linear_history, "Require a straight history", |r, v| r.rules.linear_history = v),
        ("rule-bypass", r.admins_bypass, "Admins can push past it", |r, v| r.admins_bypass = v),
    ] {
        out.push(parts::toggle(t, id, on, title, None, None, can, with(edit)));
    }
    let id = r.id;
    out.push(parts::controls(vec![
        rune::el().grow(1.0),
        parts::button(t, "delete-rule", "Delete rule", Variant::Danger, can, move |s: &mut App| {
            s.settings.rule_open = None;
            s.settings_do(Action::Forge(Write::DeleteRuleset(id)));
        }),
    ]));
    out
}

/// Adds the typed check to the open rule set.
fn add_check(s: &mut App) {
    let check = s.settings.drafts.rule_check.text.trim().to_owned();
    s.settings.drafts.rule_check = TextEdit::new("");
    let Some(Reading::Rules(list)) = s.settings.forge(Page::Rules) else { return };
    let Some(mut r) = list.iter().find(|r| s.settings.rule_open.as_deref() == Some(r.name.as_str())).cloned() else {
        return;
    };
    let checks = r.rules.checks.get_or_insert_with(Vec::new);
    if check.is_empty() || checks.contains(&check) {
        return;
    }
    checks.push(check);
    s.settings_do(save(r));
}

pub fn people(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Reading::People(p)) = m.forge(Page::People) else {
        return super::waiting(s, t, crate::settings::What::Forge(Page::People));
    };
    let can = m.admin();
    let me = s.account.as_ref().map(|a| a.login.clone()).unwrap_or_default();
    let owner = m.repo().map(|r| r.owner.clone()).unwrap_or_default();
    let roles = |current: Role| -> Vec<(&'static str, &'static str, bool)> {
        Role::ALL.iter().map(|r| (role_id(*r), r.word(), *r == current)).collect()
    };
    let typed = !m.drafts.invite.text.trim().is_empty();
    let mut out = vec![parts::controls(vec![
        row().grow(1.0).child(parts::field(
            t,
            "invite",
            &m.drafts.invite,
            "Invite by username",
            can,
            false,
            |s: &mut App| &mut s.settings.drafts.invite,
            parts::on(invite),
        )),
        parts::choice(t, "invite-role", &roles(m.invite_role), can, |s: &mut App, i| {
            s.settings.invite_role = Role::ALL[i];
        }),
        parts::button(t, "invite", "Invite", Variant::Primary, can && typed, invite),
    ])];
    if let Some(base) = p.base {
        out.push(parts::note(
            t,
            &format!(
                "Everyone in {owner} can {} it: {owner}'s owners set that, for every repository at once.",
                base.word().to_lowercase()
            ),
        ));
    }
    let mut rows = Vec::new();
    for c in &p.collaborators {
        let mine = c.login == me;
        let (who, gone) = (c.login.clone(), c.login.clone());
        rows.push(
            parts::item("person", &c.login)
                .child(ui::avatar(t, &c.login, super::super::history::author_color(t, &c.login)))
                .child(parts::named(
                    t,
                    &c.login,
                    &if mine { "You".to_owned() } else { format!("{} {}", c.role.word(), c.role.means()) },
                    false,
                ))
                .child(parts::choice(
                    t,
                    &format!("role-{}", c.login),
                    &roles(c.role),
                    can && !mine,
                    move |s: &mut App, i| {
                        s.settings_do(Action::Forge(Write::SetRole {
                            who: Grantee::Person(who.clone()),
                            role: Role::ALL[i],
                        }));
                    },
                ))
                .child(parts::icon_button(
                    t,
                    &format!("remove-{}", c.login),
                    &icons::X,
                    &format!("Remove {}", c.login),
                    can && !mine,
                    move |s: &mut App| {
                        s.settings_do(Action::Forge(Write::Remove(Grantee::Person(gone.clone()))));
                    },
                )),
        );
    }
    for team in &p.teams {
        let (slug, gone) = (team.slug.clone(), team.slug.clone());
        rows.push(
            parts::item("team", &team.slug)
                .child(ui::tile(t, &team.name, t.lane(3)))
                .child(parts::named(t, &team.name, "Team", false))
                .child(parts::choice(
                    t,
                    &format!("role-team-{}", team.slug),
                    &roles(team.role),
                    can,
                    move |s: &mut App, i| {
                        s.settings_do(Action::Forge(Write::SetRole {
                            who: Grantee::Team(slug.clone()),
                            role: Role::ALL[i],
                        }));
                    },
                ))
                .child(parts::icon_button(
                    t,
                    &format!("remove-team-{}", team.slug),
                    &icons::X,
                    &format!("Remove {}", team.name),
                    can,
                    move |s: &mut App| {
                        s.settings_do(Action::Forge(Write::Remove(Grantee::Team(gone.clone()))));
                    },
                )),
        );
    }
    for i in &p.invitations {
        let id = i.id;
        let sent = time::long(i.sent, s.env.now(), &s.env.tz);
        rows.push(
            parts::item("invitation", &i.login)
                .child(row().size(16.0, 16.0).radius(8.0).box_shadow(Theme::ring(1.25, t.edge)))
                .child(parts::named(t, &i.login, &format!("Invited {sent} · hasn’t accepted yet"), false))
                .child(text(i.role.word().to_owned()).font(ty::CONTROL).color(t.ink_muted))
                .child(parts::button(
                    t,
                    &format!("cancel-invite-{}", i.login),
                    "Cancel invite",
                    Variant::Ghost,
                    can,
                    move |s: &mut App| {
                        s.settings_do(Action::Forge(Write::CancelInvite(id)));
                    },
                )),
        );
    }
    out.push(parts::list(t, rows));
    out.push(parts::note(
        t,
        &Role::ALL.iter().map(|r| format!("{} {}", r.word(), r.means())).collect::<Vec<_>>().join(" · "),
    ));
    out
}

fn role_id(r: Role) -> &'static str {
    match r {
        Role::Read => "read",
        Role::Triage => "triage",
        Role::Write => "write",
        Role::Maintain => "maintain",
        Role::Admin => "admin",
    }
}

fn invite(s: &mut App) {
    let login = s.settings.drafts.invite.text.trim().trim_start_matches('@').to_owned();
    if login.is_empty() {
        return;
    }
    s.settings.drafts.invite = TextEdit::new("");
    let role = s.settings.invite_role;
    s.settings_do(Action::Forge(Write::Invite { login, role }));
}
