//! What a provider says about pull requests, in the provider's terms but none of its formats:
//! GitHub, GitLab and Bitbucket all map onto these.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// A person on the provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub login: String,
    /// Their display name, or their login when they haven't set one.
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PullState {
    Open,
    Draft,
    Merged,
    Closed,
}

impl PullState {
    pub fn word(self) -> &'static str {
        match self {
            PullState::Open => "Open",
            PullState::Draft => "Draft",
            PullState::Merged => "Merged",
            PullState::Closed => "Closed",
        }
    }
}

/// A pull request as a list of them shows it: enough to find its commits in the history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullSummary {
    pub number: u64,
    pub title: String,
    pub state: PullState,
    pub author: User,
    /// The branch it merges from, and the commit that branch is at.
    pub head_ref: String,
    pub head_sha: String,
    /// The branch it merges into.
    pub base_ref: String,
    /// The commit merging it made, once merged.
    pub merge_commit: Option<String>,
    /// Its page on the provider.
    pub url: String,
}

/// A pull request in full: the summary plus its review, its checks and its threads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pull {
    pub summary: PullSummary,
    /// The provider's id for it, which writes (a review) refer to.
    pub id: String,
    pub body: String,
    pub created: Timestamp,
    pub commits: u32,
    /// When its branch last moved: its newest commit's date. A review from before it is a
    /// review of older code.
    #[serde(default)]
    pub pushed: Option<Timestamp>,
    pub mergeable: Mergeable,
    /// Who reviewed it, in the order they did, then who's still asked to.
    pub reviewers: Vec<Reviewer>,
    pub checks: Vec<Check>,
    pub threads: Vec<Thread>,
    /// Reviews and comments, oldest first: what the Conversation tab lists and counts.
    pub conversation: Vec<Event>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mergeable {
    Clean,
    Conflicts,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reviewer {
    pub user: User,
    pub status: ReviewStatus,
    /// When they were last asked to review it, while they're still asked.
    #[serde(default)]
    pub requested_at: Option<Timestamp>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewStatus {
    Requested,
    Approved,
    ChangesRequested,
    Commented,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    pub state: CheckState,
    /// What it says about itself: `failed at step 20`.
    pub title: String,
    /// How long it ran, once it finished.
    pub seconds: Option<u32>,
    /// The provider's id for the job, when it can be run again from here (a GitHub Actions job).
    #[serde(default)]
    pub job: Option<String>,
    /// The workflow run it belongs to, whose failed jobs can be run again together.
    #[serde(default)]
    pub run: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckState {
    Pending,
    Passed,
    Failed,
    Skipped,
}

/// Which side of a diff a line is on: as it was, or as the pull request leaves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Old,
    New,
}

/// Comments on one line of the diff, replies included.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    pub path: String,
    /// None when the line no longer exists in the diff (an outdated thread).
    pub line: Option<u32>,
    pub side: Side,
    pub resolved: bool,
    pub comments: Vec<Comment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub author: User,
    pub body: String,
    pub at: Timestamp,
}

/// An entry in a pull request's conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Review { author: User, status: ReviewStatus, body: String, at: Timestamp },
    Comment(Comment),
}

impl Event {
    pub fn at(&self) -> Timestamp {
        match self {
            Event::Review { at, .. } => *at,
            Event::Comment(c) => c.at,
        }
    }
}

/// What a review concludes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    #[default]
    Comment,
    Approve,
    RequestChanges,
}

/// A comment on a line, not yet sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewComment {
    pub path: String,
    pub line: u32,
    pub side: Side,
    pub body: String,
}

/// A reply to a thread, not yet sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub thread: String,
    pub body: String,
}

/// A review to send, all at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submission {
    pub verdict: Verdict,
    pub body: String,
    pub comments: Vec<NewComment>,
    pub replies: Vec<Reply>,
}

impl Pull {
    pub fn failing(&self) -> impl Iterator<Item = &Check> {
        self.checks.iter().filter(|c| c.state == CheckState::Failed)
    }

    pub fn approvals(&self) -> usize {
        self.reviewers.iter().filter(|r| r.status == ReviewStatus::Approved).count()
    }

    /// `login`'s place in the review: None when they're neither asked nor have reviewed.
    pub fn review_of(&self, login: &str) -> Option<ReviewStatus> {
        self.reviewers.iter().find(|r| r.user.login == login).map(|r| r.status)
    }

    /// Threads no one has resolved.
    pub fn unresolved(&self) -> usize {
        self.threads.iter().filter(|t| !t.resolved).count()
    }
}
