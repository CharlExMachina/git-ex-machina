//! The work list: what's open in a repository and waiting on someone, in three sections, each
//! oldest first. Pull requests the viewer is asked to review, the viewer's own open pull
//! requests, and worktrees whose branch has no pull request yet. Merged work isn't listed: it's
//! done.
//!
//! [`rules`] holds the pure rules (which section a pull request is in, why it's stuck, which
//! threads need the viewer, how stale it is); [`worktree`] reads the checkouts from git;
//! [`conflicts`] works out what a pull request conflicts in and which merge caused it; and
//! [`read`] puts it together, from git and the provider.

pub mod brief;
pub mod conflicts;
pub mod read;
pub mod rules;
pub mod worktree;

pub use conflicts::Conflict;
pub use read::{Local, Open, Work};
pub use rules::{Entry, Item, Next, Reason, Section, Staleness, ThreadState, Tone};
pub use worktree::Worktree;
