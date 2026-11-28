//! Stable element keys. Journeys, scripts and accessibility (keys become automation ids) all
//! address elements through these, so a key is part of the app's interface: rename one only
//! together with every journey that uses it.

pub const HISTORY: &str = "history";
/// The uncommitted-changes row's id in the history.
pub const WORKTREE_ROW: &str = "worktree";
pub const REPO_FILTER: &str = "repo-filter";
pub const COMMIT_SEARCH: &str = "commit-search";
pub const SIDEBAR_TOGGLE: &str = "sidebar-toggle";
/// The bars along the window's top edge, which move the window as its title bar would: the
/// sidebar's, the toolbar over the history, the inspector's, and the band over an empty
/// workspace.
pub const SIDEBAR_BAR: &str = "sidebar-bar";
pub const TOOLBAR: &str = "toolbar";
pub const INSPECTOR_BAR: &str = "inspector-bar";
pub const TITLE_BAND: &str = "title-band";
pub const ADD_REPO: &str = "add-repo";
/// The menu the sidebar's + opens, and its two ways in: Add (from disk) and Clone.
pub const ADD_MENU: &str = "add-menu";
pub const ADD_MENU_ADD: &str = "add-menu-add";
pub const ADD_MENU_CLONE: &str = "add-menu-clone";
pub const FETCH: &str = "fetch";
pub const PULL: &str = "pull";
pub const PUSH: &str = "push";
pub const STAGE_ALL: &str = "stage-all";
pub const UNSTAGE_ALL: &str = "unstage-all";
pub const DISCARD_ALL: &str = "discard-all";
pub const COMMIT: &str = "commit";
pub const COMMIT_MESSAGE: &str = "commit-message";
pub const CHECKOUT: &str = "checkout";
pub const CHERRY_PICK: &str = "cherry-pick";
pub const REVERT: &str = "revert";
pub const BRANCH_FROM_HERE: &str = "branch-from-here";
pub const BISECT: &str = "bisect";
pub const SQUASH: &str = "squash";
/// Create tag on the selected commit: "Tag a release" where pushing a tag releases.
pub const CREATE_TAG: &str = "create-tag";
/// The tag window, its title: the commit it tags, the name and message, Push to origin, what
/// pushing starts, why the commit may not be the one to tag, and what went wrong.
pub const TAG_WINDOW: &str = "tag-window";
pub const TAG_TITLE: &str = "tag-title";
pub const TAG_ERROR: &str = "tag-error";
pub const TAG_CANCEL: &str = "tag-cancel";
pub const TAG_COMMIT: &str = "tag-commit";
pub const TAG_NAME: &str = "tag-name";
pub const TAG_MESSAGE: &str = "tag-message";
pub const TAG_PUSH: &str = "tag-push";
pub const TAG_RELEASE: &str = "tag-release";
pub const TAG_WARNING: &str = "tag-warning";
pub const TAG_CONFIRM: &str = "tag-confirm";
/// The release notes the tag carries, where the release takes its notes from the tag: the
/// editor, its formatting bar, GitHub's draft, and Write beside Markdown (what will be sent).
pub const NOTES: &str = "notes";
pub const NOTES_GENERATE: &str = "notes-generate";
pub const NOTES_WRITE: &str = "notes-write";
pub const NOTES_SOURCE: &str = "notes-source";
pub const NOTES_MARKDOWN: &str = "notes-markdown";
pub const NOTES_HEADING: &str = "notes-heading";
pub const NOTES_BOLD: &str = "notes-bold";
pub const NOTES_ITALIC: &str = "notes-italic";
pub const NOTES_STRIKE: &str = "notes-strike";
pub const NOTES_CODE: &str = "notes-code";
pub const NOTES_LINK: &str = "notes-link";
pub const NOTES_BULLETS: &str = "notes-bullets";
pub const NOTES_NUMBERS: &str = "notes-numbers";
pub const NOTES_QUOTE: &str = "notes-quote";
pub const NOTES_CODE_BLOCK: &str = "notes-code-block";
pub const NOTES_FOLD: &str = "notes-fold";
pub const NOTES_TASKS: &str = "notes-tasks";
pub const NOTES_ATTACH: &str = "notes-attach";
/// The popup `@` and `#` open: people to mention, pull requests to refer to.
pub const NOTES_PICKER: &str = "notes-picker";
/// The link's address, under the formatting bar while a link is being made.
pub const NOTES_LINK_URL: &str = "notes-link-url";

/// A task's checkbox, by its place among the tasks.
pub fn notes_task(i: usize) -> String {
    format!("notes-task-{i}")
}

/// A row of the `@` or `#` popup, by its place.
pub fn notes_pick(i: usize) -> String {
    format!("notes-pick-{i}")
}

/// One part of the release notes (a heading, a collapsed section's summary, or the text
/// between them), by its id.
pub fn notes_part(id: u64) -> String {
    format!("notes-part-{id}")
}
/// Why the last operation failed.
pub const OP_ERROR: &str = "op-error";
pub const BRANCH_SUMMARY: &str = "branch-summary";
pub const INSPECTOR: &str = "inspector";
pub const TAB_OVERVIEW: &str = "tab-overview";
pub const TAB_FILES: &str = "tab-files";
pub const COPY_ID: &str = "copy-id";
pub const INSPECTOR_TITLE: &str = "inspector-title";
pub const INSPECTOR_FILES: &str = "inspector-files";
pub const HUNK: &str = "hunk";
pub const EMPTY_STATE: &str = "empty-state";
/// The empty workspace's own Add and Clone buttons.
pub const EMPTY_ADD: &str = "empty-add";
pub const EMPTY_CLONE: &str = "empty-clone";
/// The sheet over the window (Add, Clone), and what it says went wrong.
pub const SHEET: &str = "sheet";
pub const SHEET_ERROR: &str = "sheet-error";
pub const SHEET_CANCEL: &str = "sheet-cancel";
pub const ADD_PATH: &str = "add-path";
/// Add's Choose…: the folder, picked in the system's folder chooser.
pub const ADD_CHOOSE: &str = "add-choose";
pub const ADD_CONFIRM: &str = "add-confirm";
pub const CLONE_URL: &str = "clone-url";
pub const CLONE_INTO: &str = "clone-into";
/// Clone's Choose…: where it clones into, picked in the system's folder chooser.
pub const CLONE_CHOOSE: &str = "clone-choose";
/// What Clone will do: the URL it clones over SSH, and the folder it clones into.
pub const CLONE_PLAN: &str = "clone-plan";
pub const CLONE_CONFIRM: &str = "clone-confirm";
/// Making a folder a repository: its name, where it lives, the README and the ignore list.
pub const INIT_NAME: &str = "init-name";
pub const INIT_LOCAL: &str = "init-local";
pub const INIT_GITHUB: &str = "init-github";
pub const INIT_README: &str = "init-readme";
pub const INIT_README_TEXT: &str = "init-readme-text";
pub const INIT_IGNORE: &str = "init-ignore";
pub const INIT_LFS: &str = "init-lfs";
pub const INIT_CONFIRM: &str = "init-confirm";

/// The sidebar's account control: who's signed in to GitHub, or Sign in.
pub const ACCOUNT: &str = "account";
/// The one-time code a sign-in shows, and the sheet's buttons.
pub const SIGNIN_CODE: &str = "signin-code";
pub const SIGNIN_COPY: &str = "signin-copy";
pub const SIGNIN_OPEN: &str = "signin-open";
pub const SIGNIN_RETRY: &str = "signin-retry";
/// The window's lower right corner, where toasts say something just happened.
pub const TOASTS: &str = "toasts";
pub const SIGN_OUT: &str = "sign-out";
/// The update's icon button at the sidebar's foot: check for one, download it, restart into
/// it. Its label says which.
pub const UPDATE: &str = "update";
/// Why the last download of an update failed, under its button.
pub const UPDATE_ERROR: &str = "update-error";
/// The restart sheet's release notes, and its two answers.
pub const UPDATE_NOTES: &str = "update-notes";
pub const UPDATE_LATER: &str = "update-later";
pub const UPDATE_RESTART: &str = "update-restart";

/// A repository's row in the sidebar.
pub fn repo(name: &str) -> String {
    format!("repo-{}", sanitize(name))
}

/// A changed file's row in the inspector.
pub fn changed_file(path: &str) -> String {
    format!("file-{}", sanitize(path))
}

/// Keys can't contain `/` (it separates a key path), so paths and branch names swap it for `:`.
fn sanitize(s: &str) -> String {
    s.replace('/', ":")
}

// Pull requests: the inspector's row for a commit's pull request and its button.
pub const PULL_REQUEST_ROW: &str = "pull-request-row";
pub const SEE_PULL_REQUEST: &str = "see-pull-request";
/// Why pull requests can't be shown (no token, a provider not supported yet, an error).
pub const PULL_NOTICE: &str = "pull-notice";

// A pull request on the graph.
/// The `#131` row above the pull request's tip.
pub const PULL_BANNER: &str = "pull-banner";
/// The line under the toolbar naming the pull request.
pub const PULL_SUMMARY: &str = "pull-summary";
pub const REVIEW: &str = "review";
pub const CHECK_OUT_PULL: &str = "check-out-pull";
pub const SQUASH_AND_MERGE: &str = "squash-and-merge";
pub const CLOSE_PULL_REQUEST: &str = "close-pull-request";
pub const OPEN_ON_PROVIDER: &str = "open-on-provider";
pub const PULL_INSPECTOR: &str = "pull-inspector";
pub const TAB_CHECKS: &str = "tab-checks";
pub const TAB_CONVERSATION: &str = "tab-conversation";
pub const MERGE_BOX: &str = "merge-box";

// The review cockpit.
pub const COCKPIT_BACK: &str = "cockpit-back";
pub const COCKPIT_FILES: &str = "cockpit-files";
pub const DIFF: &str = "diff";
pub const DIFF_UNIFIED: &str = "diff-unified";
pub const DIFF_SPLIT: &str = "diff-split";
pub const FILE_HEADER: &str = "file-header";
pub const VIEWED: &str = "viewed";
/// The cockpit's shelf of viewed files: its header folds it open and shut.
pub const VIEWED_SHELF: &str = "viewed-shelf";
/// The composer for a new comment on a line.
pub const LINE_COMMENT: &str = "line-comment";
pub const REVIEW_SUMMARY: &str = "review-summary";
pub const VERDICT_COMMENT: &str = "verdict-comment";
pub const VERDICT_APPROVE: &str = "verdict-approve";
pub const VERDICT_REQUEST_CHANGES: &str = "verdict-request-changes";
pub const SUBMIT_REVIEW: &str = "submit-review";
pub const REVIEW_PANEL: &str = "review-panel";

/// The cockpit's `n`th hunk of the file shown, for `j` and `k`.
pub fn hunk(n: usize) -> String {
    format!("hunk-{n}")
}

/// A reviewer's row, by login.
pub fn reviewer(login: &str) -> String {
    format!("reviewer-{}", sanitize(login))
}

/// A file's row in the review cockpit's sidebar.
pub fn pull_file(path: &str) -> String {
    format!("pull-file-{}", sanitize(path))
}

/// A review thread in the diff, by the provider's id.
pub fn thread(id: &str) -> String {
    format!("thread-{}", sanitize(id))
}

/// The reply well under a thread, which opens the field when pressed.
pub fn reply(id: &str) -> String {
    format!("reply-{}", sanitize(id))
}

/// The field a reply is written in, once open.
pub fn reply_field(id: &str) -> String {
    format!("reply-field-{}", sanitize(id))
}

/// A line of the diff: `new` for the line as the pull request leaves it, else as it was.
pub fn diff_line(path: &str, new: bool, line: u32) -> String {
    format!("line-{}-{}{line}", sanitize(path), if new { 'R' } else { 'L' })
}

// Merging from the merge box.
pub const MERGE_START: &str = "merge-start";
/// The squash commit's message, while confirming.
pub const MERGE_HEADLINE: &str = "merge-headline";
pub const CONFIRM_MERGE: &str = "confirm-merge";
pub const CANCEL_MERGE: &str = "cancel-merge";

/// A commit in the cockpit's sidebar, by its short id: pressed, the cockpit shows it alone.
pub fn pull_commit(short: &str) -> String {
    format!("pull-commit-{short}")
}

/// Back to every commit's changes in the cockpit.
pub const PULL_COMMITS_ALL: &str = "pull-commits-all";

// Repository settings.
/// The history header's button that opens them.
pub const SETTINGS: &str = "settings";
pub const SETTINGS_DIALOG: &str = "settings-dialog";
pub const SETTINGS_CLOSE: &str = "settings-close";
/// The header's word on the last change: `Saved on GitHub`, `Saving…`, or what went wrong.
pub const SETTINGS_STATUS: &str = "settings-status";
/// Why some settings can't be changed here (no admin rights, no provider).
pub const SETTINGS_NOTICE: &str = "settings-notice";
/// The section's content.
pub const SETTINGS_PAGE: &str = "settings-page";
/// Settings › Agents with no agent harness installed: how to add one.
pub const AGENTS_EMPTY: &str = "agents-empty";

/// A section in the dialog's sidebar: `settings-section-pull-requests`.
pub fn settings_section(title: &str) -> String {
    format!("settings-section-{}", slug(title))
}

/// A setting's control (a checkbox, a choice, a button), by its id: `setting-wiki`.
pub fn setting(id: &str) -> String {
    format!("setting-{}", slug(id))
}

/// A setting's text field: `setting-field-description`.
pub fn setting_field(id: &str) -> String {
    format!("setting-field-{}", slug(id))
}

/// A row in one of the dialog's lists: `setting-item-person-kenji-mori`.
pub fn setting_item(kind: &str, id: &str) -> String {
    format!("setting-item-{kind}-{}", slug(id))
}

/// `Pull requests` → `pull-requests`, `fix/avatar-cache` → `fix:avatar-cache`.
fn slug(s: &str) -> String {
    sanitize(&s.to_lowercase().replace(' ', "-"))
}

/// Toast `id` in [`TOASTS`].
pub fn toast(id: u64) -> String {
    format!("toast-{id}")
}
