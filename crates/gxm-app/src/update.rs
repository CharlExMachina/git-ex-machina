//! Updating the app, as a machine: asked once at startup whether there's a newer release (and
//! again whenever the person asks), which the sidebar's foot then offers; downloaded and put in
//! place off the UI thread when the person takes it; then a sheet offering to restart into it,
//! with the release's notes. The network,
//! the file swap and starting the new version all run off the UI thread.
//!
//! Only an AppImage or the Mac app updates itself (see `gxm_core::update`); anywhere else the
//! check finds nothing, and nothing is shown.

use std::sync::{Arc, Mutex};

use gxm_core::update::{self, Release};
use rune::{Latest, Tx, Waker};

use crate::sheet::Sheet;
use crate::state::App;

/// Where an update is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Update {
    /// Nothing found yet: not asked, or the start's check found nothing or couldn't ask. The
    /// corner offers to check.
    Idle,
    /// Asking GitHub. `manual` when the person asked, so the answer is said whatever it is.
    Checking {
        manual: bool,
    },
    /// The person's check found nothing newer.
    UpToDate,
    /// The person's check couldn't ask GitHub.
    CheckFailed(String),
    Available(Release),
    Downloading {
        release: Release,
        done: u64,
        total: u64,
    },
    /// The download failed; taking the update again starts it over.
    Failed {
        release: Release,
        why: String,
    },
    /// Downloaded and in place: the next start runs it. Restarting runs it now.
    Ready(Release),
    /// Starting the new version, then quitting.
    Restarting(Release),
}

impl Update {
    /// The release the update is about, once there is one.
    pub fn release(&self) -> Option<&Release> {
        match self {
            Update::Idle | Update::Checking { .. } | Update::UpToDate | Update::CheckFailed(_) => None,
            Update::Available(r)
            | Update::Downloading { release: r, .. }
            | Update::Failed { release: r, .. }
            | Update::Ready(r)
            | Update::Restarting(r) => Some(r),
        }
    }
}

pub enum UpdateEvent {
    /// The window is up: ask whether there's a newer release.
    Boot,
    /// The person asks whether there's a newer release.
    Check,
    Checked(Result<Option<Release>, String>),
    /// The sidebar's update button: download, try again, or offer the restart again.
    Take,
    Progress(u64, u64),
    Installed(Result<(), String>),
    /// The restart sheet's Restart.
    Restart,
    Restarted(Result<(), String>),
}

#[derive(Debug, PartialEq, Eq)]
enum Effect {
    Check,
    Install(Release),
    OfferRestart,
    Restart,
    Quit,
    /// Why starting the new version failed, on the restart sheet.
    Refused(String),
}

fn step(update: &mut Update, event: UpdateEvent) -> Vec<Effect> {
    match (&*update, event) {
        (Update::Idle, UpdateEvent::Boot) => {
            *update = Update::Checking { manual: false };
            vec![Effect::Check]
        }
        (Update::Idle | Update::UpToDate | Update::CheckFailed(_), UpdateEvent::Check) => {
            *update = Update::Checking { manual: true };
            vec![Effect::Check]
        }
        (Update::Checking { .. }, UpdateEvent::Checked(Ok(Some(release)))) => {
            *update = Update::Available(release);
            vec![]
        }
        // The start's own check says nothing when it finds nothing, or can't ask.
        (Update::Checking { manual: false }, UpdateEvent::Checked(_)) => {
            *update = Update::Idle;
            vec![]
        }
        (Update::Checking { manual: true }, UpdateEvent::Checked(Ok(None))) => {
            *update = Update::UpToDate;
            vec![]
        }
        (Update::Checking { manual: true }, UpdateEvent::Checked(Err(why))) => {
            *update = Update::CheckFailed(why);
            vec![]
        }
        (Update::Available(r) | Update::Failed { release: r, .. }, UpdateEvent::Take) => {
            let release = r.clone();
            *update = Update::Downloading { total: release.asset.size, release: release.clone(), done: 0 };
            vec![Effect::Install(release)]
        }
        (Update::Ready(_), UpdateEvent::Take) => vec![Effect::OfferRestart],
        (Update::Downloading { release, .. }, UpdateEvent::Progress(done, total)) => {
            *update = Update::Downloading { release: release.clone(), done, total };
            vec![]
        }
        (Update::Downloading { release, .. }, UpdateEvent::Installed(Ok(()))) => {
            *update = Update::Ready(release.clone());
            vec![Effect::OfferRestart]
        }
        (Update::Downloading { release, .. }, UpdateEvent::Installed(Err(why))) => {
            *update = Update::Failed { release: release.clone(), why };
            vec![]
        }
        (Update::Ready(r), UpdateEvent::Restart) => {
            *update = Update::Restarting(r.clone());
            vec![Effect::Restart]
        }
        (Update::Restarting(_), UpdateEvent::Restarted(Ok(()))) => vec![Effect::Quit],
        (Update::Restarting(r), UpdateEvent::Restarted(Err(why))) => {
            *update = Update::Ready(r.clone());
            vec![Effect::Refused(why)]
        }
        _ => vec![],
    }
}

/// The background work: the check, the download and swap, starting the new version.
#[derive(Default)]
pub struct UpdateJobs {
    check: Latest<Result<Option<Release>, String>>,
    install: Latest<Result<(), String>>,
    restart: Latest<Result<(), String>>,
    /// The download's bytes so far and total, as it runs.
    progress: Arc<Mutex<Option<(u64, u64)>>>,
    waker: Option<Waker>,
}

impl UpdateJobs {
    pub fn set_waker(&mut self, waker: Waker) {
        self.check.set_waker(waker.clone());
        self.install.set_waker(waker.clone());
        self.restart.set_waker(waker.clone());
        self.waker = Some(waker);
    }

    /// Still asking whether there's an update. (A download is followed on screen instead.)
    pub fn checking(&self) -> bool {
        self.check.is_pending()
    }
}

impl App {
    pub fn update_event(&mut self, event: UpdateEvent, tx: Option<&mut Tx>) {
        let mut tx = tx;
        for effect in step(&mut self.update, event) {
            match effect {
                Effect::Check => {
                    self.update_jobs
                        .check
                        .request(self.env.clone(), |env| update::check(&env).map_err(|e| sentence(&e.to_string())));
                }
                Effect::Install(release) => self.install_update(release),
                Effect::OfferRestart => {
                    // Not over a sheet the person is using: the sidebar offers it instead.
                    if matches!(self.sheet, Sheet::Closed | Sheet::Update { .. }) {
                        self.sheet = Sheet::Update { error: None };
                    }
                }
                Effect::Restart => {
                    if let Sheet::Update { error } = &mut self.sheet {
                        *error = None;
                    }
                    self.update_jobs
                        .restart
                        .request(self.env.clone(), |env| update::restart(&env).map_err(|e| sentence(&e.to_string())));
                }
                Effect::Quit => {
                    if let Some(tx) = tx.as_deref_mut() {
                        tx.quit();
                    }
                }
                Effect::Refused(why) => self.sheet = Sheet::Update { error: Some(why) },
            }
        }
    }

    fn install_update(&mut self, release: Release) {
        if let Ok(mut p) = self.update_jobs.progress.lock() {
            *p = None;
        }
        let shared = (self.update_jobs.progress.clone(), self.update_jobs.waker.clone());
        self.update_jobs.install.request((self.env.clone(), release, shared), |(env, release, (progress, waker))| {
            let mut heard = |done: u64, total: u64| {
                if let Ok(mut p) = progress.lock() {
                    *p = Some((done, total));
                }
                if let Some(wake) = &waker {
                    wake();
                }
            };
            update::install(&env, &release, &mut heard).map_err(|e| sentence(&e.to_string()))
        });
    }

    /// Takes whatever the update's jobs have answered. True when the state changed.
    pub fn land_update(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(checked) = self.update_jobs.check.take() {
            self.update_event(UpdateEvent::Checked(checked), Some(tx));
            changed = true;
        }
        let progress = self.update_jobs.progress.lock().ok().and_then(|mut p| p.take());
        if let Some((done, total)) = progress {
            self.update_event(UpdateEvent::Progress(done, total), Some(tx));
            changed = true;
        }
        if let Some(installed) = self.update_jobs.install.take() {
            self.update_event(UpdateEvent::Installed(installed), Some(tx));
            changed = true;
        }
        if let Some(restarted) = self.update_jobs.restart.take() {
            self.update_event(UpdateEvent::Restarted(restarted), Some(tx));
            changed = true;
        }
        changed
    }
}

/// `couldn't replace …` → `Couldn't replace …`.
fn sentence(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// A line of release notes, as the restart sheet shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    Heading(String),
    Bullet(String),
    Text(String),
}

/// Release notes (Markdown, as written on GitHub) as headings, bullets and paragraphs, with
/// inline marks (`**`, backticks, links) reduced to their words.
pub fn notes(markdown: &str) -> Vec<Note> {
    let mut out: Vec<Note> = Vec::new();
    // A blank line ends the paragraph or bullet above it.
    let mut open = false;
    for line in markdown.lines() {
        let line = line.trim();
        if line.is_empty() || line.chars().all(|c| matches!(c, '-' | '*' | '_' | '=')) {
            open = false;
            continue;
        }
        if let Some(h) = line.strip_prefix('#') {
            out.push(Note::Heading(plain(h.trim_start_matches('#').trim())));
            open = false;
            continue;
        }
        if let Some(b) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            out.push(Note::Bullet(plain(b.trim())));
        } else if let (true, Some(Note::Text(t) | Note::Bullet(t))) = (open, out.last_mut()) {
            // A wrapped line carries on the paragraph or bullet above it.
            t.push(' ');
            t.push_str(&plain(line));
        } else {
            out.push(Note::Text(plain(line)));
        }
        open = true;
    }
    out
}

/// `**Faster** [history](https://…) in `git log`` → `Faster history in git log`.
fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find("](").map(|i| open + i) else { break };
        let Some(end) = rest[close..].find(')').map(|i| close + i) else { break };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..close]);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out.replace("**", "").replace("__", "").replace('`', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(v: &str) -> Release {
        Release {
            version: v.to_owned(),
            notes: String::new(),
            page: String::new(),
            asset: update::Asset { url: String::new(), size: 100, sha256: String::new() },
        }
    }

    #[test]
    fn an_update_is_offered_downloaded_then_restarted_into() {
        let mut u = Update::Idle;
        assert_eq!(step(&mut u, UpdateEvent::Boot), vec![Effect::Check]);
        assert!(step(&mut u, UpdateEvent::Checked(Err("offline".into()))).is_empty());
        assert_eq!(u, Update::Idle, "the start's failed check says nothing");
        assert_eq!(step(&mut u, UpdateEvent::Check), vec![Effect::Check]);
        step(&mut u, UpdateEvent::Checked(Err("Offline".into())));
        assert_eq!(u, Update::CheckFailed("Offline".into()), "a check the person asked for says why it failed");
        step(&mut u, UpdateEvent::Check);
        step(&mut u, UpdateEvent::Checked(Ok(None)));
        assert_eq!(u, Update::UpToDate);
        assert_eq!(step(&mut u, UpdateEvent::Check), vec![Effect::Check], "and can ask again");
        step(&mut u, UpdateEvent::Checked(Ok(Some(release("0.2.0")))));
        assert_eq!(u, Update::Available(release("0.2.0")));
        assert_eq!(step(&mut u, UpdateEvent::Take), vec![Effect::Install(release("0.2.0"))]);
        assert!(step(&mut u, UpdateEvent::Take).is_empty(), "one download at a time");
        step(&mut u, UpdateEvent::Progress(40, 100));
        assert!(matches!(u, Update::Downloading { done: 40, total: 100, .. }));
        step(&mut u, UpdateEvent::Installed(Err("Damaged".into())));
        assert!(matches!(u, Update::Failed { .. }));
        assert_eq!(step(&mut u, UpdateEvent::Take), vec![Effect::Install(release("0.2.0"))], "tries again");
        assert_eq!(step(&mut u, UpdateEvent::Installed(Ok(()))), vec![Effect::OfferRestart]);
        assert_eq!(step(&mut u, UpdateEvent::Take), vec![Effect::OfferRestart], "Later, then the button again");
        assert_eq!(step(&mut u, UpdateEvent::Restart), vec![Effect::Restart]);
        assert_eq!(step(&mut u, UpdateEvent::Restarted(Err("No".into()))), vec![Effect::Refused("No".into())]);
        assert_eq!(u, Update::Ready(release("0.2.0")));
        step(&mut u, UpdateEvent::Restart);
        assert_eq!(step(&mut u, UpdateEvent::Restarted(Ok(()))), vec![Effect::Quit]);
    }

    #[test]
    fn release_notes_read_as_headings_bullets_and_paragraphs() {
        let md = "## What's new\r\n\r\n- **Faster** history\n- Clone over [SSH](https://x.y/ssh) with `gxm clone`\n  across lines\n\nThanks to everyone.\n---\n# Fixes\n* A crash";
        assert_eq!(
            notes(md),
            vec![
                Note::Heading("What's new".into()),
                Note::Bullet("Faster history".into()),
                Note::Bullet("Clone over SSH with gxm clone across lines".into()),
                Note::Text("Thanks to everyone.".into()),
                Note::Heading("Fixes".into()),
                Note::Bullet("A crash".into()),
            ]
        );
    }
}
