//! What runs for the repository: Actions (which may run, what their token may do), secrets and
//! variables, deployment environments, webhooks, and the Pages site.

use gxm_core::forge::admin::{Actions, Allowed, Environment, NewHook, Page, PagesSource, Reading, Scope, Write};
use gxm_core::time;
use rune::{El, TextEdit, col, row, text};

use super::parts;
use crate::icons;
use crate::keys::setting_item;
use crate::settings::{Action, Event, What};
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::Variant;

fn forge(w: Write) -> Action {
    Action::Forge(w)
}

pub fn actions(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Reading::Actions(a)) = m.forge(Page::Actions) else {
        return super::waiting(s, t, What::Forge(Page::Actions));
    };
    let can = m.admin();
    let with = |edit: fn(&mut Actions, bool)| {
        let a = a.clone();
        move |v: bool| {
            let mut a = a.clone();
            edit(&mut a, v);
            forge(Write::Actions(a))
        }
    };
    let mut out = vec![parts::toggle(
        t,
        "actions-enabled",
        a.enabled,
        "Workflows run in this repository",
        Some("Off, nothing in .github/workflows runs."),
        None,
        can,
        with(|a, v| a.enabled = v),
    )];
    if a.enabled {
        let opts: Vec<(&str, &str, bool)> =
            [("all", Allowed::All), ("selected", Allowed::Selected), ("local", Allowed::Local)]
                .iter()
                .map(|(id, x)| (*id, x.word(), a.allowed == *x))
                .collect();
        let set = a.clone();
        out.push(parts::group(
            t,
            "Which actions they may use",
            vec![parts::choice(t, "allowed", &opts, can, move |s: &mut App, i| {
                let mut a = set.clone();
                a.allowed = Allowed::ALL[i];
                s.settings_do(forge(Write::Actions(a)));
            })],
        ));
    }
    out.push(parts::group(
        t,
        "The workflow token",
        vec![
            parts::toggle(
                t,
                "workflow-write",
                a.write,
                "It may write to the repository",
                Some("Else it only reads, and a workflow asks for more where it needs it."),
                None,
                can,
                with(|a, v| a.write = v),
            ),
            parts::toggle(
                t,
                "approve-reviews",
                a.approve_reviews,
                "Workflows may approve pull requests",
                None,
                None,
                can,
                with(|a, v| a.approve_reviews = v),
            ),
        ],
    ));
    out
}

pub fn secrets(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let can = m.admin();
    let scopes: Vec<(&str, &str, bool)> = Scope::ALL.iter().map(|sc| (sc.path(), sc.word(), *sc == m.scope)).collect();
    let mut out = vec![parts::controls(vec![
        parts::choice(t, "scope", &scopes, true, |s: &mut App, i| s.settings_event(Event::Scope(Scope::ALL[i]))),
        parts::note(t, "What reads them. Environment secrets live with their environment."),
    ])];
    let Some(Reading::Secrets(_, list)) = m.forge(Page::Secrets(m.scope)) else {
        out.extend(super::waiting(s, t, What::Forge(Page::Secrets(m.scope))));
        return out;
    };
    let d = &m.drafts;
    let mut rows: Vec<El> = list
        .iter()
        .map(|x| {
            let (scope, name) = (m.scope, x.name.clone());
            parts::item("secret", &x.name)
                .child(rune::icon(&icons::KEY, 14.0).color(t.ink_muted))
                .child(parts::named(
                    t,
                    &x.name,
                    &format!("Updated {}", time::long(x.updated, s.env.now(), &s.env.tz)),
                    true,
                ))
                .child(parts::icon_button(
                    t,
                    &format!("delete-secret-{}", x.name),
                    &icons::TRASH,
                    &format!("Delete {}", x.name),
                    can,
                    move |s: &mut App| {
                        s.settings_do(forge(Write::DeleteSecret { scope, name: name.clone() }));
                    },
                ))
        })
        .collect();
    if rows.is_empty() {
        rows.push(parts::note(t, &format!("No secrets for {} yet.", m.scope.word())));
    }
    let name = secret_name(&d.secret_name.text);
    let replace = list.iter().any(|x| x.name == name);
    rows.push(
        col()
            .w_full()
            .gap(8.0)
            .child(parts::controls(vec![
                row().w(200.0).child(parts::field(
                    t,
                    "secret-name",
                    &d.secret_name,
                    "NAME",
                    can,
                    false,
                    |s: &mut App| &mut s.settings.drafts.secret_name,
                    None,
                )),
                row().grow(1.0).child(parts::field(
                    t,
                    "secret-value",
                    &d.secret_value,
                    "Its value",
                    can,
                    true,
                    |s: &mut App| &mut s.settings.drafts.secret_value,
                    parts::on(add_secret),
                )),
                parts::button(
                    t,
                    "add-secret",
                    if replace { "Replace secret" } else { "Add secret" },
                    Variant::Primary,
                    can && !name.is_empty() && !d.secret_value.text.is_empty(),
                    add_secret,
                ),
            ]))
            .child(parts::note(t, "Sealed on this computer before it’s sent. Nobody reads it back, you included.")),
    );
    out.push(parts::group(t, "Secrets", vec![parts::list(t, rows)]));
    if m.scope != Scope::Actions {
        return out;
    }
    let Some(Reading::Variables(vars)) = m.forge(Page::Variables) else { return out };
    let mut rows: Vec<El> = vars
        .iter()
        .map(|v| {
            let name = v.name.clone();
            parts::item("variable", &v.name).child(parts::named(t, &v.name, &v.value, true)).child(parts::icon_button(
                t,
                &format!("delete-variable-{}", v.name),
                &icons::TRASH,
                &format!("Delete {}", v.name),
                can,
                move |s: &mut App| s.settings_do(forge(Write::DeleteVariable(name.clone()))),
            ))
        })
        .collect();
    let vname = secret_name(&d.var_name.text);
    let exists = vars.iter().any(|v| v.name == vname);
    rows.push(parts::controls(vec![
        row().w(200.0).child(parts::field(
            t,
            "variable-name",
            &d.var_name,
            "NAME",
            can,
            false,
            |s: &mut App| &mut s.settings.drafts.var_name,
            None,
        )),
        row().grow(1.0).child(parts::field(
            t,
            "variable-value",
            &d.var_value,
            "Its value",
            can,
            false,
            |s: &mut App| &mut s.settings.drafts.var_value,
            parts::on(add_variable),
        )),
        parts::button(
            t,
            "add-variable",
            if exists { "Change variable" } else { "Add variable" },
            Variant::Secondary,
            can && !vname.is_empty(),
            add_variable,
        ),
    ]));
    out.push(parts::group(
        t,
        "Variables",
        vec![
            parts::note(t, "Plain values workflows read as vars.NAME: not for anything secret."),
            parts::list(t, rows),
        ],
    ));
    out
}

/// A secret's or variable's name as GitHub takes it: capitals and underscores.
fn secret_name(typed: &str) -> String {
    typed.trim().to_uppercase().split_whitespace().collect::<Vec<_>>().join("_")
}

fn add_secret(s: &mut App) {
    let d = &mut s.settings.drafts;
    let (name, value) = (secret_name(&d.secret_name.text), d.secret_value.text.clone());
    if name.is_empty() || value.is_empty() {
        return;
    }
    (d.secret_name, d.secret_value) = (TextEdit::new(""), TextEdit::new(""));
    let scope = s.settings.scope;
    s.settings_do(forge(Write::PutSecret { scope, name, value }));
}

fn add_variable(s: &mut App) {
    let d = &mut s.settings.drafts;
    let (name, value) = (secret_name(&d.var_name.text), d.var_value.text.trim().to_owned());
    if name.is_empty() {
        return;
    }
    (d.var_name, d.var_value) = (TextEdit::new(""), TextEdit::new(""));
    let new =
        !matches!(s.settings.forge(Page::Variables), Some(Reading::Variables(v)) if v.iter().any(|x| x.name == name));
    s.settings_do(forge(Write::PutVariable { name, value, new }));
}

/// How long a deployment can be made to wait, in minutes, step by step.
const WAITS: [u32; 7] = [0, 1, 5, 10, 15, 30, 60];

fn wait_words(n: u32) -> String {
    match n {
        0 => "Starts at once".to_owned(),
        1 => "Waits 1 minute".to_owned(),
        n => format!("Waits {n} minutes"),
    }
}

pub fn environments(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Reading::Environments(list)) = m.forge(Page::Environments) else {
        return super::waiting(s, t, What::Forge(Page::Environments));
    };
    let can = m.admin();
    let mut rows: Vec<El> = list.iter().map(|e| environment(s, t, e, can)).collect();
    if rows.is_empty() {
        rows.push(parts::note(t, "No environments yet: workflows deploy without waiting or asking."));
    }
    let typed = !m.drafts.env_name.text.trim().is_empty();
    rows.push(parts::controls(vec![
        row().grow(1.0).child(parts::field(
            t,
            "environment-name",
            &m.drafts.env_name,
            "A new environment: staging, production…",
            can,
            false,
            |s: &mut App| &mut s.settings.drafts.env_name,
            parts::on(add_environment),
        )),
        parts::button(t, "add-environment", "Add environment", Variant::Secondary, can && typed, add_environment),
    ]));
    vec![
        parts::note(t, "Where workflows deploy, and what guards each: a wait, someone's approval, which branches may."),
        parts::list(t, rows),
    ]
}

fn environment(s: &App, t: &Theme, e: &Environment, can: bool) -> El {
    let save = |edit: Box<dyn Fn(&mut Environment)>| {
        let e = e.clone();
        move |s: &mut App| {
            let mut e = e.clone();
            edit(&mut e);
            s.settings_do(forge(Write::SaveEnvironment(e)));
        }
    };
    let at = WAITS.iter().position(|w| *w >= e.wait_minutes).unwrap_or(0);
    let approval = if e.reviewers.is_empty() { "No approval needed".to_owned() } else { "Approved by".to_owned() };
    let summary = format!(
        "{} · {}",
        wait_words(e.wait_minutes),
        if e.protected_only { "protected branches only" } else { "any branch" }
    );
    let mut reviewers = row().gap(6.0).center_y().flex_wrap().child(parts::note(t, &approval));
    for login in &e.reviewers {
        let gone = login.clone();
        let remove: parts::Remove = can.then(|| {
            (
                format!("remove-reviewer-{}-{login}", e.name),
                Box::new(save(Box::new(move |e| e.reviewers.retain(|r| *r != gone)))) as parts::Handler,
            )
        });
        reviewers = reviewers.child(parts::chip(t, "reviewer", login, remove));
    }
    if can {
        let empty = TextEdit::new("");
        let edit = s.settings.drafts.reviewers.get(&e.name).unwrap_or(&empty);
        let (name, env) = (e.name.clone(), e.clone());
        let add = move |s: &mut App| {
            let login = s
                .settings
                .drafts
                .reviewers
                .remove(&env.name)
                .map(|t| t.text.trim().trim_start_matches('@').to_owned())
                .unwrap_or_default();
            if login.is_empty() || env.reviewers.contains(&login) {
                return;
            }
            let mut e = env.clone();
            e.reviewers.push(login);
            s.settings_do(forge(Write::SaveEnvironment(e)));
        };
        reviewers = reviewers.child(row().w(150.0).child(parts::field(
            t,
            &format!("reviewer-{}", e.name),
            edit,
            "Add someone",
            true,
            false,
            move |s: &mut App| s.settings.drafts.reviewers.entry(name.clone()).or_insert_with(|| TextEdit::new("")),
            parts::on(add),
        )));
    }
    let name = e.name.clone();
    parts::item("environment", &e.name).child(
        col()
            .grow(1.0)
            .min_w(0.0)
            .gap(8.0)
            .child(row().gap(12.0).center_y().child(parts::named(t, &e.name, &summary, false)).child(
                parts::icon_button(
                    t,
                    &format!("delete-environment-{}", e.name),
                    &icons::TRASH,
                    &format!("Delete {}", e.name),
                    can,
                    move |s: &mut App| s.settings_do(forge(Write::DeleteEnvironment(name.clone()))),
                ),
            ))
            .child(parts::controls(vec![
                parts::icon_button(
                    t,
                    &format!("wait-less-{}", e.name),
                    &icons::BACK,
                    "Wait less",
                    can && at > 0,
                    save(Box::new(move |e| e.wait_minutes = WAITS[at.saturating_sub(1)])),
                ),
                text(wait_words(e.wait_minutes)).font(ty::CONTROL).color(t.ink).w(108.0),
                parts::icon_button(
                    t,
                    &format!("wait-more-{}", e.name),
                    &icons::CHEVRON_RIGHT,
                    "Wait more",
                    can && at + 1 < WAITS.len(),
                    save(Box::new(move |e| e.wait_minutes = WAITS[(at + 1).min(WAITS.len() - 1)])),
                ),
                rune::el().w(12.0),
                {
                    let on = e.protected_only;
                    let c = parts::check(
                        t,
                        &format!("protected-{}", e.name),
                        on,
                        "Only protected branches deploy",
                        save(Box::new(move |e| e.protected_only = !on)),
                    );
                    if can { c } else { c.disabled(true).opacity(0.5) }
                },
            ]))
            .child(reviewers),
    )
}

fn add_environment(s: &mut App) {
    let name = s.settings.drafts.env_name.text.trim().to_owned();
    if name.is_empty() {
        return;
    }
    s.settings.drafts.env_name = TextEdit::new("");
    s.settings_do(forge(Write::SaveEnvironment(Environment {
        name,
        wait_minutes: 0,
        reviewers: Vec::new(),
        protected_only: false,
    })));
}

/// A webhook's events as people say them.
fn events_words(events: &[String]) -> String {
    if events.iter().any(|e| e == "*") {
        return "Everything".to_owned();
    }
    let words: Vec<String> = events
        .iter()
        .map(|e| match e.as_str() {
            "push" => "pushes".to_owned(),
            "pull_request" => "pull requests".to_owned(),
            other => other.replace('_', " ") + "s",
        })
        .collect();
    let mut s = words.join(", ");
    if let Some(f) = s.get_mut(0..1) {
        f.make_ascii_uppercase();
    }
    s
}

const HOOK_EVENTS: [(&str, &str); 3] =
    [("pushes", "Pushes"), ("pushes-and-pull-requests", "Pushes and pull requests"), ("everything", "Everything")];

pub fn webhooks(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Reading::Hooks(list)) = m.forge(Page::Hooks) else {
        return super::waiting(s, t, What::Forge(Page::Hooks));
    };
    let can = m.admin();
    let mut rows: Vec<El> = list
        .iter()
        .map(|h| {
            let last =
                h.last.as_ref().map_or_else(|| "Not delivered yet".to_owned(), |l| format!("Last delivery: {l}"));
            let (id, active) = (h.id, h.active);
            let active_check = parts::check(t, &format!("active-{}", h.url), h.active, "Active", move |s: &mut App| {
                s.settings_do(forge(Write::HookActive { id, active: !active }));
            });
            parts::item("hook", &h.url)
                .child(parts::named(t, &h.url, &format!("{} · {last}", events_words(&h.events)), true))
                .child(if can { active_check } else { active_check.disabled(true).opacity(0.5) })
                .child(parts::button(
                    t,
                    &format!("ping-{}", h.url),
                    "Ping",
                    Variant::Secondary,
                    can,
                    move |s: &mut App| s.settings_do(forge(Write::PingHook(id))),
                ))
                .child(parts::icon_button(
                    t,
                    &format!("delete-hook-{}", h.url),
                    &icons::TRASH,
                    "Delete webhook",
                    can,
                    move |s: &mut App| {
                        s.settings_do(forge(Write::DeleteHook(id)));
                    },
                ))
        })
        .collect();
    if rows.is_empty() {
        rows.push(parts::note(t, "No webhooks yet."));
    }
    let d = &m.drafts;
    let opts: Vec<(&str, &str, bool)> =
        HOOK_EVENTS.iter().enumerate().map(|(i, (id, w))| (*id, *w, m.hook_events == i)).collect();
    rows.push(
        col()
            .w_full()
            .gap(8.0)
            .child(parts::controls(vec![
                row().grow(1.0).child(parts::field(
                    t,
                    "hook-url",
                    &d.hook_url,
                    "https://",
                    can,
                    false,
                    |s: &mut App| &mut s.settings.drafts.hook_url,
                    None,
                )),
                row().w(180.0).child(parts::field(
                    t,
                    "hook-secret",
                    &d.hook_secret,
                    "Its secret (optional)",
                    can,
                    true,
                    |s: &mut App| &mut s.settings.drafts.hook_secret,
                    None,
                )),
            ]))
            .child(parts::controls(vec![
                parts::choice(t, "hook-events", &opts, can, |s: &mut App, i| s.settings.hook_events = i),
                rune::el().grow(1.0),
                parts::button(
                    t,
                    "add-hook",
                    "Add webhook",
                    Variant::Primary,
                    can && !d.hook_url.text.trim().is_empty(),
                    add_hook,
                ),
            ])),
    );
    vec![parts::note(t, "GitHub tells these addresses when things happen here."), parts::list(t, rows)]
}

fn add_hook(s: &mut App) {
    let d = &mut s.settings.drafts;
    let (url, secret) = (d.hook_url.text.trim().to_owned(), d.hook_secret.text.clone());
    (d.hook_url, d.hook_secret) = (TextEdit::new(""), TextEdit::new(""));
    let events = match s.settings.hook_events {
        0 => vec!["push".to_owned()],
        1 => vec!["push".to_owned(), "pull_request".to_owned()],
        _ => vec!["*".to_owned()],
    };
    s.settings_do(forge(Write::AddHook(NewHook { url, secret, events })));
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
