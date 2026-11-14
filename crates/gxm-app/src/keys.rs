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
pub const TAB_MESSAGE: &str = "tab-message";
pub const TAB_FILES: &str = "tab-files";
pub const COPY_ID: &str = "copy-id";
pub const INSPECTOR_TITLE: &str = "inspector-title";
pub const INSPECTOR_FILES: &str = "inspector-files";
pub const HUNK: &str = "hunk";
pub const EMPTY_STATE: &str = "empty-state";

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
