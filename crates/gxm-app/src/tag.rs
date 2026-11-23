//! The tag window's machine: opened on a commit, editing the name, message (or release notes)
//! and whether to push, then tagging (and pushing) off the UI thread, then closed with the main
//! window told what happened, or back to editing with git's reason. Closing the window keeps
//! what was written for that commit; Cancel throws it away.

use std::path::PathBuf;

use gxm_core::attach;
use gxm_core::history::RefLabel;
use gxm_core::notes::Notes;
use gxm_core::release::NotesFrom;
use gxm_core::remote::Remote;
use gxm_core::tag::{NewTag, Tagging};
use rune::TextEdit;

use crate::keys;
use crate::notes::{Draft, NotesEditor};
use crate::ops::Ops;
use crate::sheet::Phase;

/// What the main window knows about the commit to tag, sent to the tag window to open it.
#[derive(Clone, Debug)]
pub struct Opening {
    /// The main window's environment, signed in as it is.
    pub env: gxm_core::env::Env,
    /// The repository's folder.
    pub repo: PathBuf,
    /// The commit's full id, its short id and subject as the history shows them.
    pub commit: String,
    pub short: String,
    pub subject: String,
    pub tagging: Tagging,
    /// Where the repository lives on its provider: where its notes link attached files.
    pub origin: Option<Remote>,
    /// People the notes can mention (login, name): who's signed in, then who wrote the
    /// repository's pull requests.
    pub people: Vec<(String, String)>,
    /// Pull requests the notes can refer to (number, title).
    pub pulls: Vec<(u64, String)>,
}

/// The commit being tagged, what the repository's tags say about it, and where tagging is.
pub struct TagSheet {
    pub opening: Opening,
    /// The refs that contain the commit, once read: why it may be an odd one to tag.
    pub contained_in: Option<Vec<RefLabel>>,
    pub phase: Phase,
}

/// The window's fields.
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
    /// Whether the release takes its notes from the tag, so the window writes them.
    pub fn writes_notes(&self) -> bool {
        self.opening.tagging.trigger.as_ref().is_some_and(|t| t.notes == NotesFrom::Tag)
    }

    /// Where the notes link the files of `commit` (`gxm_core::attach`), when the repository is
    /// on GitHub.
    pub fn files_base(&self, commit: &str) -> Option<String> {
        self.opening.origin.as_ref().and_then(|o| attach::files_base(o, commit))
    }

    /// Why this commit may not be the one to tag, once the refs that contain it are read.
    pub fn warnings(&self, push: bool) -> Vec<String> {
        self.contained_in.as_ref().map_or_else(Vec::new, |c| self.opening.tagging.warnings(c, push))
    }
}

pub enum TagEvent {
    /// Tag this commit.
    Open(Box<Opening>),
    /// The refs that contain the commit, or why they couldn't be read.
    Read(Result<Vec<RefLabel>, String>),
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
    /// Throw away what was written, and close.
    Cancel,
}

#[derive(Debug)]
pub enum TagEffect {
    /// Show the window in front.
    Show,
    Focus(&'static str),
    /// The caret into the notes, where it last was.
    FocusNotes,
    /// Read which refs contain the commit.
    Read,
    /// GitHub's notes for the tag called this.
    Draft(String),
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
    /// Hide the window.
    Close,
    /// The tag is made: the main window reads the history again and says this.
    Done(String),
}

/// One transition.
pub fn step(sheet: &mut Option<TagSheet>, fields: &mut TagFields, event: TagEvent) -> Vec<TagEffect> {
    match (sheet.as_mut(), event) {
        // Tagging is under way: the window shows it, whatever else was asked for.
        (Some(t), TagEvent::Open(_)) if !t.phase.editing() => vec![TagEffect::Show],
        // What was written for this commit, left in the closed window, is still there.
        (Some(t), TagEvent::Open(o)) if t.opening.commit == o.commit && t.opening.repo == o.repo => {
            t.opening = *o;
            vec![TagEffect::Show, TagEffect::Read]
        }
        (_, TagEvent::Open(o)) => {
            fields.name = TextEdit::new(o.tagging.suggest().unwrap_or_default());
            fields.message = TextEdit::new("");
            fields.notes = NotesEditor::default();
            // Where pushing a tag releases, the push is the point; elsewhere a tag starts local.
            fields.push = o.tagging.trigger.is_some() && o.tagging.remote.is_some();
            let phase = Phase::Editing { error: None };
            *sheet = Some(TagSheet { opening: *o, contained_in: None, phase });
            vec![TagEffect::Show, TagEffect::Read, TagEffect::Focus(keys::TAG_NAME)]
        }
        (Some(t), TagEvent::Read(read)) => {
            t.contained_in = read.ok();
            vec![]
        }
        (Some(t), TagEvent::TogglePush) if t.phase.editing() && t.opening.tagging.remote.is_some() => {
            fields.push = !fields.push;
            vec![]
        }
        (Some(t), TagEvent::NameEdited) if t.phase.editing() => {
            t.phase = Phase::Editing { error: None };
            vec![]
        }
        (Some(t), TagEvent::Draft) if t.phase.editing() && fields.notes.draft != Draft::Asking => {
            let name = fields.name.text.trim().to_owned();
            if name.is_empty() {
                fields.notes.draft =
                    Draft::Failed("Name the tag first: the notes count from the last one to it.".to_owned());
                return vec![];
            }
            fields.notes.draft = Draft::Asking;
            vec![TagEffect::Draft(name)]
        }
        (Some(_), TagEvent::Drafted(result)) => match result {
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
        (Some(_), TagEvent::FilesChanged) => {
            fields.notes.files_commit = None;
            vec![TagEffect::CommitFiles(fields.notes.files.clone())]
        }
        (Some(_), TagEvent::FilesCommitted(made)) => {
            fields.notes.files_commit = made.ok();
            vec![]
        }
        (Some(t), TagEvent::Submit) if t.phase.editing() => submit(t, fields),
        (Some(t), TagEvent::Landed(Ok(remote))) => {
            let name = fields.name.text.trim().to_owned();
            let with = match fields.notes.files.as_slice() {
                _ if !t.writes_notes() => String::new(),
                [] => String::new(),
                [(one, _)] => format!(" and {one}"),
                many => format!(" and {} files", many.len()),
            };
            let said = match remote {
                Some(remote) => format!("Pushed {name}{with} to {remote}"),
                None => format!("Tagged {name}"),
            };
            *sheet = None;
            vec![TagEffect::Close, TagEffect::Done(said)]
        }
        (Some(t), TagEvent::Landed(Err(message))) => {
            t.phase = Phase::Editing { error: Some(Ops::reason(&message).to_owned()) };
            vec![]
        }
        (Some(t), TagEvent::Cancel) if t.phase.editing() => {
            *sheet = None;
            *fields = TagFields::default();
            vec![TagEffect::Close]
        }
        _ => vec![],
    }
}

/// Submit: refused with the reason, or the tag to make.
fn submit(t: &mut TagSheet, fields: &mut TagFields) -> Vec<TagEffect> {
    let name = fields.name.text.trim().to_owned();
    let taken = t.opening.tagging.existing(&name).filter(|on| *on != t.opening.commit);
    let files = t.writes_notes() && !fields.notes.files.is_empty();
    let refuse = match taken {
        _ if name.is_empty() => Some("Name the tag".to_owned()),
        Some(on) => Some(format!("{name} already exists, on {}", on.get(..7).unwrap_or(on))),
        None if files && t.files_base("").is_none() => {
            Some("Attached files go up to GitHub, and this repository isn't on GitHub".to_owned())
        }
        None if files && !fields.push => Some("Attached files go up with the tag, so it has to be pushed".to_owned()),
        None => None,
    };
    if let Some(why) = refuse {
        t.phase = Phase::Editing { error: Some(why) };
        return vec![];
    }
    t.phase = Phase::Working;
    let commit = t.opening.commit.clone();
    let new = |message| NewTag { name, commit, message, push: fields.push, files: None };
    match (files, t.opening.origin.clone()) {
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

#[cfg(test)]
mod tests {
    use gxm_core::tag::Tagging;

    use super::*;

    fn opening(commit: &str) -> Box<Opening> {
        Box::new(Opening {
            env: gxm_core::env::Env::from_process(),
            repo: PathBuf::from("/code/beacon"),
            commit: commit.to_owned(),
            short: commit.get(..7).unwrap_or(commit).to_owned(),
            subject: "Fix the --json flag".to_owned(),
            tagging: Tagging::default(),
            origin: None,
            people: Vec::new(),
            pulls: Vec::new(),
        })
    }

    #[test]
    fn closing_keeps_what_was_written_for_the_commit_and_cancel_throws_it_away() {
        let (mut sheet, mut fields) = (None, TagFields::default());
        step(&mut sheet, &mut fields, TagEvent::Open(opening("aaaaaaa1")));
        fields.message = TextEdit::new("The first beta");
        // The window closed (its close button): opening the same commit again finds the message.
        let again = step(&mut sheet, &mut fields, TagEvent::Open(opening("aaaaaaa1")));
        assert!(matches!(again.as_slice(), [TagEffect::Show, TagEffect::Read]), "{again:?}");
        assert_eq!(fields.message.text, "The first beta");
        // Another commit starts afresh.
        step(&mut sheet, &mut fields, TagEvent::Open(opening("bbbbbbb2")));
        assert_eq!(fields.message.text, "");
        fields.message = TextEdit::new("A milestone");
        assert!(matches!(step(&mut sheet, &mut fields, TagEvent::Cancel).as_slice(), [TagEffect::Close]));
        step(&mut sheet, &mut fields, TagEvent::Open(opening("bbbbbbb2")));
        assert_eq!(fields.message.text, "", "Cancel threw the message away");
    }
}
