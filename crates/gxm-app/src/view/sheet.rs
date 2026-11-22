//! A sheet over the window (Add, Init, Clone, Account): a 460-wide panel raised over a dimmed window, with a
//! title, what it's for, its fields, what went wrong, and Cancel beside the primary button.

use rune::{El, col, el, row, text};

use crate::keys;
use crate::sheet::{self, Event, Phase, Sheet};
use crate::state::App;
use crate::theme::{Theme, size, ty};
use crate::ui::{self, Variant};

/// The scrim and the panel, laid over the window; nothing while no sheet is open.
pub fn layer(s: &App, t: &Theme) -> Vec<El> {
    let f = &s.fields;
    let (title, about, fields, phase, confirm) = match &s.sheet {
        Sheet::Closed => return Vec::new(),
        Sheet::Account(state) => {
            let (title, about, body, footer) = super::account::sheet(t, state);
            return panel(t, title, about, body, footer);
        }
        Sheet::Add(phase) => (
            "Add a repository".to_owned(),
            "A repository already on this computer: its folder, or any folder inside it.".to_owned(),
            vec![ui::field(
                t,
                &f.path,
                "Folder",
                "~/code/project",
                keys::ADD_PATH,
                |s: &mut App| &mut s.fields.path,
                submit,
            )],
            phase,
            ("Add", keys::ADD_CONFIRM),
        ),
        Sheet::Init { offer, phase } => {
            let folder = offer.folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            (
                format!("Make {folder} a repository"),
                format!(
                    "{} has no git repository yet. This starts one right there: your files stay where they are, uncommitted, for you to look over and commit.",
                    s.env.tilde(&offer.folder)
                ),
                init_fields(s, t, offer),
                phase,
                ("Create repository", keys::INIT_CONFIRM),
            )
        }
        Sheet::Clone(phase) => (
            "Clone a repository".to_owned(),
            "Paste its URL from GitHub, GitLab or Bitbucket, or type owner/name for GitHub. It clones over SSH."
                .to_owned(),
            vec![
                ui::field(
                    t,
                    &f.url,
                    "Repository",
                    "git@github.com:owner/name.git",
                    keys::CLONE_URL,
                    |s: &mut App| &mut s.fields.url,
                    submit,
                ),
                ui::field(t, &f.into, "Into", "~/code", keys::CLONE_INTO, |s: &mut App| &mut s.fields.into, submit),
                plan(s, t, phase),
            ],
            phase,
            ("Clone", keys::CLONE_CONFIRM),
        ),
    };
    let mut body = fields;
    if let Phase::Editing { error: Some(e) } = phase {
        body.push(error(t, e));
    }
    panel(t, title, about, body, buttons(t, phase, confirm))
}

/// What went wrong, in the sheet.
pub(super) fn error(t: &Theme, e: &str) -> El {
    text(e.to_owned()).key(keys::SHEET_ERROR).font(ty::CAPTION).color(t.removed).wrap()
}

/// The panel and its scrim: title, what it's for, the body, and the buttons at the foot.
fn panel(t: &Theme, title: String, about: String, body: Vec<El>, footer: El) -> Vec<El> {
    let mut panel = col()
        .key(keys::SHEET)
        .role("dialog")
        .label(title.clone())
        .w(size::SHEET)
        .padding(20.0)
        .gap(16.0)
        .radius(size::RADIUS_LG)
        .fill(t.raised)
        .modal(true, |s: &mut App| s.sheet_event(Event::Dismiss, None))
        .child(
            col()
                .w_full()
                .gap(4.0)
                .child(text(title).font(ty::TITLE).color(t.ink))
                .child(text(about).font(ty::CAPTION).color(t.ink_muted).wrap()),
        )
        .children(body);
    for sh in t.shadow_overlay() {
        panel = panel.box_shadow(sh);
    }
    panel = panel.child(footer);
    vec![
        el().key("sheet-scrim").absolute(0.0, 0.0).w_full().h_full().fill(t.scrim).a11y_hidden(),
        el().key("sheet-layer").absolute(0.0, 0.0).w_full().h_full().padding(16.0).center().child(panel),
    ]
}

/// Return in a field does what the primary button does.
fn submit(field: El) -> El {
    field.on_submit(|s: &mut App, tx| s.sheet_event(Event::Submit, Some(tx)))
}

/// Cancel (it stops a clone that's running), then the primary button, held down while it works.
fn buttons(t: &Theme, phase: &Phase, (label, key): (&str, &str)) -> El {
    let working = !phase.editing();
    let cancel = ui::button(t, Variant::Secondary, None, "Cancel", None).key(keys::SHEET_CANCEL);
    let cancel = match phase {
        Phase::Cancelling => cancel.disabled(true).opacity(0.6),
        _ => cancel.on_press(|s: &mut App| s.sheet_event(Event::Dismiss, None)),
    };
    let confirm = ui::button_in(t, Variant::Primary, None, label, None, working).key(key);
    let confirm = if working {
        confirm.disabled(true)
    } else {
        confirm.on_press_tx(|s: &mut App, tx| s.sheet_event(Event::Submit, Some(tx)))
    };
    row().w_full().gap(8.0).justify_end().child(cancel).child(confirm)
}

/// Where the clone comes from and where it lands, as the fields stand; git's progress while it
/// runs.
fn plan(s: &App, t: &Theme, phase: &Phase) -> El {
    let line = |label: &str, value: String, ink| {
        row()
            .w_full()
            .gap(8.0)
            .child(text(label.to_owned()).font(ty::CAPTION).color(t.ink_faint).w(36.0))
            .child(text(value).font(ty::MONO).color(ink).truncate().grow(1.0).min_w(0.0))
    };
    let mut box_ = col().key(keys::CLONE_PLAN).w_full().gap(2.0).padding(10.0).radius(size::RADIUS_SM).fill(t.canvas);
    match sheet::plan(&s.env, &s.fields) {
        Some((source, dest)) => {
            box_ = box_.child(line("From", source.url, t.ink)).child(line("To", s.env.tilde(&dest), t.ink));
        }
        None => {
            box_ = box_.child(text("Where it clones from and to shows here").font(ty::CAPTION).color(t.ink_faint));
        }
    }
    if !phase.editing() {
        let said = match (phase, &s.sheet_jobs.shown) {
            (Phase::Cancelling, _) => "Cancelling…".to_owned(),
            (_, Some(progress)) => progress.clone(),
            (_, None) => "Connecting…".to_owned(),
        };
        box_ = box_.child(line("", said, t.ink_muted).mt(4.0));
    }
    box_
}

/// Init's fields: the name, where the repository lives, the README and the ignore list. A
/// README or `.gitignore` already in the folder is kept, and says so.
fn init_fields(s: &App, t: &Theme, offer: &sheet::Offer) -> Vec<El> {
    let f = &s.fields;
    let name = ui::field(
        t,
        &f.name,
        "Name",
        "my-project",
        keys::INIT_NAME,
        |s: &mut App| &mut s.fields.name,
        |e| submit(e).on_input_change(|s: &mut App, _| s.sheet_event(Event::NameEdited, None)),
    );
    let hint = text("The README's heading, and the repository's name on GitHub. The folder keeps its name.")
        .font(ty::HINT)
        .color(t.ink_faint)
        .wrap();
    let github = ui::segment_label(t, "On GitHub", false)
        .key(keys::INIT_GITHUB)
        .opacity(0.5)
        .disabled(true)
        .help("Coming next: sign in to GitHub to create it there too, public or private");
    let local = ui::segment_label(t, "Local only", true).key(keys::INIT_LOCAL);
    let place = col()
        .gap(6.0)
        .child(text("Where it lives").font(ty::CAPTION_STRONG).color(t.ink_muted))
        .child(row().child(ui::segmented(t, vec![(local, true), (github, false)])));
    let kept = |file: &str, key: &str| {
        text(format!("Keeps the {file} that's there")).key(key).font(ty::BODY).color(t.ink_muted)
    };
    let readme = if offer.survey.has_readme {
        kept("README.md", keys::INIT_README)
    } else {
        let mut readme = col().w_full().gap(8.0).child(
            ui::check(t, f.write_readme, "Add a README")
                .key(keys::INIT_README)
                .on_press(|s: &mut App| s.sheet_event(Event::ToggleReadme, None)),
        );
        if f.write_readme {
            let area = rune::text_input(&f.readme, ty::MONO)
                .key(keys::INIT_README_TEXT)
                .label("README".to_owned())
                .on_edit(|s: &mut App| &mut s.fields.readme)
                .on_input_change(|s: &mut App, _| s.sheet_event(Event::ReadmeEdited, None))
                .color(t.ink)
                .w_full()
                .min_h(54.0);
            let mut well =
                col().w_full().padding(10.0).radius(size::RADIUS_SM).fill(t.well).role("textbox").child(area);
            for sh in t.shadow_well() {
                well = well.box_shadow(sh);
            }
            readme = readme.child(well);
        }
        readme
    };
    let mut fields = vec![col().w_full().gap(6.0).child(name).child(hint), place, readme];
    match (offer.survey.kind, offer.survey.has_ignore) {
        (_, true) => fields.push(kept(".gitignore", keys::INIT_IGNORE)),
        (Some(kind), false) => fields.push(
            ui::check(t, f.write_ignore, &format!("Ignore {}'s generated files ({})", kind.name(), kind.ignores()))
                .key(keys::INIT_IGNORE)
                .on_press(|s: &mut App| s.sheet_event(Event::ToggleIgnore, None)),
        ),
        (None, false) => {}
    }
    if let Some(lfs) = lfs(t, s, offer) {
        fields.push(lfs);
    }
    fields
}

/// Git LFS for the binary files the folder has, naming their extensions; nothing when it has
/// none. Without `git-lfs` installed it says how to get it.
fn lfs(t: &Theme, s: &App, offer: &sheet::Offer) -> Option<El> {
    let binary = &offer.survey.binary;
    if binary.is_empty() {
        return None;
    }
    const SHOWN: usize = 5;
    let mut exts: Vec<String> = binary.iter().take(SHOWN).map(|e| format!(".{e}")).collect();
    if binary.len() > SHOWN {
        exts.push(format!("{} more", binary.len() - SHOWN));
    }
    let label = format!("Track binary files with Git LFS ({})", exts.join(", "));
    let check = ui::check(t, s.fields.use_lfs && offer.survey.lfs, &label).key(keys::INIT_LFS);
    Some(if offer.survey.lfs {
        check.on_press(|s: &mut App| s.sheet_event(Event::ToggleLfs, None))
    } else {
        check
            .disabled(true)
            .opacity(0.5)
            .help("Install Git LFS first: brew install git-lfs, or your package manager's git-lfs")
    })
}
