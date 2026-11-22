//! Keeping the repository safe and linked: its security features, the deploy keys that reach
//! it, and the autolinks that turn references into links.

use gxm_core::forge::admin::{Page, Reading, SecurityFeature, Write};
use gxm_core::time;
use rune::{El, TextEdit, col, row};

use super::parts;
use crate::icons;
use crate::settings::{Action, What};
use crate::state::App;
use crate::theme::Theme;
use crate::ui::Variant;

fn forge(w: Write) -> Action {
    Action::Forge(w)
}

pub fn code_security(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Reading::Security(sec)) = m.forge(Page::Security) else {
        return super::waiting(s, t, What::Forge(Page::Security));
    };
    let can = m.admin();
    let feature = |id: &str, on: bool, title: &str, hint: &str, f: SecurityFeature, enabled: bool| {
        parts::toggle(t, id, on, title, Some(hint), None, can && enabled, move |v| forge(Write::Security(f, v)))
    };
    vec![
        parts::group(
            t,
            "Dependencies",
            vec![
                feature(
                    "dependabot-alerts",
                    sec.dependabot_alerts,
                    "Dependabot alerts",
                    "Word when a dependency has a known vulnerability.",
                    SecurityFeature::DependabotAlerts,
                    true,
                ),
                feature(
                    "dependabot-updates",
                    sec.dependabot_updates,
                    "Dependabot security updates",
                    "Pull requests that update a vulnerable dependency to a fixed version.",
                    SecurityFeature::DependabotUpdates,
                    sec.dependabot_alerts,
                ),
            ],
        ),
        parts::group(
            t,
            "Secrets in the code",
            vec![
                feature(
                    "secret-scanning",
                    sec.secret_scanning,
                    "Secret scanning",
                    "Word when a token or key is pushed.",
                    SecurityFeature::SecretScanning,
                    true,
                ),
                feature(
                    "push-protection",
                    sec.push_protection,
                    "Push protection",
                    "Refuses a push that holds a secret, before it lands.",
                    SecurityFeature::PushProtection,
                    sec.secret_scanning,
                ),
            ],
        ),
    ]
}

pub fn deploy_keys(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Reading::DeployKeys(keys)) = m.forge(Page::DeployKeys) else {
        return super::waiting(s, t, What::Forge(Page::DeployKeys));
    };
    let can = m.admin();
    let mut rows: Vec<El> = keys
        .iter()
        .map(|k| {
            let opening: String =
                k.key.split_whitespace().take(2).collect::<Vec<_>>().join(" ").chars().take(28).collect();
            let access = if k.read_only { "Read only" } else { "Read and write" };
            let id = k.id;
            parts::item("deploy-key", &k.title)
                .child(rune::icon(&icons::KEY, 14.0).color(t.ink_muted))
                .child(parts::named(
                    t,
                    &k.title,
                    &format!("{access} · {opening}… · added {}", time::long(k.added, s.env.now(), &s.env.tz)),
                    false,
                ))
                .child(parts::icon_button(
                    t,
                    &format!("delete-key-{}", k.title),
                    &icons::TRASH,
                    &format!("Delete {}", k.title),
                    can,
                    move |s: &mut App| {
                        s.settings_do(forge(Write::DeleteDeployKey(id)));
                    },
                ))
        })
        .collect();
    if rows.is_empty() {
        rows.push(parts::note(t, "No deploy keys yet."));
    }
    let d = &m.drafts;
    let ready = !d.key_title.text.trim().is_empty() && !d.key_value.text.trim().is_empty();
    let write = m.key_write;
    rows.push(
        col()
            .w_full()
            .gap(8.0)
            .child(parts::controls(vec![
                row().w(160.0).child(parts::field(
                    t,
                    "key-title",
                    &d.key_title,
                    "Its name",
                    can,
                    false,
                    |s: &mut App| &mut s.settings.drafts.key_title,
                    None,
                )),
                row().grow(1.0).child(parts::field(
                    t,
                    "key",
                    &d.key_value,
                    "ssh-ed25519 AAAA…",
                    can,
                    false,
                    |s: &mut App| &mut s.settings.drafts.key_value,
                    parts::on(add_key),
                )),
            ]))
            .child(parts::controls(vec![
                parts::check(t, "key-write", write, "It may push, not only clone", |s: &mut App| {
                    s.settings.key_write = !s.settings.key_write;
                }),
                rune::el().grow(1.0),
                parts::button(t, "add-key", "Add key", Variant::Primary, can && ready, add_key),
            ])),
    );
    vec![
        parts::note(t, "SSH keys that reach this repository alone: a server that deploys it, a bot that clones it."),
        parts::list(t, rows),
    ]
}

fn add_key(s: &mut App) {
    let m = &mut s.settings;
    let (title, key) = (m.drafts.key_title.text.trim().to_owned(), m.drafts.key_value.text.trim().to_owned());
    if title.is_empty() || key.is_empty() {
        return;
    }
    (m.drafts.key_title, m.drafts.key_value) = (TextEdit::new(""), TextEdit::new(""));
    let read_only = !m.key_write;
    m.key_write = false;
    s.settings_do(forge(Write::AddDeployKey { title, key, read_only }));
}

pub fn autolinks(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Reading::Autolinks(links)) = m.forge(Page::Autolinks) else {
        return super::waiting(s, t, What::Forge(Page::Autolinks));
    };
    let can = m.admin();
    let mut rows: Vec<El> = links
        .iter()
        .map(|a| {
            let id = a.id;
            parts::item("autolink", &a.prefix)
                .child(parts::named(t, &format!("{}123", a.prefix), &a.template.replace("<num>", "123"), true))
                .child(parts::icon_button(
                    t,
                    &format!("delete-autolink-{}", a.prefix),
                    &icons::TRASH,
                    &format!("Delete {}", a.prefix),
                    can,
                    move |s: &mut App| {
                        s.settings_do(forge(Write::DeleteAutolink(id)));
                    },
                ))
        })
        .collect();
    if rows.is_empty() {
        rows.push(parts::note(t, "No autolinks yet."));
    }
    let d = &m.drafts;
    let ready = !d.link_prefix.text.trim().is_empty() && d.link_template.text.contains("<num>");
    rows.push(parts::controls(vec![
        row().w(120.0).child(parts::field(
            t,
            "autolink-prefix",
            &d.link_prefix,
            "TICKET-",
            can,
            false,
            |s: &mut App| &mut s.settings.drafts.link_prefix,
            None,
        )),
        row().grow(1.0).child(parts::field(
            t,
            "autolink-template",
            &d.link_template,
            "https://tracker.example.com/<num>",
            can,
            false,
            |s: &mut App| &mut s.settings.drafts.link_template,
            parts::on(add_autolink),
        )),
        parts::button(t, "add-autolink", "Add autolink", Variant::Secondary, can && ready, add_autolink),
    ]));
    vec![
        parts::note(
            t,
            "A prefix in an issue, a commit or a pull request links to your tracker: <num> is where the number goes.",
        ),
        parts::list(t, rows),
    ]
}

fn add_autolink(s: &mut App) {
    let d = &mut s.settings.drafts;
    let (prefix, template) = (d.link_prefix.text.trim().to_owned(), d.link_template.text.trim().to_owned());
    if prefix.is_empty() || !template.contains("<num>") {
        return;
    }
    (d.link_prefix, d.link_template) = (TextEdit::new(""), TextEdit::new(""));
    s.settings_do(forge(Write::AddAutolink { prefix, template }));
}
