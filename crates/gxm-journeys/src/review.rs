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
const COMMENT: &str = "Could this flush the metrics too?";
const SUMMARY: &str = "Evicting by last paint fixes the flicker. Ship it once first_look is green.";

/// The drafts file for #131, where the review waits until it's submitted.
fn draft_file(w: &World) -> std::path::PathBuf {
    drafts::path(w.env(), "github.com", "studio", "atlas-web", 131)
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
    // A commit in an open pull request: labelled rows, then the pull request once GitHub answers.
    .tap(TIP)
    .see_in(keys::INSPECTOR, "COMMIT")
    .see_in(keys::INSPECTOR, "Author")
    .see_in(keys::INSPECTOR, "Nov 11, 2026, 10:00 · 5 hours ago")
    .see_in(keys::INSPECTOR, "1cabd07c")
    .see_in(keys::INSPECTOR, "Parent 8f566f4")
    .wait_for(keys::PULL_REQUEST_ROW, "#131")
    .see_in(keys::PULL_REQUEST_ROW, "Open")
    .see_in(keys::PULL_REQUEST_ROW, PR)
    .see_in(keys::SEE_PULL_REQUEST, "See pull request")
    .mark("commit")
    // HEAD is the merge commit that merged #128.
    .tap(TIP)
    .see_in(keys::INSPECTOR, "HEAD OF MAIN")
    .see_in(keys::INSPECTOR, "Parents bd3b72a · c0e8db2")
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
    // The other file, read and ticked off.
    .tap(keys::pull_file(METRICS))
    .see_in(keys::FILE_HEADER, METRICS)
    .tap(keys::VIEWED)
    .see_in(keys::COCKPIT_FILES, "1 of 2 viewed")
    .tap(keys::pull_file(CACHE))
    .tap(keys::VERDICT_APPROVE)
    .gone(keys::reviewer("lea-brandt"))
    .mark("cockpit")
    // Split shows the lines as they were beside the lines as they are.
    .tap(keys::DIFF_SPLIT)
    .see_in(keys::diff_line(CACHE, false, 5), "// avatar cache 5")
    .see_in(keys::diff_line(CACHE, true, 5), "let oldest = self.by_last_paint.pop_min(); // 1")
    .mark("split")
    .tap(keys::DIFF_UNIFIED)
    // A reply and a new comment, kept as drafts on this machine until the review goes.
    .scroll_to(keys::reply("PRRT_1"))
    .tap(keys::reply("PRRT_1"))
    .type_text(REPLY)
    .press(Press::ENTER)
    .see_in(keys::thread("PRRT_1"), REPLY)
    .see_in(keys::thread("PRRT_1"), "Pending")
    .scroll_to(keys::diff_line(CACHE, true, 6))
    .tap(keys::diff_line(CACHE, true, 6))
    .type_text(COMMENT)
    .press(Press::ENTER)
    .see_in(keys::DIFF, COMMENT)
    .see_in(keys::REVIEW_SUMMARY, "(2 pending comments will be sent with it)")
    .world_check("the drafts are saved locally, not sent", |w| {
        let saved = std::fs::read_to_string(draft_file(w)).map_err(|e| format!("no drafts file: {e}"))?;
        if !saved.contains(REPLY) || !saved.contains(COMMENT) {
            return Err(format!("drafts file: {saved}"));
        }
        let sent = Hub::of(w).reviews("studio", "atlas-web", 131);
        if sent.iter().any(|r| r.author == "ana-ruiz") { Err("a review was sent already".to_owned()) } else { Ok(()) }
    })
    .mark("drafts")
    // Leaving the cockpit keeps the drafts.
    .tap(keys::COCKPIT_BACK)
    .see_in(keys::PULL_SUMMARY, "Pull request #131")
    .tap(keys::REVIEW)
    .see_in(keys::DIFF, COMMENT)
    .see_in(keys::COCKPIT_FILES, "1 of 2 viewed")
    // The verdict and the summary, sent as one review.
    .tap(keys::REVIEW_SUMMARY)
    .type_text(SUMMARY)
    .mark("verdict")
    .tap(keys::SUBMIT_REVIEW)
    .wait_for(keys::PULL_SUMMARY, "Your approval was sent to GitHub")
    .see_in(keys::PULL_BANNER, "2 commits · Lea Brandt · you approved")
    .see_in(keys::PULL_BANNER, "2 approvals")
    .see_in(keys::reviewer("ana-ruiz"), "Approved")
    .see_in(keys::MERGE_BOX, "2 approvals · ready once checks pass")
    .see_in(keys::TAB_CONVERSATION, "Conversation 5")
    .see_in(keys::REVIEW, "Reviewed")
    .world_check("GitHub got one approving review with the summary, the comment and the reply", |w| {
        let sent: Vec<_> =
            Hub::of(w).reviews("studio", "atlas-web", 131).into_iter().filter(|r| r.author == "ana-ruiz").collect();
        let [review] = sent.as_slice() else { return Err(format!("{} reviews from Ana", sent.len())) };
        let comment = review.comments.iter().any(|c| c.body == COMMENT && c.path == CACHE && c.line == Some(6));
        let reply = review.comments.iter().any(|c| c.body == REPLY && c.thread.as_deref() == Some("PRRT_1"));
        if review.verdict == Verdict::Approve && review.body == SUMMARY && comment && reply {
            Ok(())
        } else {
            Err(format!("{review:?}"))
        }
    })
    .world_check("the sent drafts are gone, the files marked viewed stay", |w| {
        let saved = std::fs::read_to_string(draft_file(w)).unwrap_or_default();
        if saved.contains(REPLY) || saved.contains(COMMENT) || !saved.contains(METRICS) {
            return Err(format!("drafts file: {saved}"));
        }
        Ok(())
    })
    .mark("reviewed")
    // Back to the plain history.
    .tap(keys::CLOSE_PULL_REQUEST)
    .gone(keys::PULL_BANNER)
    .see_in(keys::BRANCH_SUMMARY, "main ↑4 · 1,284 commits")
    .see_in(keys::INSPECTOR_TITLE, PR)
    .mark("history")
}
