//! The tag window: tagging a commit in a window of its own, beside the main one, so the release
//! notes get the room they need and the window goes wherever the person wants it (another
//! screen, full height). The main window opens it on a commit ([`TagLink::open`]); the window
//! tags off the UI thread, then tells the main window what it made ([`TagLink::done`]) and hides.

use std::sync::{Arc, Mutex, OnceLock};

use gxm_core::attach;
use gxm_core::env::Env;
use gxm_core::history;
use gxm_core::remote::Remote;
use gxm_core::tag;
use rune::{AppSpec, Latest, TextEdit, Tx, Waker, WindowHandle};

use crate::keys;
use crate::ops::Ops;
use crate::tag::{Opening, TagEffect, TagEvent, TagFields, TagSheet};
use crate::theme;

/// The window's key among the app's windows (`--window tag` selects it headless).
pub const KEY: &str = "tag";
/// The window's size when it first opens: room for a changelog, on a 1440×900 screen.
pub const SIZE: (f32, f32) = (760.0, 800.0);
const MIN_SIZE: (f32, f32) = (560.0, 520.0);

/// Messages for a window that's drained when it polls: whatever posts wakes it.
pub struct Mailbox<T>(Arc<Mutex<(Vec<T>, Option<Waker>)>>);

impl<T> Clone for Mailbox<T> {
    fn clone(&self) -> Self {
        Mailbox(self.0.clone())
    }
}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Mailbox(Arc::new(Mutex::new((Vec::new(), None))))
    }
}

impl<T> Mailbox<T> {
    fn lock(&self) -> std::sync::MutexGuard<'_, (Vec<T>, Option<Waker>)> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn post(&self, mail: T) {
        let mut inbox = self.lock();
        inbox.0.push(mail);
        if let Some(wake) = &inbox.1 {
            wake();
        }
    }

    pub fn take(&self) -> Vec<T> {
        std::mem::take(&mut self.lock().0)
    }

    pub fn pending(&self) -> bool {
        !self.lock().0.is_empty()
    }

    /// Who reads this mailbox: posting wakes it.
    pub fn set_waker(&self, waker: Waker) {
        self.lock().1 = Some(waker);
    }
}

/// How the main window and the tag window reach each other.
#[derive(Clone, Default)]
pub struct TagLink {
    /// To the tag window: tag this commit.
    pub open: Mailbox<Box<Opening>>,
    /// To the main window: a tag was made, and what to say about it.
    pub done: Mailbox<String>,
    /// The native window, to show it, once the main window's app has added it; journeys,
    /// which have no native windows, follow [`TagWindow::shown`] instead.
    pub window: Arc<OnceLock<WindowHandle>>,
}

/// The refs containing the commit, the tag being made, GitHub's draft of its notes, and the
/// commit of their files, off the UI thread.
#[derive(Default)]
pub struct TagJobs {
    read: Latest<Result<Vec<history::RefLabel>, String>>,
    job: Latest<Result<Option<String>, String>>,
    draft: Latest<Result<String, String>>,
    files: Latest<Result<String, String>>,
}

impl TagJobs {
    fn set_waker(&self, waker: &Waker) {
        self.read.set_waker(waker.clone());
        self.job.set_waker(waker.clone());
        self.draft.set_waker(waker.clone());
        self.files.set_waker(waker.clone());
    }

    fn busy(&self) -> bool {
        self.read.is_pending() || self.job.is_pending() || self.draft.is_pending() || self.files.is_pending()
    }
}

pub struct TagWindow {
    pub env: Env,
    pub link: TagLink,
    /// The commit being tagged; None before the first, and once a tag is made or cancelled.
    pub sheet: Option<TagSheet>,
    pub tag_fields: TagFields,
    pub tag_jobs: TagJobs,
    /// The window is on screen.
    pub shown: bool,
    /// Where a field's edits land once what it edited is gone.
    pub scratch: TextEdit,
}

impl TagWindow {
    pub fn new(mut env: Env, link: TagLink) -> TagWindow {
        env.claim_ui_thread();
        TagWindow {
            env,
            link,
            sheet: None,
            tag_fields: TagFields::default(),
            tag_jobs: TagJobs::default(),
            shown: false,
            scratch: TextEdit::new(""),
        }
    }

    /// Something runs off the UI thread, or the main window has asked for something not yet
    /// taken.
    pub fn busy(&self) -> bool {
        self.tag_jobs.busy() || self.link.open.pending()
    }

    /// Feeds `event` to the machine and carries out what it asks for.
    pub fn tag_event(&mut self, event: TagEvent, tx: Option<&mut Tx>) {
        let mut tx = tx;
        for effect in crate::tag::step(&mut self.sheet, &mut self.tag_fields, event) {
            self.effect(effect, tx.as_deref_mut());
        }
    }

    fn effect(&mut self, effect: TagEffect, tx: Option<&mut Tx>) {
        let Some(sheet) = &self.sheet else {
            // Only these outlive the commit they were about.
            match effect {
                TagEffect::Close => self.close(tx),
                TagEffect::Done(said) => self.link.done.post(said),
                _ => {}
            }
            return;
        };
        let (env, repo) = (sheet.opening.env.clone(), sheet.opening.repo.clone());
        match effect {
            TagEffect::Show => {
                self.shown = true;
                if let Some(window) = self.link.window.get() {
                    window.open();
                }
            }
            TagEffect::Focus(key) => {
                if let Some(tx) = tx {
                    tx.focus(key, true);
                }
            }
            TagEffect::FocusNotes => {
                if let Some(tx) = tx {
                    tx.focus(keys::notes_part(self.tag_fields.notes.at), true);
                }
            }
            TagEffect::Read => {
                let commit = sheet.opening.commit.clone();
                self.tag_jobs.read.request((env, repo, commit), |(env, repo, commit)| {
                    history::info(&env, &repo, &commit).map(|i| i.contained_in).map_err(Ops::said)
                });
            }
            TagEffect::Draft(name) => {
                let (commit, origin) = (sheet.opening.commit.clone(), sheet.opening.origin.clone());
                let trigger = sheet.opening.tagging.trigger.clone();
                let input = (env, repo, origin, name, commit, trigger);
                self.tag_jobs.draft.request(input, |(env, repo, origin, name, commit, trigger)| {
                    let origin: Remote = origin
                        .ok_or_else(|| "This repository has no remote on GitHub to draft notes from.".to_owned())?;
                    let previous = tag::previous(&env, &repo, &commit, trigger.as_ref());
                    gxm_core::forge::notes::generate(&env, &origin, &name, &commit, previous.as_deref())
                });
            }
            TagEffect::CommitFiles(files) => {
                self.tag_jobs.files.request((env, repo, files), |(env, repo, files)| {
                    attach::files_commit(&env, &repo, &files).map_err(Ops::said)
                });
            }
            TagEffect::Create(new) => {
                self.tag_jobs
                    .job
                    .request((env, repo, new), |(env, repo, new)| tag::create(&env, &repo, &new).map_err(Ops::said));
            }
            TagEffect::CreateWithFiles { new, notes, files, origin } => {
                let input = (env, repo, new, notes, files, origin);
                self.tag_jobs.job.request(input, |(env, repo, mut new, notes, files, origin)| {
                    let commit = attach::files_commit(&env, &repo, &files).map_err(Ops::said)?;
                    let base = attach::files_base(&origin, &commit);
                    new.message = gxm_core::notes::markdown_with(&notes, base.as_deref());
                    new.files = Some(commit);
                    tag::create(&env, &repo, &new).map_err(Ops::said)
                });
            }
            TagEffect::Close => self.close(tx),
            TagEffect::Done(said) => self.link.done.post(said),
        }
    }

    /// Hides the window; its state stays, for the next commit to tag.
    fn close(&mut self, tx: Option<&mut Tx>) {
        self.shown = false;
        if let Some(tx) = tx {
            tx.close_window();
        }
    }

    /// Takes what the main window asked for and what finished off the UI thread. True when
    /// the state changed.
    fn land(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        for opening in self.link.open.take() {
            self.tag_event(TagEvent::Open(opening), Some(tx));
            changed = true;
        }
        if let Some(read) = self.tag_jobs.read.take() {
            self.tag_event(TagEvent::Read(read), None);
            changed = true;
        }
        if let Some(made) = self.tag_jobs.files.take() {
            self.tag_event(TagEvent::FilesCommitted(made), None);
            changed = true;
        }
        if let Some(drafted) = self.tag_jobs.draft.take() {
            self.tag_event(TagEvent::Drafted(drafted), Some(tx));
            changed = true;
        }
        if let Some(result) = self.tag_jobs.job.take() {
            self.tag_event(TagEvent::Landed(result), Some(tx));
            changed = true;
        }
        changed
    }
}

/// The tag window, as the main window's app adds it and as headless runs build it. `link`
/// reaches the main window.
pub fn app(link: TagLink) -> rune::App<TagWindow> {
    let init_link = link;
    rune::App::new(AppSpec {
        poll: Some(|s: &mut TagWindow, _, tx| s.land(tx)),
        setup: Some(|s: &mut TagWindow, waker| {
            s.tag_jobs.set_waker(&waker);
            s.link.open.set_waker(waker);
        }),
        min_size: Some(MIN_SIZE),
        fonts: theme::FONTS,
        families: (theme::SANS, theme::MONO),
        ..AppSpec::new(
            "Tag a release",
            SIZE,
            || TagWindow::new(Env::from_process(), TagLink::default()),
            crate::view::tag_window,
        )
    })
    .init(move || TagWindow::new(Env::from_process(), init_link))
    .chrome(crate::CHROME)
    .font_smoothing(crate::FONT_SMOOTHING)
}

/// The window, added to `main` hidden: it shows when the main window opens it on a commit.
/// Returns the link the main window's state keeps.
pub fn add_to<S: 'static>(main: &mut rune::App<S>) -> TagLink {
    let link = TagLink::default();
    let window = app(link.clone()).on_close(|s: &mut TagWindow, _| s.shown = false);
    let hidden = rune::WindowOptions { visible: false, min_size: Some(MIN_SIZE), ..Default::default() };
    let spec = rune::WindowSpec::from_app(KEY, window).options(hidden);
    let (_, handle) = main.window(spec);
    link.window.get_or_init(|| handle);
    link
}

impl crate::state::App {
    /// Opens the tag window on `commit`, the selected repository's.
    pub fn open_tag(&self, commit: &str) {
        let (Some(repo), Some(history), Some(c)) = (self.repo(), self.history.as_ref(), self.commit(commit)) else {
            return;
        };
        let mut people: Vec<(String, String)> = Vec::new();
        let viewer = self.pulls.viewer().map(|u| (u.login.clone(), u.name.clone()));
        let authors = self.pulls.all().iter().map(|p| (p.author.login.clone(), p.author.name.clone()));
        for (login, name) in viewer.into_iter().chain(authors) {
            if !login.is_empty() && !people.iter().any(|(l, _)| *l == login) {
                people.push((login, name));
            }
        }
        self.tag_link.open.post(Box::new(Opening {
            env: self.env.clone(),
            repo: repo.summary.path.clone(),
            commit: commit.to_owned(),
            short: c.short_id().to_owned(),
            subject: c.subject.clone(),
            tagging: history.tagging.clone(),
            origin: repo.summary.origin.clone(),
            people,
            pulls: self.pulls.all().iter().map(|p| (p.number, p.title.clone())).collect(),
        }));
    }

    /// What the tag window made: the history read again, and a toast saying so. True when one
    /// was.
    pub fn land_tagged(&mut self, tx: &mut Tx) -> bool {
        let made = self.tag_link.done.take();
        for said in &made {
            self.reload_repo();
            self.toasts.show(said, tx);
        }
        !made.is_empty()
    }
}
