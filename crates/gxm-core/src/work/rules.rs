//! The work list's rules, with no git and no network: given what the provider and git said,
//! which section each piece of work is in, how stale it is, why it's stuck and which threads
//! wait on the viewer.

use std::path::PathBuf;

use jiff::Timestamp;

use super::conflicts::Conflict;
use super::worktree::Worktree;
use crate::forge::model::{Check, CheckState, Event, Mergeable, Pull, PullState, ReviewStatus, Thread, User};

/// The work list's sections, in the order they're listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Section {
    /// Pull requests the viewer is asked to review and hasn't since the latest push.
    Review,
    /// The viewer's own open pull requests, drafts included.
    Yours,
    /// Worktrees whose branch has no pull request yet.
    Unproposed,
}

impl Section {
    pub const ALL: [Section; 3] = [Section::Review, Section::Yours, Section::Unproposed];

    pub fn title(self) -> &'static str {
        match self {
            Section::Review => "Needs your review",
            Section::Yours => "Your open pull requests",
            Section::Unproposed => "Worktrees with no PR yet",
        }
    }
}

/// A piece of work: a pull request, or a worktree with no pull request.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    Pull(u64),
    Worktree(PathBuf),
}

/// A row of the work list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub section: Section,
    pub item: Item,
    /// What its age counts from: the review request, the pull request's opening, or the
    /// worktree's last edit.
    pub since: Timestamp,
}

/// How a state reads at a glance: red, amber, green, or neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Bad,
    Warn,
    Good,
    Quiet,
}

/// How long something has waited, in the four steps the list colours ages by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Staleness {
    /// Under two days.
    Fresh,
    /// Two to five days.
    Settled,
    /// Five to ten days.
    Aging,
    /// Ten days or more.
    Stale,
}

impl Staleness {
    pub fn of(since: Timestamp, now: Timestamp) -> Staleness {
        match days(since, now) {
            ..2 => Staleness::Fresh,
            2..5 => Staleness::Settled,
            5..10 => Staleness::Aging,
            _ => Staleness::Stale,
        }
    }

    /// A count's tone when this is its oldest: red once stale, amber while aging.
    pub fn tone(self) -> Tone {
        match self {
            Staleness::Stale => Tone::Bad,
            Staleness::Aging => Tone::Warn,
            Staleness::Fresh | Staleness::Settled => Tone::Quiet,
        }
    }
}

/// Whole days from `since` to `now`, never negative.
pub fn days(since: Timestamp, now: Timestamp) -> i64 {
    (now.as_second() - since.as_second()).max(0) / 86_400
}

/// An age as a bare number in its largest whole unit: `now`, `4m`, `5h`, `23d`.
pub fn age(since: Timestamp, now: Timestamp) -> String {
    match (now.as_second() - since.as_second()).max(0) {
        ..60 => "now".to_owned(),
        s @ ..3600 => format!("{}m", s / 60),
        s @ ..86_400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86_400),
    }
}

/// What a person is called in a sentence: the first word of their name.
pub fn first_name(u: &User) -> &str {
    u.name.split_whitespace().next().unwrap_or(&u.login)
}

/// When the viewer was asked to review `pull`, if it's waiting on them: they're asked, it isn't
/// theirs or a draft, and they haven't reviewed it since its branch last moved. A request made
/// again after new commits brings it back.
pub fn asked_since(pull: &Pull, viewer: &str) -> Option<Timestamp> {
    if pull.summary.author.login == viewer || pull.summary.state != PullState::Open {
        return None;
    }
    let asked = pull.reviewers.iter().find(|r| r.user.login == viewer && r.status == ReviewStatus::Requested)?;
    let reviewed = pull
        .conversation
        .iter()
        .filter_map(|e| match e {
            Event::Review { author, at, .. } if author.login == viewer => Some(*at),
            _ => None,
        })
        .max();
    if let (Some(reviewed), Some(pushed)) = (reviewed, pull.pushed)
        && reviewed >= pushed
    {
        return None;
    }
    Some(asked.requested_at.unwrap_or(pull.created))
}

/// The viewer's own open pull request, drafts included.
pub fn is_yours(pull: &Pull, viewer: &str) -> bool {
    pull.summary.author.login == viewer && matches!(pull.summary.state, PullState::Open | PullState::Draft)
}

/// The work list's rows, in order: each section oldest first. `branches_with_pulls` are the
/// branches any pull request (open, merged or closed) came from: a worktree on one of those
/// isn't waiting for a pull request.
pub fn entries(
    viewer: Option<&str>,
    pulls: &[Pull],
    branches_with_pulls: &[&str],
    worktrees: &[Worktree],
) -> Vec<Entry> {
    let mut out = Vec::new();
    if let Some(viewer) = viewer {
        for p in pulls {
            let n = Item::Pull(p.summary.number);
            if let Some(since) = asked_since(p, viewer) {
                out.push(Entry { section: Section::Review, item: n, since });
            } else if is_yours(p, viewer) {
                out.push(Entry { section: Section::Yours, item: n, since: p.created });
            }
        }
    }
    for w in worktrees.iter().filter(|w| !w.main) {
        let (Some(branch), Some(since)) = (w.branch.as_deref(), w.edited) else { continue };
        let proposed = pulls.iter().any(|p| p.summary.head_ref == branch) || branches_with_pulls.contains(&branch);
        if !proposed {
            out.push(Entry { section: Section::Unproposed, item: Item::Worktree(w.path.clone()), since });
        }
    }
    out.sort_by_key(|e| (e.section, e.since));
    out
}

/// What a row of your own offers next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// It conflicts with its base.
    ResolveConflicts,
    /// Approved, every check green, nothing in the way.
    Merge,
    /// Waiting on something, said in a word or two: `Draft`, `Changes asked`.
    Wait(&'static str),
}

/// What's next for `pull`, from its author's side.
pub fn next(pull: &Pull) -> Next {
    let reviewed = |s| pull.reviewers.iter().any(|r| r.status == s);
    if pull.mergeable == Mergeable::Conflicts {
        Next::ResolveConflicts
    } else if pull.summary.state == PullState::Draft {
        Next::Wait("Draft")
    } else if reviewed(ReviewStatus::ChangesRequested) {
        Next::Wait("Changes asked")
    } else if pull.failing().next().is_some() {
        Next::Wait("Checks failing")
    } else if pull.checks.iter().any(|c| c.state == CheckState::Pending) {
        Next::Wait("Checks running")
    } else if reviewed(ReviewStatus::Approved) && pull.mergeable == Mergeable::Clean {
        Next::Merge
    } else {
        Next::Wait("In review")
    }
}

/// Where the review of your pull request stands, in a few words, and its tone: `Kenji
/// reviewed`, `Lea asked for changes`, `Kenji approved`.
pub fn review_state(pull: &Pull) -> (String, Tone) {
    if pull.summary.state == PullState::Draft {
        return ("not ready for review".to_owned(), Tone::Quiet);
    }
    let who = |s: ReviewStatus| -> Option<String> {
        let names: Vec<&str> = pull.reviewers.iter().filter(|r| r.status == s).map(|r| first_name(&r.user)).collect();
        (!names.is_empty()).then(|| and_list(&names))
    };
    if let Some(n) = who(ReviewStatus::ChangesRequested) {
        (format!("{n} asked for changes"), Tone::Warn)
    } else if let Some(n) = who(ReviewStatus::Approved) {
        (format!("{n} approved"), Tone::Good)
    } else if let Some(n) = who(ReviewStatus::Commented) {
        (format!("{n} reviewed"), Tone::Quiet)
    } else if let Some(n) = who(ReviewStatus::Requested) {
        (format!("waiting on {n}"), Tone::Quiet)
    } else {
        ("no reviewers yet".to_owned(), Tone::Quiet)
    }
}

/// `Kenji`, `Kenji and Lea`, `Kenji, Lea and Jo`.
fn and_list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Where an unresolved thread stands for the viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreadState {
    /// Someone else wrote last, to the viewer: on their own pull request, or in a thread they
    /// took part in.
    NeedsYou,
    /// The viewer wrote last; it's waiting on whoever they answered (None when no one else has
    /// written in it).
    Waiting(Option<User>),
    /// Others are talking; the viewer hasn't taken part.
    Others,
}

/// Where `thread` stands for `viewer` on `pull`; None once it's resolved (whatever its replies
/// say) or when it's empty. An outdated thread still counts.
pub fn thread_state(pull: &Pull, thread: &Thread, viewer: &str) -> Option<ThreadState> {
    let last = thread.comments.last()?;
    if thread.resolved {
        return None;
    }
    if last.author.login == viewer {
        let on = thread.comments.iter().rev().find(|c| c.author.login != viewer).map(|c| c.author.clone());
        return Some(ThreadState::Waiting(on));
    }
    let involved = pull.summary.author.login == viewer || thread.comments.iter().any(|c| c.author.login == viewer);
    Some(if involved { ThreadState::NeedsYou } else { ThreadState::Others })
}

/// The unresolved threads, those that need the viewer first and those waiting on others last.
pub fn unresolved<'a>(pull: &'a Pull, viewer: &str) -> Vec<(&'a Thread, ThreadState)> {
    let mut out: Vec<(&Thread, ThreadState)> =
        pull.threads.iter().filter_map(|t| thread_state(pull, t, viewer).map(|s| (t, s))).collect();
    out.sort_by_key(|(_, s)| match s {
        ThreadState::NeedsYou => 0,
        ThreadState::Others => 1,
        ThreadState::Waiting(_) => 2,
    });
    out
}

/// Someone other than its author has reviewed it or commented on a line.
pub fn reviewed(pull: &Pull) -> bool {
    let author = &pull.summary.author.login;
    !pull.threads.is_empty()
        || pull.conversation.iter().any(|e| matches!(e, Event::Review { author: a, .. } if a.login != *author))
}

/// A pull request's checks as the inspector lists them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checks<'a> {
    pub passed: usize,
    pub failed: usize,
    pub running: usize,
    /// Failed first, then running, then the rest, each in the provider's order.
    pub rows: Vec<&'a Check>,
}

impl Checks<'_> {
    /// Everything passed (or was skipped): one summary line says it all.
    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.running == 0
    }
}

/// The checks, or None when the repository runs none.
pub fn checks(pull: &Pull) -> Option<Checks<'_>> {
    if pull.checks.is_empty() {
        return None;
    }
    let count = |s| pull.checks.iter().filter(|c| c.state == s).count();
    let mut rows: Vec<&Check> = pull.checks.iter().collect();
    rows.sort_by_key(|c| match c.state {
        CheckState::Failed => 0,
        CheckState::Pending => 1,
        CheckState::Passed | CheckState::Skipped => 2,
    });
    Some(Checks {
        passed: count(CheckState::Passed),
        failed: count(CheckState::Failed),
        running: count(CheckState::Pending),
        rows,
    })
}

/// One reason a pull request isn't moving.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    Conflicts(Conflict),
    Failing(usize),
    /// Threads that need the viewer: how many, whose (when one person's), and since when.
    Unanswered {
        count: usize,
        from: Option<String>,
        since: Timestamp,
    },
    ChangesAsked(String),
    /// The viewer was asked to review it.
    Asked(Timestamp),
    Running(usize),
    Draft,
    /// Its reviewers haven't answered yet.
    WaitingOn {
        who: String,
        since: Timestamp,
    },
    NoReviewers,
    /// Nothing: approved, green and clean.
    Ready,
}

impl Reason {
    pub fn tone(&self) -> Tone {
        match self {
            Reason::Conflicts(_) | Reason::Failing(_) => Tone::Bad,
            Reason::Unanswered { .. } | Reason::ChangesAsked(_) | Reason::Asked(_) | Reason::NoReviewers => Tone::Warn,
            Reason::Running(_) | Reason::Draft | Reason::WaitingOn { .. } => Tone::Quiet,
            Reason::Ready => Tone::Good,
        }
    }

    pub fn sentence(&self, now: Timestamp) -> String {
        let plural = |n: usize, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
        match self {
            Reason::Conflicts(c) => c.sentence(),
            Reason::Failing(n) => format!("{} failing", plural(*n, "check is", "checks are")),
            Reason::Running(n) => format!("{} still running", plural(*n, "check is", "checks are")),
            Reason::Unanswered { count, from, since } => {
                let what = match (count, from) {
                    (1, Some(who)) => format!("{who}'s comment"),
                    (n, Some(who)) => format!("{n} of {who}'s comments"),
                    (n, None) => plural(*n, "comment", "comments"),
                };
                match days(*since, now) {
                    0 => format!("You haven't answered {what} yet"),
                    d => format!("You haven't answered {what} in {}", plural(d as usize, "day", "days")),
                }
            }
            Reason::ChangesAsked(who) => format!("{who} asked for changes"),
            Reason::Asked(since) => match days(*since, now) {
                0 => "You were asked to review it today".to_owned(),
                1 => "You were asked to review it yesterday".to_owned(),
                d => format!("You were asked to review it {d} days ago"),
            },
            Reason::Draft => "It's still a draft".to_owned(),
            Reason::WaitingOn { who, since } => match days(*since, now) {
                0 => format!("Waiting on {who}'s review"),
                d => format!("Waiting on {who}'s review for {}", plural(d as usize, "day", "days")),
            },
            Reason::NoReviewers => "No one is asked to review it".to_owned(),
            Reason::Ready => "Approved and every check passed: ready to merge".to_owned(),
        }
    }
}

/// Why `pull` is stuck, worst first. `conflict` is what git found when the provider reports
/// conflicts.
pub fn stuck(pull: &Pull, viewer: &str, conflict: Option<&Conflict>) -> Vec<Reason> {
    let mut out = Vec::new();
    if pull.mergeable == Mergeable::Conflicts {
        let base = pull.summary.base_ref.clone();
        out.push(Reason::Conflicts(conflict.cloned().unwrap_or(Conflict { base, files: Vec::new(), since: None })));
    }
    let failing = pull.failing().count();
    if failing > 0 {
        out.push(Reason::Failing(failing));
    }
    let needs: Vec<&Thread> =
        unresolved(pull, viewer).into_iter().filter(|(_, s)| *s == ThreadState::NeedsYou).map(|(t, _)| t).collect();
    if let Some(since) = needs.iter().filter_map(|t| t.comments.last().map(|c| c.at)).min() {
        let mut from: Vec<&User> = needs.iter().filter_map(|t| t.comments.last().map(|c| &c.author)).collect();
        from.dedup_by(|a, b| a.login == b.login);
        let from = if from.len() == 1 { Some(first_name(from[0]).to_owned()) } else { None };
        out.push(Reason::Unanswered { count: needs.len(), from, since });
    }
    let yours = pull.summary.author.login == viewer;
    if let Some(since) = asked_since(pull, viewer) {
        out.push(Reason::Asked(since));
    }
    if yours {
        let names =
            |s| -> Vec<&str> { pull.reviewers.iter().filter(|r| r.status == s).map(|r| first_name(&r.user)).collect() };
        let changes = names(ReviewStatus::ChangesRequested);
        if !changes.is_empty() {
            out.push(Reason::ChangesAsked(and_list(&changes)));
        }
        if pull.summary.state == PullState::Draft {
            out.push(Reason::Draft);
        } else if names(ReviewStatus::Approved).is_empty() && changes.is_empty() {
            let asked: Vec<_> = pull.reviewers.iter().filter(|r| r.status == ReviewStatus::Requested).collect();
            match asked.iter().filter_map(|r| r.requested_at).min() {
                _ if asked.is_empty() && names(ReviewStatus::Commented).is_empty() => out.push(Reason::NoReviewers),
                _ if asked.is_empty() => {}
                since => out.push(Reason::WaitingOn {
                    who: and_list(&asked.iter().map(|r| first_name(&r.user)).collect::<Vec<_>>()),
                    since: since.unwrap_or(pull.created),
                }),
            }
        }
    }
    let running = pull.checks.iter().filter(|c| c.state == CheckState::Pending).count();
    if running > 0 {
        out.push(Reason::Running(running));
    }
    if out.is_empty() && next(pull) == Next::Merge {
        out.push(Reason::Ready);
    }
    out
}

#[cfg(test)]
mod tests;
