//! Reviewing a pull request without leaving the app: from a commit to its pull request, the
//! pull request on the graph, the review cockpit, and back with the review sent.

use gxm_app::keys;
use gxm_core::forge::drafts;
use gxm_world::World;
use gxm_world::hub::{Hub, Verdict};

use crate::{Journey, Press};

/// atlas-web's pull requests on the world's GitHub: #131 open, #128 merged.
fn mockup_world(w: &World) {
    gxm_world::fixtures::mockup(w);
}

const PR: &str = "Evict avatars by last paint, not insert";
/// Its tip's history row, by key: once the inspector names the pull request, the title shows
/// there too. A world's commit ids never change.
const TIP: &str = "1cabd07cdeb1ce14a0568eb597113c22513572bc";
const CACHE: &str = "src/avatars/cache.rs";
const METRICS: &str = "src/avatars/metrics.rs";
const REPLY: &str = "Thanks, the bench in metrics.rs settles it.";
const CHANGE: &str = "Flush the metrics here too, or the eviction counts drift.";
const CHANGES_SUMMARY: &str = "One change before this goes in: flush the metrics on eviction.";
const NOTE: &str = "Nit: this loop could stop at the budget.";
const NOTES_SUMMARY: &str = "Two notes, nothing blocking.";
const SUMMARY: &str = "Evicting by last paint fixes the flicker. Ship it once first_look is green.";

/// The drafts file for #131, where the review waits until it's submitted.
fn draft_file(w: &World) -> std::path::PathBuf {
    drafts::path(w.env(), "github.com", "studio", "atlas-web", 131)
}

/// The reviews Ana sent, oldest first.
fn ana_reviews(w: &World) -> Vec<gxm_world::hub::SentReview> {
    Hub::of(w).reviews("studio", "atlas-web", 131).into_iter().filter(|r| r.author == "ana-ruiz").collect()
}

/// Lea's pull request, reviewed from the history: the person sees which pull request a commit
/// belongs to, opens it on the graph, reads its state, reviews its diff with a reply and a new
/// comment kept as drafts, approves, and sees the approval on the graph.
pub fn review_pull_request() -> Journey {
    Journey::new(
        "review_pull_request",
        "Someone sees that a commit belongs to a pull request, opens it on the graph, reviews its diff with comments, approves it and sees the approval land, without opening GitHub.",
        mockup_world,
    )
    // A commit in an open pull request: its Overview's details name the pull request once
    // GitHub answers.
    .tap(TIP)
    .tap(keys::TAB_OVERVIEW)
    .see_in(keys::INSPECTOR, "COMMIT")
    .see_in(keys::INSPECTOR, "DETAILS")
    .see_in(keys::INSPECTOR, "Nov 11, 2026, 10:00 · 5h")
    .see_in(keys::INSPECTOR, "Commit 1cabd07c")
    .see_in(keys::INSPECTOR, "Parent 8f566f4")
    .wait_for(keys::PULL_REQUEST_ROW, "#131")
    .see_in(keys::PULL_REQUEST_ROW, "Open")
    .see_in(keys::PULL_REQUEST_ROW, PR)
    .see_in(keys::SEE_PULL_REQUEST, "See pull request")
    .mark("commit")
    // HEAD is the merge commit that merged #128.
    .tap(TIP)
    .see_in(keys::INSPECTOR, "HEAD OF MAIN")
    .see_in(keys::INSPECTOR, "Parents a52c1c2 · c0e8db2")
    .wait_for(keys::PULL_REQUEST_ROW, "#128")
    .see_in(keys::PULL_REQUEST_ROW, "Merged")
    .see_in(keys::PULL_REQUEST_ROW, "Virtualize commit rows past 10k")
    .mark("merged")
    // A commit no pull request brought in has no pull request row.
    .tap_text("Bump rune to 0.9")
    .see_in(keys::INSPECTOR_TITLE, "Bump rune to 0.9")
    .gone(keys::PULL_REQUEST_ROW)
    .gone(keys::SEE_PULL_REQUEST)
    // The pull request on the graph: its lane lit, a row above its tip, the inspector as the PR.
    .tap(TIP)
    .wait_for(keys::SEE_PULL_REQUEST, "See pull request")
    .tap(keys::SEE_PULL_REQUEST)
    .wait_for(keys::PULL_INSPECTOR, "REVIEWERS")
    .see_in(keys::PULL_SUMMARY, "Pull request #131 · fix/avatar-cache → main")
    .see_in(keys::PULL_BANNER, "#131")
    .see_in(keys::PULL_BANNER, "2 commits · Lea Brandt · review requested from you")
    .see_in(keys::PULL_BANNER, "1 failing")
    .see_in(keys::PULL_BANNER, "1 approval")
    .see_in(keys::REVIEW, "Review")
    .see_in(keys::CHECK_OUT_PULL, "Check out")
    .see_in(keys::SQUASH_AND_MERGE, "Squash and merge")
    .see_in(keys::CLOSE_PULL_REQUEST, "Close pull request")
    .see_in(keys::PULL_INSPECTOR, "PULL REQUEST")
    .see_in(keys::PULL_INSPECTOR, PR)
    .see_in(keys::PULL_INSPECTOR, "Lea Brandt wants to merge 2 commits into main from fix/avatar-cache")
    .see_in(keys::PULL_INSPECTOR, "Evict by last_paint instead of insertion order")
    .see_in(keys::MERGE_BOX, "1 check failing")
    .see_in(keys::MERGE_BOX, "journeys · first_look failed at step 20")
    .see_in(keys::MERGE_BOX, "No conflicts with main")
    .see_in(keys::MERGE_BOX, "1 approval · your review is requested")
    .see_in(keys::reviewer("kenji-mori"), "Approved")
    .see_in(keys::reviewer("ana-ruiz"), "Ana Ruiz (you)")
    .see_in(keys::reviewer("ana-ruiz"), "Requested")
    .see_in(keys::TAB_CONVERSATION, "Conversation 4")
    .gone(keys::reviewer("lea-brandt")) // the author isn't her own reviewer
    .mark("pull-request")
    // Checks and the conversation, without a browser.
    .tap(keys::TAB_CHECKS)
    .see_in(keys::PULL_INSPECTOR, "journeys · first_look")
    .see_in(keys::PULL_INSPECTOR, "2m 03s")
    .see_in(keys::PULL_INSPECTOR, "unit tests")
    .mark("checks")
    .tap(keys::TAB_CONVERSATION)
    .see_in(keys::PULL_INSPECTOR, "Does pop_min stay O(log n) once the heap holds 10k avatars?")
    .see_in(keys::PULL_INSPECTOR, "Kenji Mori approved")
    .mark("conversation")
    .tap(keys::TAB_OVERVIEW)
    // The review cockpit: the PR's files and commits, the diff with its thread, the verdict.
    .tap(keys::REVIEW)
    .see_in(keys::COCKPIT_BACK, "Pull request #131")
    .see_in(keys::COCKPIT_FILES, "0 of 2 viewed")
    .see_in(keys::COCKPIT_FILES, "8f566f4 Measure avatar cache pressure")
    .see_in(keys::FILE_HEADER, CACHE)
    .see_in(keys::FILE_HEADER, "+30 −32")
    .see("2 comments · 1 unresolved")
    .see_in(keys::DIFF, "let oldest = self.by_last_paint.pop_min(); // 1")
    .see_in(keys::thread("PRRT_1"), "Kenji Mori · line 5 · 3h")
    .see_in(keys::thread("PRRT_1"), "Does pop_min stay O(log n) once the heap holds 10k avatars?")
    .see_in(keys::thread("PRRT_1"), "Yes, a binary heap keyed by paint time. There's a bench in metrics.rs.")
    .see_in(keys::REVIEW_PANEL, "YOUR REVIEW")
    // The other file, read and ticked off: it leaves the list for the folded Viewed shelf, and
    // the file still to view opens.
    .tap(keys::pull_file(METRICS))
    .see_in(keys::FILE_HEADER, METRICS)
    .tap(keys::VIEWED)
    .see_in(keys::COCKPIT_FILES, "1 of 2 viewed")
    .see_in(keys::FILE_HEADER, CACHE)
    .gone(keys::pull_file(METRICS))
    .see_in(keys::VIEWED_SHELF, "1")
    // Open, the shelf holds it; unticked, it comes back to the list, and the shelf goes.
    .tap(keys::VIEWED_SHELF)
    .tap(keys::pull_file(METRICS))
    .see_in(keys::FILE_HEADER, METRICS)
    .tap(keys::VIEWED)
    .see_in(keys::COCKPIT_FILES, "0 of 2 viewed")
    .gone(keys::VIEWED_SHELF)
    .see_in(keys::pull_file(METRICS), "metrics.rs")
    .tap(keys::VIEWED)
    .see_in(keys::VIEWED_SHELF, "1")
    .see_in(keys::pull_file(METRICS), "metrics.rs")
    .see_in(keys::FILE_HEADER, CACHE)
    .mark("viewed")
    .tap(keys::VERDICT_APPROVE)
    .gone(keys::reviewer("lea-brandt"))
    .mark("cockpit")
    // Split shows the lines as they were beside the lines as they are.
    .tap(keys::DIFF_SPLIT)
    .see_in(keys::diff_line(CACHE, false, 5), "// avatar cache 5")
    .see_in(keys::diff_line(CACHE, true, 5), "let oldest = self.by_last_paint.pop_min(); // 1")
    .mark("split")
    .tap(keys::DIFF_UNIFIED)
    // One commit alone: Lea's first, which only measured.
    .tap(keys::pull_commit("8f566f4"))
    .see_in(keys::FILE_HEADER, "+3 −1")
    .see_in(keys::DIFF, "counters.record(hit, miss, evicted);")
    .see_in(keys::COCKPIT_FILES, "8f566f4 · show all")
    .gone(keys::thread("PRRT_1"))
    .mark("one-commit")
    .tap(keys::PULL_COMMITS_ALL)
    .see_in(keys::FILE_HEADER, "+30 −32")
    .see_in(keys::thread("PRRT_1"), "Kenji Mori")
    // First review: changes requested, with a comment on line 6, kept as a draft on this
    // machine until the review goes.
    .scroll_to(keys::diff_line(CACHE, true, 6))
    .tap(keys::diff_line(CACHE, true, 6))
    .type_text(CHANGE)
    .press(Press::ENTER)
    .see_in(keys::DIFF, "Ana Ruiz · line 6 · Pending")
    .see_in(keys::DIFF, CHANGE)
    .see_in(keys::REVIEW_SUMMARY, "(1 pending comment will be sent with it)")
    .world_check("the draft is saved locally, not sent", |w| {
        let saved = std::fs::read_to_string(draft_file(w)).map_err(|e| format!("no drafts file: {e}"))?;
        if !saved.contains(CHANGE) {
            return Err(format!("drafts file: {saved}"));
        }
        let sent = Hub::of(w).reviews("studio", "atlas-web", 131);
        if sent.iter().any(|r| r.author == "ana-ruiz") { Err("a review was sent already".to_owned()) } else { Ok(()) }
    })
    .mark("drafts")
    // Leaving the cockpit keeps the draft.
    .tap(keys::COCKPIT_BACK)
    .see_in(keys::PULL_SUMMARY, "Pull request #131")
    .tap(keys::REVIEW)
    .see_in(keys::DIFF, CHANGE)
    .see_in(keys::COCKPIT_FILES, "1 of 2 viewed")
    .tap(keys::REVIEW_SUMMARY)
    .type_text(CHANGES_SUMMARY)
    .tap(keys::VERDICT_REQUEST_CHANGES)
    .mark("request-changes")
    .tap(keys::SUBMIT_REVIEW)
    .wait_for(keys::PULL_SUMMARY, "Your request for changes was sent to GitHub")
    .see_in(keys::PULL_BANNER, "2 commits · Lea Brandt · you requested changes")
    .see_in(keys::PULL_BANNER, "1 approval")
    .see_in(keys::reviewer("ana-ruiz"), "Changes requested")
    .see_in(keys::TAB_CONVERSATION, "Conversation 5")
    .see_in(keys::REVIEW, "Reviewed")
    .world_check("GitHub got a request for changes with the line comment", |w| {
        let sent = ana_reviews(w);
        let [review] = sent.as_slice() else { return Err(format!("{} reviews from Ana", sent.len())) };
        let on_line = review.comments.iter().any(|c| c.body == CHANGE && c.path == CACHE && c.line == Some(6));
        if review.verdict == Verdict::RequestChanges && review.body == CHANGES_SUMMARY && on_line {
            Ok(())
        } else {
            Err(format!("{review:?}"))
        }
    })
    .world_check("the sent draft is gone, the files marked viewed stay", |w| {
        let saved = std::fs::read_to_string(draft_file(w)).unwrap_or_default();
        if saved.contains(CHANGE) || !saved.contains(METRICS) { Err(format!("drafts file: {saved}")) } else { Ok(()) }
    })
    .mark("changes-requested")
    // Second review, a comment only: Ana's line comment is on its line now, sent; she replies to
    // Kenji and notes something on line 10.
    .tap(keys::REVIEW)
    .scroll_to(keys::thread("PRRT_2"))
    .see_in(keys::thread("PRRT_2"), "Ana Ruiz · line 6 · now")
    .see_in(keys::thread("PRRT_2"), CHANGE)
    .not_see("Pending")
    .see("3 comments · 2 unresolved")
    .mark("line-comment")
    .scroll_to(keys::reply("PRRT_1"))
    .tap(keys::reply("PRRT_1"))
    .type_text(REPLY)
    .press(Press::ENTER)
    .see_in(keys::thread("PRRT_1"), REPLY)
    .see_in(keys::thread("PRRT_1"), "Pending")
    .scroll_to(keys::diff_line(CACHE, true, 10))
    .tap(keys::diff_line(CACHE, true, 10))
    .type_text(NOTE)
    .press(Press::ENTER)
    .see_in(keys::DIFF, NOTE)
    .see_in(keys::REVIEW_SUMMARY, "(2 pending comments will be sent with it)")
    .tap(keys::REVIEW_SUMMARY)
    .type_text(NOTES_SUMMARY)
    .tap(keys::VERDICT_COMMENT)
    .mark("comment")
    .tap(keys::SUBMIT_REVIEW)
    .wait_for(keys::PULL_SUMMARY, "Your review was sent to GitHub")
    .see_in(keys::reviewer("ana-ruiz"), "Changes requested") // a comment doesn't undo a request
    .see_in(keys::TAB_CONVERSATION, "Conversation 6")
    .world_check("GitHub got a comment review with the reply and the line 10 comment", |w| {
        let sent = ana_reviews(w);
        let Some(review) = sent.get(1) else { return Err(format!("{} reviews from Ana", sent.len())) };
        let on_line = review.comments.iter().any(|c| c.body == NOTE && c.line == Some(10));
        let reply = review.comments.iter().any(|c| c.body == REPLY && c.thread.as_deref() == Some("PRRT_1"));
        if review.verdict == Verdict::Comment && on_line && reply { Ok(()) } else { Err(format!("{review:?}")) }
    })
    .mark("commented")
    // Third review: Lea's answers are in, so Ana approves.
    .tap(keys::REVIEW)
    .tap(keys::REVIEW_SUMMARY)
    .type_text(SUMMARY)
    .tap(keys::VERDICT_APPROVE)
    .mark("verdict")
    .tap(keys::SUBMIT_REVIEW)
    .wait_for(keys::PULL_SUMMARY, "Your approval was sent to GitHub")
    .see_in(keys::PULL_BANNER, "2 commits · Lea Brandt · you approved")
    .see_in(keys::PULL_BANNER, "2 approvals")
    .see_in(keys::reviewer("ana-ruiz"), "Approved")
    .see_in(keys::MERGE_BOX, "2 approvals · ready once checks pass")
    .see_in(keys::TAB_CONVERSATION, "Conversation 7")
    .world_check("GitHub got the approval with its summary", |w| {
        let sent = ana_reviews(w);
        match sent.last() {
            Some(r) if sent.len() == 3 && r.verdict == Verdict::Approve && r.body == SUMMARY => Ok(()),
            other => Err(format!("{} reviews from Ana, the last {other:?}", sent.len())),
        }
    })
    .mark("reviewed")
    // The conversation has all three reviews, and the comments on their lines.
    .tap(keys::TAB_CONVERSATION)
    .see_in(keys::PULL_INSPECTOR, "Ana Ruiz requested changes")
    .see_in(keys::PULL_INSPECTOR, "src/avatars/cache.rs:6")
    .see_in(keys::PULL_INSPECTOR, "src/avatars/cache.rs:10")
    .see_in(keys::PULL_INSPECTOR, "Ana Ruiz approved")
    .mark("conversation-after")
    .tap(keys::TAB_OVERVIEW)
    // Back to the plain history.
    .tap(keys::CLOSE_PULL_REQUEST)
    .gone(keys::PULL_BANNER)
    .see_in(keys::BRANCH_SUMMARY, "main ↑4 · 1,297 commits")
    .see_in(keys::INSPECTOR_TITLE, PR)
    .mark("history")
    // A repository on a provider that isn't connected yet says so where the pull request goes.
    .tap(keys::repo("payments-api"))
    .see_in(keys::PULL_NOTICE, "Pull requests on Bitbucket are coming next.")
    .gone(keys::SEE_PULL_REQUEST)
    .mark("not-connected")
}
