//! The tag sheet's machine: editing the name, message and whether to push, then tagging (and
//! pushing) off the UI thread, then closed with the history read again and a toast saying what
//! happened, or back to editing with git's reason.

use std::path::PathBuf;

use gxm_core::attach;
use gxm_core::notes::Notes;
use gxm_core::release::NotesFrom;
use gxm_core::remote::Remote;
use gxm_core::tag::{self, NewTag, Tagging};
use rune::{Latest, TextEdit, Tx, Waker};

use crate::keys;
use crate::notes::{Draft, NotesEditor};
use crate::ops::Ops;
use crate::sheet::{Phase, Sheet};
use crate::state::App;

/// The commit being tagged and what the repository's tags say about it.
pub struct TagSheet {
    /// The commit's full id, its short id and subject as the history shows them.
    pub commit: String,
    pub short: String,
    pub subject: String,
    pub tagging: Tagging,
    /// Where the repository lives on its provider: where its notes link attached files.
    pub origin: Option<Remote>,
    pub phase: Phase,
}

/// The sheet's fields.
pub struct TagFields {
    pub name: TextEdit,
    pub message: TextEdit,
    /// The release notes, where the release takes them from the tag (in place of the message).
    pub notes: NotesEditor,
    pub push: bool,
}

impl Default for TagFields {
    fn default() -> Self {
        TagFields { name: TextEdit::new(""), message: TextEdit::new(""), notes: NotesEditor::default(), push: false }
    }
}

impl TagSheet {
    /// Whether the release takes its notes from the tag, so the sheet writes them.
    pub fn writes_notes(&self) -> bool {
        self.tagging.trigger.as_ref().is_some_and(|t| t.notes == NotesFrom::Tag)
    }

    /// Where the notes link the files of `commit` (`gxm_core::attach`), when the repository is
    /// on GitHub.
    pub fn files_base(&self, commit: &str) -> Option<String> {
        self.origin.as_ref().and_then(|o| attach::files_base(o, commit))
    }
}

pub enum TagEvent {
    /// Tag this commit (its full id).
    Open(String),
    TogglePush,
    /// The name changed: what was wrong with the last one no longer is.
    NameEdited,
    /// Ask GitHub for its notes for this tag.
    Draft,
    /// GitHub's notes (markdown), or why they didn't come.
    Drafted(Result<String, String>),
    /// A file was attached to the notes: their files' commit, and so their links, change.
    FilesChanged,
    /// The commit holding the notes' files, or why it couldn't be made.
    FilesCommitted(Result<String, String>),
    Submit,
    /// The tag was made, and pushed to this remote when it says one; or git's reason it wasn't.
    Landed(Result<Option<String>, String>),
}

pub enum TagEffect {
    Focus(&'static str),
    /// The caret into the notes, where it last was.
    FocusNotes,
    /// GitHub's notes for the tag called this, on this commit.
    Draft {
        name: String,
        commit: String,
    },
    /// The commit holding these files (each one's name in the notes, and where it is).
    CommitFiles(Vec<(String, PathBuf)>),
    Create(NewTag),
    /// Commit `files`, link them from `notes` by that commit, and make (and push) the tag with
    /// them: the files go up with the tag.
    CreateWithFiles {
        new: NewTag,
        notes: Notes,
        files: Vec<(String, PathBuf)>,
        origin: Remote,
    },
    /// The tag is made: read the history again and say so.
    Done(String),
}

/// One transition. `tagging` and `commit` are the selected repository's and commit's, for Open.
pub fn step(
    sheet: &mut Sheet,
    fields: &mut TagFields,
    event: TagEvent,
    found: Option<(Tagging, String, String, Option<Remote>)>,
) -> Vec<TagEffect> {
    match (sheet, event) {
        (sheet @ Sheet::Closed, TagEvent::Open(commit)) => {
            let Some((tagging, short, subject, origin)) = found else { return vec![] };
            fields.name = TextEdit::new(tagging.suggest().unwrap_or_default());
            fields.message = TextEdit::new("");
            fields.notes = NotesEditor::default();
            // Where pushing a tag releases, the push is the point; elsewhere a tag starts local.
            fields.push = tagging.trigger.is_some() && tagging.remote.is_some();
            let phase = Phase::Editing { error: None };
            *sheet = Sheet::Tag(Box::new(TagSheet { commit, short, subject, tagging, origin, phase }));
            vec![TagEffect::Focus(keys::TAG_NAME)]
        }
        (Sheet::Tag(t), TagEvent::TogglePush) if t.phase.editing() && t.tagging.remote.is_some() => {
            fields.push = !fields.push;
            vec![]
        }
        (Sheet::Tag(t), TagEvent::NameEdited) if t.phase.editing() => {
            t.phase = Phase::Editing { error: None };
            vec![]
        }
        (Sheet::Tag(t), TagEvent::Draft) if t.phase.editing() && fields.notes.draft != Draft::Asking => {
            let name = fields.name.text.trim().to_owned();
            if name.is_empty() {
                fields.notes.draft =
                    Draft::Failed("Name the tag first: the notes count from the last one to it.".to_owned());
                return vec![];
            }
            fields.notes.draft = Draft::Asking;
            vec![TagEffect::Draft { name, commit: t.commit.clone() }]
        }
        (Sheet::Tag(_), TagEvent::Drafted(result)) => match result {
            Ok(markdown) => {
                fields.notes.draft = Draft::Idle;
                fields.notes.markdown_shown = false;
                let draft = gxm_core::notes::draft(fields.name.text.trim(), &markdown);
                fields.notes.take_draft(&draft);
                vec![TagEffect::FocusNotes]
            }
            Err(why) => {
                fields.notes.draft = Draft::Failed(why);
                vec![]
            }
        },
        (Sheet::Tag(_), TagEvent::FilesChanged) => {
            fields.notes.files_commit = None;
            vec![TagEffect::CommitFiles(fields.notes.files.clone())]
        }
        (Sheet::Tag(_), TagEvent::FilesCommitted(made)) => {
            fields.notes.files_commit = made.ok();
            vec![]
        }
        (Sheet::Tag(t), TagEvent::Submit) if t.phase.editing() => {
            let name = fields.name.text.trim().to_owned();
            let taken = t.tagging.existing(&name).filter(|on| *on != t.commit);
            let files = t.writes_notes() && !fields.notes.files.is_empty();
            let refuse = match taken {
                _ if name.is_empty() => Some("Name the tag".to_owned()),
                Some(on) => Some(format!("{name} already exists, on {}", on.get(..7).unwrap_or(on))),
                None if files && t.files_base("").is_none() => {
                    Some("Attached files go up to GitHub, and this repository isn't on GitHub".to_owned())
                }
                None if files && !fields.push => {
                    Some("Attached files go up with the tag, so it has to be pushed".to_owned())
                }
                None => None,
            };
            if let Some(why) = refuse {
                t.phase = Phase::Editing { error: Some(why) };
                return vec![];
            }
            t.phase = Phase::Working;
            let new = |message| NewTag { name, commit: t.commit.clone(), message, push: fields.push, files: None };
            match (files, t.origin.clone()) {
                (true, Some(origin)) => vec![TagEffect::CreateWithFiles {
                    new: new(String::new()),
                    notes: fields.notes.notes(),
                    files: fields.notes.files.clone(),
                    origin,
                }],
                _ if t.writes_notes() => vec![TagEffect::Create(new(fields.notes.markdown(None)))],
                _ => vec![TagEffect::Create(new(fields.message.text.trim().to_owned()))],
            }
        }
        (sheet @ Sheet::Tag(_), TagEvent::Landed(Ok(remote))) => {
            let name = fields.name.text.trim().to_owned();
            let writes_notes = matches!(sheet, Sheet::Tag(t) if t.writes_notes());
            let with = match fields.notes.files.as_slice() {
                _ if !writes_notes => String::new(),
                [] => String::new(),
                [(one, _)] => format!(" and {one}"),
                many => format!(" and {} files", many.len()),
            };
            let said = match remote {
                Some(remote) => format!("Pushed {name}{with} to {remote}"),
                None => format!("Tagged {name}"),
            };
            *sheet = Sheet::Closed;
            vec![TagEffect::Done(said)]
        }
        (Sheet::Tag(t), TagEvent::Landed(Err(message))) => {
            t.phase = Phase::Editing { error: Some(Ops::reason(&message).to_owned()) };
            vec![]
        }
        _ => vec![],
    }
}

/// The tag being made, GitHub's draft of its notes, and the commit of their files, off the UI
/// thread.
#[derive(Default)]
pub struct TagJobs {
    job: Latest<Result<Option<String>, String>>,
    draft: Latest<Result<String, String>>,
    files: Latest<Result<String, String>>,
}

impl TagJobs {
    pub fn set_waker(&self, waker: Waker) {
        self.job.set_waker(waker.clone());
        self.draft.set_waker(waker.clone());
        self.files.set_waker(waker);
    }
}

impl App {
    /// Feeds `event` to the tag sheet and carries out what it asks for.
    pub fn tag_event(&mut self, event: TagEvent, tx: Option<&mut Tx>) {
        let found = match &event {
            TagEvent::Open(id) => self.commit(id).and_then(|c| {
                let tagging = self.history.as_ref()?.tagging.clone();
                let origin = self.repo().and_then(|r| r.summary.origin.clone());
                Some((tagging, c.short_id().to_owned(), c.subject.clone(), origin))
            }),
            _ => None,
        };
        let mut tx = tx;
        for effect in step(&mut self.sheet, &mut self.tag_fields, event, found) {
            match effect {
                TagEffect::Focus(key) => {
                    if let Some(tx) = tx.as_deref_mut() {
                        tx.focus(key, true);
                    }
                }
                TagEffect::FocusNotes => {
                    if let Some(tx) = tx.as_deref_mut() {
                        tx.focus(keys::notes_part(self.tag_fields.notes.at), true);
                    }
                }
                TagEffect::Draft { name, commit } => {
                    let Some(repo) = self.repo() else { continue };
                    let (path, origin) = (repo.summary.path.clone(), repo.summary.origin.clone());
                    let trigger = self.history.as_ref().and_then(|h| h.tagging.trigger.clone());
                    let input = (self.env.clone(), path, origin, name, commit, trigger);
                    self.tag_jobs.draft.request(input, |(env, path, origin, name, commit, trigger)| {
                        let origin: Remote = origin
                            .ok_or_else(|| "This repository has no remote on GitHub to draft notes from.".to_owned())?;
                        let previous = tag::previous(&env, &path, &commit, trigger.as_ref());
                        gxm_core::forge::notes::generate(&env, &origin, &name, &commit, previous.as_deref())
                    });
                }
                TagEffect::CommitFiles(files) => {
                    let Some(path) = self.repo().map(|r| r.summary.path.clone()) else { continue };
                    self.tag_jobs.files.request((self.env.clone(), path, files), |(env, path, files)| {
                        attach::files_commit(&env, &path, &files).map_err(Ops::said)
                    });
                }
                TagEffect::Create(new) => {
                    let Some(path) = self.repo().map(|r| r.summary.path.clone()) else { continue };
                    self.tag_jobs.job.request((self.env.clone(), path, new), |(env, path, new)| {
                        tag::create(&env, &path, &new).map_err(Ops::said)
                    });
                }
                TagEffect::CreateWithFiles { new, notes, files, origin } => {
                    let Some(path) = self.repo().map(|r| r.summary.path.clone()) else { continue };
                    let input = (self.env.clone(), path, new, notes, files, origin);
                    self.tag_jobs.job.request(input, |(env, path, mut new, notes, files, origin)| {
                        let commit = attach::files_commit(&env, &path, &files).map_err(Ops::said)?;
                        let base = attach::files_base(&origin, &commit);
                        new.message = gxm_core::notes::markdown_with(&notes, base.as_deref());
                        new.files = Some(commit);
                        tag::create(&env, &path, &new).map_err(Ops::said)
                    });
                }
                TagEffect::Done(said) => {
                    self.reload_repo();
                    if let Some(tx) = tx.as_deref_mut() {
                        self.toasts.show(&said, tx);
                    }
                }
            }
        }
    }

    /// Takes the finished tag, GitHub's draft, or the files' commit. True when the state changed.
    pub fn land_tag(&mut self, tx: &mut Tx) -> bool {
        let committed = self.tag_jobs.files.take().map(|c| self.tag_event(TagEvent::FilesCommitted(c), None)).is_some();
        let drafted =
            committed | self.tag_jobs.draft.take().map(|d| self.tag_event(TagEvent::Drafted(d), Some(tx))).is_some();
        let Some(result) = self.tag_jobs.job.take() else { return drafted };
        self.tag_event(TagEvent::Landed(result), Some(tx));
        true
    }
}
