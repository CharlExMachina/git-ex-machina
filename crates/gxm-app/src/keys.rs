//! Stable element keys. Journeys, scripts and accessibility (keys become automation ids) all
//! address elements through these, so a key is part of the app's interface: rename one only
//! together with every journey that uses it.

pub const HISTORY: &str = "history";
/// The uncommitted-changes row's id in the history.
pub const WORKTREE_ROW: &str = "worktree";
pub const REPO_FILTER: &str = "repo-filter";
pub const COMMIT_SEARCH: &str = "commit-search";
pub const SIDEBAR_TOGGLE: &str = "sidebar-toggle";
pub const ADD_REPO: &str = "add-repo";
pub const CLONE: &str = "clone";
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
pub const ADD_CONFIRM: &str = "add-confirm";
pub const CLONE_URL: &str = "clone-url";
pub const CLONE_INTO: &str = "clone-into";
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
pub const INIT_CONFIRM: &str = "init-confirm";

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
