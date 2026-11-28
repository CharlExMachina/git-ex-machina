use std::path::PathBuf;

use jiff::Timestamp;

use super::*;
use crate::forge::model::{Comment, PullSummary, Reviewer, Side};

const NOW: &str = "2026-11-11T15:00:00Z";

fn at(s: &str) -> Timestamp {
    s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
}

fn ago(days: i64) -> Timestamp {
    at(NOW) - jiff::SignedDuration::from_hours(24 * days)
}

fn user(login: &str, name: &str) -> User {
    User { login: login.to_owned(), name: name.to_owned() }
}

fn ana() -> User {
    user("ana-ruiz", "Ana Ruiz")
}

fn kenji() -> User {
    user("kenji-mori", "Kenji Mori")
}

fn lea() -> User {
    user("lea-brandt", "Lea Brandt")
}

fn pull(number: u64, author: User, created: Timestamp) -> Pull {
    Pull {
        summary: PullSummary {
            number,
            title: format!("Pull {number}"),
            state: PullState::Open,
            author,
            head_ref: format!("branch-{number}"),
            head_sha: "abc".to_owned(),
            base_ref: "main".to_owned(),
            merge_commit: None,
            url: String::new(),
        },
        id: format!("PR_{number}"),
        body: String::new(),
        created,
        commits: 1,
        pushed: Some(created),
        mergeable: Mergeable::Clean,
        reviewers: Vec::new(),
        checks: Vec::new(),
        threads: Vec::new(),
        conversation: Vec::new(),
    }
}

fn asked(who: User, when: Timestamp) -> Reviewer {
    Reviewer { user: who, status: ReviewStatus::Requested, requested_at: Some(when) }
}

fn review(who: User, status: ReviewStatus) -> Reviewer {
    Reviewer { user: who, status, requested_at: None }
}

fn reviewed_at(who: User, when: Timestamp) -> Event {
    Event::Review { author: who, status: ReviewStatus::Commented, body: String::new(), at: when }
}

fn thread(resolved: bool, line: Option<u32>, said: &[(User, Timestamp)]) -> Thread {
    Thread {
        id: "T".to_owned(),
        path: "src/a.rs".to_owned(),
        line,
        side: Side::New,
        resolved,
        comments: said.iter().map(|(u, t)| Comment { author: u.clone(), body: "…".to_owned(), at: *t }).collect(),
    }
}

fn check(name: &str, state: CheckState) -> Check {
    Check { name: name.to_owned(), state, title: String::new(), seconds: Some(60), job: None, run: None }
}

fn worktree(name: &str, branch: Option<&str>, edited: Timestamp) -> Worktree {
    Worktree {
        path: PathBuf::from(format!("/w/{name}")),
        name: name.to_owned(),
        main: false,
        branch: branch.map(str::to_owned),
        head: Some("h".to_owned()),
        subject: None,
        upstream: None,
        changed: 0,
        conflicted: 0,
        unpushed: 0,
        behind: 0,
        behind_base: 0,
        edited: Some(edited),
    }
}

#[test]
fn a_review_waits_on_you_until_you_review_and_comes_back_when_asked_again_after_new_commits() {
    let mut p = pull(122, lea(), ago(20));
    assert_eq!(asked_since(&p, "ana-ruiz"), None, "not asked");
    p.reviewers.push(asked(ana(), ago(14)));
    assert_eq!(asked_since(&p, "ana-ruiz"), Some(ago(14)), "its age counts from the request");
    // Ana reviews; GitHub drops the request.
    p.reviewers.clear();
    p.conversation.push(reviewed_at(ana(), ago(3)));
    assert_eq!(asked_since(&p, "ana-ruiz"), None);
    // Asked again without new commits: she's seen this code.
    p.reviewers.push(asked(ana(), ago(2)));
    assert_eq!(asked_since(&p, "ana-ruiz"), None, "nothing new to review since her review");
    // New commits, then a new request: it's back, aged from the new request.
    p.pushed = Some(ago(1));
    p.reviewers = vec![asked(ana(), ago(1))];
    assert_eq!(asked_since(&p, "ana-ruiz"), Some(ago(1)));
    // Someone else's draft, or one's own pull request, never waits on a review from them.
    p.summary.state = PullState::Draft;
    assert_eq!(asked_since(&p, "ana-ruiz"), None);
    let mut own = pull(118, ana(), ago(23));
    own.reviewers.push(asked(ana(), ago(1)));
    assert_eq!(asked_since(&own, "ana-ruiz"), None);
}

#[test]
fn the_list_has_three_sections_each_oldest_first_and_nothing_merged() {
    let mut review_new = pull(135, kenji(), ago(5));
    review_new.reviewers.push(asked(ana(), ago(2)));
    let mut review_old = pull(122, lea(), ago(20));
    review_old.reviewers.push(asked(ana(), ago(14)));
    let mine_new = Pull {
        summary: PullSummary { state: PullState::Draft, ..pull(133, ana(), ago(1)).summary },
        ..pull(133, ana(), ago(1))
    };
    let mine_old = pull(118, ana(), ago(23));
    let theirs = pull(140, kenji(), ago(30));
    let pulls = vec![review_new, mine_new, theirs, review_old, mine_old];
    let mut main = worktree("atlas-web", Some("main"), ago(0));
    main.main = true;
    let worktrees = vec![
        main,
        worktree("passkeys", Some("branch-118"), ago(19)),
        worktree("badges", Some("ui/badges"), at("2026-11-11T14:56:00Z")),
        worktree("old", Some("ui/old"), ago(40)),
        worktree("landed", Some("feature/landed"), ago(9)),
        worktree("detached", None, ago(2)),
    ];
    let e = entries(Some("ana-ruiz"), &pulls, &["feature/landed"], &worktrees);
    let rows: Vec<(Section, String)> = e
        .iter()
        .map(|e| {
            let what = match &e.item {
                Item::Pull(n) => format!("#{n}"),
                Item::Worktree(p) => p.display().to_string(),
            };
            (e.section, what)
        })
        .collect();
    let want = [
        (Section::Review, "#122"),
        (Section::Review, "#135"),
        (Section::Yours, "#118"),
        (Section::Yours, "#133"),
        (Section::Unproposed, "/w/old"),
        (Section::Unproposed, "/w/badges"),
    ];
    assert_eq!(rows, want.map(|(s, w)| (s, w.to_owned())), "#140 isn't Ana's business; landed's PR merged");
}

#[test]
fn a_thread_needs_you_when_someone_else_wrote_last_and_waits_when_you_did() {
    let mut p = pull(118, ana(), ago(23));
    p.threads = vec![
        thread(false, Some(88), &[(kenji(), ago(19))]),
        thread(false, Some(121), &[(kenji(), ago(19)), (ana(), ago(18))]),
        thread(true, Some(5), &[(kenji(), ago(19))]),
        // Outdated: its line is gone, but it's still unanswered.
        thread(false, None, &[(kenji(), ago(19)), (ana(), ago(18)), (kenji(), ago(17))]),
    ];
    let states: Vec<ThreadState> = unresolved(&p, "ana-ruiz").into_iter().map(|(_, s)| s).collect();
    assert_eq!(states, [ThreadState::NeedsYou, ThreadState::NeedsYou, ThreadState::Waiting(Some(kenji()))]);

    // On someone else's pull request, only threads Ana took part in are hers.
    let mut theirs = pull(131, lea(), ago(1));
    theirs.threads = vec![
        thread(false, Some(5), &[(kenji(), ago(1)), (lea(), ago(1))]),
        thread(false, Some(9), &[(ana(), ago(1)), (lea(), ago(0))]),
    ];
    let states: Vec<ThreadState> = unresolved(&theirs, "ana-ruiz").into_iter().map(|(_, s)| s).collect();
    assert_eq!(states, [ThreadState::NeedsYou, ThreadState::Others]);
}

#[test]
fn stuck_says_why_worst_first() {
    let now = at(NOW);
    let mut p = pull(118, ana(), ago(23));
    p.mergeable = Mergeable::Conflicts;
    p.reviewers.push(review(kenji(), ReviewStatus::Commented));
    p.conversation.push(reviewed_at(kenji(), ago(19)));
    p.checks = vec![
        check("build", CheckState::Passed),
        check("e2e / safari", CheckState::Failed),
        check("e2e / firefox", CheckState::Failed),
    ];
    p.threads = vec![
        thread(false, Some(88), &[(kenji(), ago(19))]),
        thread(false, Some(140), &[(kenji(), ago(19))]),
        thread(false, Some(121), &[(kenji(), ago(19)), (ana(), ago(18))]),
    ];
    let conflict = Conflict { base: "main".to_owned(), files: vec!["src/sidebar.rs".to_owned()], since: Some(121) };
    let why: Vec<(Tone, String)> =
        stuck(&p, "ana-ruiz", Some(&conflict)).iter().map(|r| (r.tone(), r.sentence(now))).collect();
    assert_eq!(
        why,
        [
            (Tone::Bad, "Conflicts with main in src/sidebar.rs, since #121 landed".to_owned()),
            (Tone::Bad, "2 checks are failing".to_owned()),
            (Tone::Warn, "You haven't answered 2 of Kenji's comments in 19 days".to_owned()),
        ]
    );
    assert_eq!(next(&p), Next::ResolveConflicts);

    // A review request, and nothing else wrong.
    let mut r = pull(122, lea(), ago(20));
    r.reviewers.push(asked(ana(), ago(14)));
    r.checks = vec![check("build", CheckState::Passed)];
    let why: Vec<String> = stuck(&r, "ana-ruiz", None).iter().map(|r| r.sentence(now)).collect();
    assert_eq!(why, ["You were asked to review it 14 days ago"]);

    // Approved, green, clean: nothing in the way.
    let mut ready = pull(132, ana(), ago(3));
    ready.reviewers.push(review(kenji(), ReviewStatus::Approved));
    ready.checks = vec![check("build", CheckState::Passed)];
    assert_eq!(stuck(&ready, "ana-ruiz", None), [Reason::Ready]);
    assert_eq!(next(&ready), Next::Merge);
    assert_eq!(review_state(&ready), ("Kenji approved".to_owned(), Tone::Good));

    // Waiting on reviewers, still running, changes asked, a draft.
    let mut waiting = pull(140, ana(), ago(4));
    waiting.reviewers.push(asked(kenji(), ago(3)));
    waiting.checks = vec![check("build", CheckState::Pending)];
    let why: Vec<String> = stuck(&waiting, "ana-ruiz", None).iter().map(|r| r.sentence(now)).collect();
    assert_eq!(why, ["Waiting on Kenji's review for 3 days", "1 check is still running"]);
    assert_eq!(next(&waiting), Next::Wait("Checks running"));
    let mut changes = pull(127, ana(), ago(6));
    changes.reviewers.push(review(lea(), ReviewStatus::ChangesRequested));
    assert_eq!(stuck(&changes, "ana-ruiz", None), [Reason::ChangesAsked("Lea".to_owned())]);
    assert_eq!(
        (next(&changes), review_state(&changes).0.as_str()),
        (Next::Wait("Changes asked"), "Lea asked for changes")
    );
    let mut draft = pull(133, ana(), ago(1));
    draft.summary.state = PullState::Draft;
    assert_eq!(stuck(&draft, "ana-ruiz", None), [Reason::Draft]);
    assert_eq!(next(&draft), Next::Wait("Draft"));
}

#[test]
fn checks_list_failed_first_and_vanish_without_ci() {
    let mut p = pull(1, ana(), ago(1));
    assert_eq!(checks(&p), None, "a repository with no CI has no checks card");
    p.checks = vec![check("build", CheckState::Passed), check("lint", CheckState::Passed)];
    assert!(checks(&p).is_some_and(|c| c.all_passed() && c.passed == 2));
    p.checks.push(check("e2e", CheckState::Failed));
    p.checks.push(check("docs", CheckState::Pending));
    let c = checks(&p).unwrap_or_else(|| panic!("checks"));
    let order: Vec<&str> = c.rows.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(order, ["e2e", "docs", "build", "lint"]);
    assert_eq!((c.failed, c.running, c.all_passed()), (1, 1, false));
}

#[test]
fn a_pull_request_with_no_review_says_so() {
    let mut p = pull(122, lea(), ago(14));
    assert!(!reviewed(&p));
    // The author's own comments aren't a review.
    p.conversation.push(reviewed_at(lea(), ago(13)));
    assert!(!reviewed(&p));
    p.conversation.push(reviewed_at(kenji(), ago(12)));
    assert!(reviewed(&p));
}

#[test]
fn ages_are_bare_numbers_coloured_by_how_stale_they_are() {
    let now = at(NOW);
    let cases = [
        (at("2026-11-11T14:59:30Z"), "now", Staleness::Fresh),
        (at("2026-11-11T14:56:00Z"), "4m", Staleness::Fresh),
        (at("2026-11-11T10:00:00Z"), "5h", Staleness::Fresh),
        (ago(1), "1d", Staleness::Fresh),
        (ago(3), "3d", Staleness::Settled),
        (ago(6), "6d", Staleness::Aging),
        (ago(14), "14d", Staleness::Stale),
        (ago(23), "23d", Staleness::Stale),
    ];
    for (since, label, staleness) in cases {
        assert_eq!((age(since, now).as_str(), Staleness::of(since, now)), (label, staleness), "{since}");
    }
    assert_eq!(age(at("2026-11-12T00:00:00Z"), now), "now", "a skewed clock reads as now");
}
