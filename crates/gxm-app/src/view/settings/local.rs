//! What lives on this computer: the repository's remotes and how it pulls and pushes, who its
//! commits say made them and how they're signed, and which files go to Git LFS.

use gxm_core::local::{Lfs, LocalChange, PullMode, RemoteChange, RemoteEntry, Signing};
use gxm_core::remote::{Protocol, Remote};
use rune::{El, TextEdit, col, row};

use super::parts;
use crate::icons;
use crate::settings::{Action, Found, What};
use crate::state::App;
use crate::theme::Theme;
use crate::ui::Variant;

pub fn remotes(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Found::Remotes(list)) = m.found.get(&What::Remotes) else { return super::waiting(s, t, What::Remotes) };
    let mut rows: Vec<El> = list.iter().map(|r| remote_row(t, r)).collect();
    let d = &m.drafts;
    let ready = !d.remote_name.text.trim().is_empty() && !d.remote_url.text.trim().is_empty();
    rows.push(parts::controls(vec![
        row().w(120.0).child(parts::field(
            t,
            "remote-name",
            &d.remote_name,
            "upstream",
            true,
            false,
            |s: &mut App| &mut s.settings.drafts.remote_name,
            None,
        )),
        row().grow(1.0).child(parts::field(
            t,
            "remote-url",
            &d.remote_url,
            "git@github.com:owner/name.git",
            true,
            false,
            |s: &mut App| &mut s.settings.drafts.remote_url,
            parts::on(add_remote),
        )),
        parts::button(t, "add-remote", "Add remote", Variant::Secondary, ready, add_remote),
    ]));
    let mut out = vec![parts::group(t, "Where it fetches and pushes", vec![parts::list(t, rows)])];
    if let Some(l) = m.local() {
        let opts: Vec<(&str, &str, bool)> =
            [("rebase", PullMode::Rebase), ("merge", PullMode::Merge), ("refuse", PullMode::FastForward)]
                .iter()
                .map(|(id, p)| (*id, p.word(), l.pull == *p))
                .collect();
        out.push(parts::group(
            t,
            "When pulling",
            vec![
                parts::note(
                    t,
                    "Your commits that aren't pushed yet, when someone else pushed first (git’s pull.rebase):",
                ),
                parts::choice(t, "pull", &opts, true, |s: &mut App, i| {
                    s.settings_do(Action::Local(LocalChange::Pull(PullMode::ALL[i])));
                }),
            ],
        ));
        out.push(parts::group(
            t,
            "When pushing",
            vec![
                parts::toggle(
                    t,
                    "auto-setup-remote",
                    l.auto_setup_remote,
                    "A new branch's first push sets up where it pushes",
                    Some("git’s push.autoSetupRemote"),
                    None,
                    true,
                    |v| Action::Local(LocalChange::AutoSetupRemote(v)),
                ),
                parts::toggle(
                    t,
                    "follow-tags",
                    l.follow_tags,
                    "Tags go with the commits they point at",
                    Some("git’s push.followTags"),
                    None,
                    true,
                    |v| Action::Local(LocalChange::FollowTags(v)),
                ),
            ],
        ));
    }
    out
}

fn remote_row(t: &Theme, r: &RemoteEntry) -> El {
    let mut item = parts::item("remote", &r.name).child(parts::named(t, &r.name, &r.url, false));
    // SSH is the default; HTTPS is there for whoever opts in.
    if let Some(parsed) = Remote::parse(&r.url).filter(|p| p.protocol != Protocol::File) {
        let (ssh, https) = (parsed.ssh_url(), parsed.https_url());
        let name = r.name.clone();
        let opts =
            [("ssh", "SSH", parsed.protocol == Protocol::Ssh), ("https", "HTTPS", parsed.protocol == Protocol::Https)];
        item = item.child(parts::choice(t, &r.name, &opts, true, move |s: &mut App, i| {
            let url = if i == 0 { ssh.clone() } else { https.clone() };
            s.settings_do(Action::Remote(RemoteChange::SetUrl { name: name.clone(), url }));
        }));
    }
    let (name, fetch_only) = (r.name.clone(), r.fetch_only);
    item = item.child(parts::check(
        t,
        &format!("{}-fetch-only", r.name),
        r.fetch_only,
        "Fetch only",
        move |s: &mut App| {
            s.settings_do(Action::Remote(RemoteChange::FetchOnly { name: name.clone(), on: !fetch_only }));
        },
    ));
    let gone = r.name.clone();
    item.child(parts::icon_button(
        t,
        &format!("remove-remote-{}", r.name),
        &icons::TRASH,
        &format!("Remove {}", r.name),
        r.name != "origin",
        move |s: &mut App| {
            s.settings_do(Action::Remote(RemoteChange::Remove(gone.clone())));
        },
    ))
}

fn add_remote(s: &mut App) {
    let d = &mut s.settings.drafts;
    let (name, url) = (d.remote_name.text.trim().to_owned(), d.remote_url.text.trim().to_owned());
    if name.is_empty() || url.is_empty() {
        return;
    }
    (d.remote_name, d.remote_url) = (TextEdit::new(""), TextEdit::new(""));
    s.settings_do(Action::Remote(RemoteChange::Add { name, url }));
}

pub fn identity(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(l) = m.local() else { return super::waiting(s, t, What::Local) };
    let id = &l.identity;
    let d = &m.drafts;
    let changed = d.user_name.text.trim() != id.name || d.user_email.text.trim() != id.email;
    let own = id.own_name || id.own_email;
    let mut who = vec![
        parts::labelled(
            t,
            "Name",
            parts::field(
                t,
                "user-name",
                &d.user_name,
                "Your name",
                true,
                false,
                |s: &mut App| &mut s.settings.drafts.user_name,
                parts::on(save_identity),
            ),
        ),
        parts::labelled(
            t,
            "Email",
            parts::field(
                t,
                "user-email",
                &d.user_email,
                "you@example.com",
                true,
                false,
                |s: &mut App| &mut s.settings.drafts.user_email,
                parts::on(save_identity),
            ),
        ),
        row().pl(108.0).child(parts::note(
            t,
            if own {
                "Set for this repository: other repositories keep your ~/.gitconfig’s."
            } else {
                "From your ~/.gitconfig, as in every repository."
            },
        )),
    ];
    let mut buttons = vec![parts::button(t, "save-identity", "Save", Variant::Primary, changed, save_identity)];
    if own {
        buttons.push(parts::button(
            t,
            "inherit-identity",
            "Use my ~/.gitconfig’s",
            Variant::Ghost,
            true,
            |s: &mut App| {
                let d = &mut s.settings.drafts;
                (d.user_name, d.user_email) = (TextEdit::new(""), TextEdit::new(""));
                s.settings_do(Action::Local(LocalChange::Name(None)));
                s.settings_do(Action::Local(LocalChange::Email(None)));
            },
        ));
    }
    who.push(row().pl(108.0).child(parts::controls(buttons)));
    let kind = |x: &Signing| match x {
        Signing::Off => 0,
        Signing::Ssh(_) => 1,
        Signing::Gpg(_) => 2,
    };
    let shown = if m.signing == Signing::Off { kind(&id.signing) } else { kind(&m.signing) };
    let opts = [("off", "Off", shown == 0), ("ssh", "SSH key", shown == 1), ("gpg", "GPG key", shown == 2)];
    let mut signing = vec![parts::choice(t, "signing", &opts, true, |s: &mut App, i| match i {
        0 => {
            s.settings.signing = Signing::Off;
            s.settings_do(Action::Local(LocalChange::Signing(Signing::Off)));
        }
        1 => s.settings.signing = Signing::Ssh(String::new()),
        _ => s.settings.signing = Signing::Gpg(String::new()),
    })];
    if shown != 0 {
        let placeholder = if shown == 1 { "~/.ssh/id_ed25519.pub" } else { "Its key id" };
        signing.push(parts::controls(vec![
            row().grow(1.0).child(parts::field(
                t,
                "signing-key",
                &d.signing_key,
                placeholder,
                true,
                false,
                |s: &mut App| &mut s.settings.drafts.signing_key,
                parts::on(save_signing),
            )),
            parts::button(
                t,
                "save-signing",
                "Save",
                Variant::Secondary,
                !d.signing_key.text.trim().is_empty(),
                save_signing,
            ),
        ]));
        signing
            .push(parts::note(t, "Commits made here are signed with it, and show as verified where the key is known."));
    }
    vec![parts::group(t, "Who commits", who), parts::group(t, "Signing commits", signing)]
}

fn save_identity(s: &mut App) {
    let Some(id) = s.settings.local().map(|l| l.identity.clone()) else { return };
    let d = &s.settings.drafts;
    let (name, email) = (d.user_name.text.trim().to_owned(), d.user_email.text.trim().to_owned());
    if !name.is_empty() && name != id.name {
        s.settings_do(Action::Local(LocalChange::Name(Some(name))));
    }
    if !email.is_empty() && email != id.email {
        s.settings_do(Action::Local(LocalChange::Email(Some(email))));
    }
}

fn save_signing(s: &mut App) {
    let key = s.settings.drafts.signing_key.text.trim().to_owned();
    if key.is_empty() {
        return;
    }
    let signing = match (&s.settings.signing, s.settings.local().map(|l| &l.identity.signing)) {
        (Signing::Gpg(_), _) | (Signing::Off, Some(Signing::Gpg(_))) => Signing::Gpg(key),
        _ => Signing::Ssh(key),
    };
    s.settings.signing = Signing::Off;
    s.settings_do(Action::Local(LocalChange::Signing(signing)));
}

pub fn large_files(s: &App, t: &Theme) -> Vec<El> {
    let m = &s.settings;
    let Some(Found::Lfs(Lfs { installed, patterns })) = m.found.get(&What::Lfs) else {
        return super::waiting(s, t, What::Lfs);
    };
    let mut rows: Vec<El> = patterns
        .iter()
        .map(|p| {
            let gone = p.clone();
            parts::item("lfs", p).child(parts::named(t, p, "Goes to Git LFS", true)).child(parts::icon_button(
                t,
                &format!("untrack-{p}"),
                &icons::X,
                &format!("Stop sending {p} to Git LFS"),
                true,
                move |s: &mut App| s.settings_do(Action::Untrack(gone.clone())),
            ))
        })
        .collect();
    if rows.is_empty() {
        rows.push(parts::note(t, "Nothing goes to Git LFS yet."));
    }
    let typed = !m.drafts.lfs_pattern.text.trim().is_empty();
    rows.push(parts::controls(vec![
        row().grow(1.0).child(parts::field(
            t,
            "lfs-pattern",
            &m.drafts.lfs_pattern,
            "*.psd",
            true,
            false,
            |s: &mut App| &mut s.settings.drafts.lfs_pattern,
            parts::on(track),
        )),
        parts::button(t, "track", "Track", Variant::Secondary, typed, track),
    ]));
    let about = if *installed {
        "Git LFS is installed: files matching these go to its storage when you push. The patterns live in .gitattributes, which you commit."
    } else {
        "Git LFS isn't installed: files matching these stay ordinary files until it is (brew install git-lfs, or your package manager’s git-lfs)."
    };
    vec![col().w_full().gap(12.0).child(parts::note(t, about)).child(parts::list(t, rows))]
}

fn track(s: &mut App) {
    let pattern = s.settings.drafts.lfs_pattern.text.trim().to_owned();
    if pattern.is_empty() {
        return;
    }
    s.settings.drafts.lfs_pattern = TextEdit::new("");
    s.settings_do(Action::Track(pattern));
}
