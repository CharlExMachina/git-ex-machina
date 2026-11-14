//! Every journey. Each one is a test (`tests/journeys.rs`), a video (`cargo xtask journey NAME
//! --video`) and a set of screenshots at its marks.

use gxm_app::keys;
use gxm_world::World;

use crate::{Journey, Press};

pub fn all() -> Vec<Journey> {
    vec![first_look(), push_and_pull(), shape_history(), crate::review::review_pull_request()]
}

pub fn find(name: &str) -> Option<Journey> {
    all().into_iter().find(|j| j.name == name)
}

fn mockup_world(w: &World) {
    gxm_world::fixtures::mockup(w);
}

/// Opening the app on a workspace: the history graph of the pinned repository, its commits
/// inspected with the mouse and the keyboard, a range selected for bisecting, uncommitted work,
/// the inspector's tabs, and a switch to another repository through the sidebar's filter.
pub fn first_look() -> Journey {
    Journey::new(
        "first_look",
        "Someone opens git-ex-machina on their workspace, reads atlas-web's history, inspects a few commits and jumps to another repository.",
        mockup_world,
    )
    // The pinned repository opens first, with its branch, how far it is ahead, and the graph.
    .see_in(keys::repo("atlas-web"), "atlas-web")
    .see_in(keys::repo("atlas-web"), "↑4")
    .see_in(keys::repo("payments-api"), "release/2.3 · studio")
    .see_in(keys::repo("infra-terraform"), "↓1")
    .see_in(keys::BRANCH_SUMMARY, "main ↑4 · 1,284 commits · fetched 2 minutes ago")
    .see_in(keys::PUSH, "Push 4")
    .see("Uncommitted changes")
    .see_in(keys::HISTORY, "3 files changed")
    .see("fix/avatar-cache")
    .see("origin/") // the remote badge: a muted `origin/` prefix, then `main`
    .see("v0.4.0")
    // Nothing is selected, so the inspector shows HEAD: the merge, what it brought in, and the
    // first file's first hunk.
    .see_in(keys::INSPECTOR, "HEAD OF MAIN")
    .see_in(keys::INSPECTOR_TITLE, "Merge feature/graph-virtualization")
    .see_in(keys::INSPECTOR_FILES, "4 files · +232 −93")
    .see_in(keys::changed_file("src/graph/legacy_layout.rs"), "D")
    .see_in(keys::HUNK, "fn layout_row(row: &Row)")
    .see_in(keys::HUNK, "draw_rail(x, y, H, lane.color);")
    .mark("history")
    // A click inspects another commit; the keyboard walks the history from there.
    .tap_text("Evict avatars by last paint, not insert")
    .see_in(keys::INSPECTOR, "COMMIT")
    .see_in(keys::INSPECTOR_TITLE, "Evict avatars by last paint, not insert")
    .see_in(keys::INSPECTOR, "Lea Brandt")
    .see_in(keys::INSPECTOR_FILES, "2 files · +81 −31")
    .see_in(keys::changed_file("src/avatars/metrics.rs"), "A")
    .mark("commit")
    // The Overview tab is for reading the commit: its whole message, who wrote and committed
    // it, its trailers, and the branches and tags that contain it.
    .see_in(keys::TAB_OVERVIEW, "Overview")
    .tap(keys::TAB_OVERVIEW)
    .gone(keys::INSPECTOR_FILES)
    .see_in(keys::INSPECTOR, "Avatars that are on screen stay cached")
    .see_in(keys::INSPECTOR, "fix/avatar-cache")
    .press(Press::DOWN)
    .see_in(keys::INSPECTOR_TITLE, "Spring the sidebar collapse")
    .see_in(keys::INSPECTOR, "Committer")
    .see_in(keys::INSPECTOR, "Kenji Mori")
    .see_in(keys::INSPECTOR, "Co-authored-by")
    .see_in(keys::INSPECTOR, "Lea Brandt")
    .see_in(keys::INSPECTOR, "#121")
    .mark("message")
    // Files is the changes; the message stays on its own tab.
    .tap(keys::TAB_FILES)
    .see_in(keys::changed_file("src/theme/brass.json"), "R")
    .not_see("collapses on the smooth spring")
    // Clicking the selected commit again lets go of it, and the inspector returns to HEAD.
    .tap_text("Spring the sidebar collapse")
    .see_in(keys::INSPECTOR, "HEAD OF MAIN")
    // A click and a shift-click select a linear range, ready to bisect.
    .tap_text("Tune brass for the light theme")
    .hold("shift")
    .tap_text("Read refs with gix instead of shelling out")
    .hold("")
    .see_in(keys::INSPECTOR, "SELECTION")
    .see_in(keys::INSPECTOR_TITLE, "3 commits")
    .see_in(keys::INSPECTOR, "A linear range: bisect or squash")
    .see_in(keys::INSPECTOR, "Split view remembers column widths")
    .mark("range")
    // The uncommitted row lists the work in progress, untracked files included.
    .tap(keys::WORKTREE_ROW)
    .see_in(keys::INSPECTOR, "WORKING TREE")
    .see_in(keys::INSPECTOR_TITLE, "Uncommitted changes")
    .see_in(keys::INSPECTOR_FILES, "NOT STAGED")
    .see_in(keys::INSPECTOR_FILES, "3 files")
    .see_in(keys::changed_file("src/toolbar/context.rs"), "A")
    .mark("uncommitted")
    // On the uncommitted row the same tab is the commit message being written: "Message".
    .see_in(keys::TAB_OVERVIEW, "Message")
    .tap(keys::TAB_OVERVIEW)
    .gone(keys::INSPECTOR_FILES)
    .tap(keys::TAB_FILES)
    .see_in(keys::INSPECTOR_FILES, "3 files")
    // Search jumps to the first commit that matches; Return goes on to the next.
    .tap(keys::COMMIT_SEARCH)
    .type_text("cache")
    .see_in(keys::INSPECTOR_TITLE, "Measure avatar cache pressure")
    .press(Press::ENTER)
    .see_in(keys::INSPECTOR_TITLE, "Fix avatar cache eviction")
    .mark("search")
    // The sidebar hides, and its toggle waits in the toolbar to bring it back.
    .tap(keys::SIDEBAR_TOGGLE)
    .gone(keys::repo("atlas-web"))
    .tap(keys::SIDEBAR_TOGGLE)
    .see_in(keys::repo("atlas-web"), "atlas-web")
    // The filter narrows the sidebar; picking a repository opens its history.
    .tap(keys::REPO_FILTER)
    .type_text("pay")
    .see_in(keys::repo("payments-api"), "payments-api")
    .gone(keys::repo("atlas-web"))
    .gone(keys::repo("dotfiles"))
    .mark("filtered")
    .tap(keys::repo("payments-api"))
    .see_in(keys::BRANCH_SUMMARY, "release/2.3 ↓4")
    .see_in(keys::INSPECTOR, "HEAD OF RELEASE/2.3")
    .see_in(keys::INSPECTOR_TITLE, "Update payments-api (5)")
    .not_see("Uncommitted changes")
    .mark("switched")
}

/// The mockup's workspace, with a local commit on `payments-api`, which is also behind its
/// remote: the two have diverged, so a fast-forward pull can't work there.
fn diverged_world(w: &World) {
    let m = gxm_world::fixtures::mockup(w);
    let r = w.open(&m.payments_api);
    r.commit(gxm_world::fixtures::LEA, w.ago(gxm_world::hours(1)), "Retry webhooks with backoff", &["src/webhooks.rs"]);
}

/// Syncing with the remotes: pushing what's ahead, pulling what's behind, fetching, and a
/// pull that git refuses, with its reason on screen.
pub fn push_and_pull() -> Journey {
    Journey::new(
        "push_and_pull",
        "Someone pushes their merged work, catches up a repository that fell behind, fetches, and sees why a pull on a diverged branch can't go through.",
        diverged_world,
    )
    // atlas-web is four ahead: Push sends them, and the badges go.
    .see_in(keys::PUSH, "Push 4")
    .see_in(keys::BRANCH_SUMMARY, "main ↑4")
    .tap(keys::PUSH)
    .wait_for(keys::BRANCH_SUMMARY, "main · 1,284 commits")
    .see_in(keys::PUSH, "Push")
    .not_see("↑4")
    .world_check("the remote's main is the local merge", |w| {
        let local = w.git(&w.root().join("code/atlas-web"), &["rev-parse", "main"]).trim().to_owned();
        let remote = w.remote_head("github.com", "studio", "atlas-web", "main");
        if local == remote { Ok(()) } else { Err(format!("remote main is {remote}, local main is {local}")) }
    })
    .mark("pushed")
    // infra-terraform is one behind: Pull fast-forwards it.
    .tap(keys::repo("infra-terraform"))
    .see_in(keys::BRANCH_SUMMARY, "main ↓1 · 7 commits")
    .tap(keys::PULL)
    .wait_for(keys::BRANCH_SUMMARY, "main · 7 commits")
    .see_in(keys::INSPECTOR_TITLE, "Update infra-terraform (6)")
    .not_see("↓1")
    .mark("pulled")
    // Fetch checks the remote; the summary says when.
    .tap(keys::FETCH)
    .wait_for(keys::BRANCH_SUMMARY, "fetched just now")
    .mark("fetched")
    // payments-api has a local commit and is behind: git won't fast-forward, and says why.
    .tap(keys::repo("payments-api"))
    .see_in(keys::BRANCH_SUMMARY, "release/2.3 ↑1 ↓4")
    .tap(keys::PULL)
    .wait_for(keys::OP_ERROR, "Pull failed")
    .see_in(keys::OP_ERROR, "fast-forward")
    .see_in(keys::BRANCH_SUMMARY, "release/2.3 ↑1 ↓4")
    .mark("refused")
}

/// Working on history from the toolbar: staging and committing the work in progress, then
/// cherry-picking, reverting and checking out from the graph, and setting work aside.
pub fn shape_history() -> Journey {
    Journey::new(
        "shape_history",
        "Someone commits their work in progress, cherry-picks a fix, reverts a commit, checks out a branch, and sets another repository's changes aside.",
        mockup_world,
    )
    // The uncommitted row offers staging; staging everything offers the commit.
    .tap(keys::WORKTREE_ROW)
    .see_in(keys::INSPECTOR, "3 files changed, nothing staged")
    .see_in(keys::STAGE_ALL, "Stage all")
    .gone(keys::FETCH)
    .tap(keys::STAGE_ALL)
    .wait_for(keys::INSPECTOR, "All 3 files staged, ready to commit")
    .see_in(keys::INSPECTOR_FILES, "STAGED")
    .see_in(keys::HISTORY, "3 files staged")
    .mark("staged")
    .tap(keys::UNSTAGE_ALL)
    .wait_for(keys::INSPECTOR, "3 files changed, nothing staged")
    .tap(keys::STAGE_ALL)
    .wait_for(keys::COMMIT, "Commit")
    // Commit without a message opens the message; with one, it commits.
    .tap(keys::COMMIT)
    .see_in(keys::INSPECTOR, "WORKING TREE")
    .type_text("Toolbar context for the selection")
    .mark("message")
    .tap(keys::COMMIT)
    .wait_for(keys::INSPECTOR_TITLE, "Toolbar context for the selection")
    .see_in(keys::BRANCH_SUMMARY, "main ↑5")
    .not_see("Uncommitted changes")
    .world_check("main's newest commit is the one just made", |w| {
        let subject = w.git(&w.root().join("code/atlas-web"), &["log", "-1", "--format=%s"]);
        if subject.trim() == "Toolbar context for the selection" { Ok(()) } else { Err(format!("HEAD is {subject:?}")) }
    })
    // One commit selected: the toolbar turns to what can be done with it.
    .tap_text("Evict avatars by last paint, not insert")
    .see_in(keys::CHECKOUT, "Checkout")
    .see_in(keys::REVERT, "Revert")
    .see_in(keys::BRANCH_FROM_HERE, "Branch from here")
    .mark("one")
    .tap(keys::CHERRY_PICK)
    .wait_for(keys::BRANCH_SUMMARY, "main ↑6")
    .tap_text("Bump rune to 0.9")
    .tap(keys::REVERT)
    .wait_for(keys::BRANCH_SUMMARY, "main ↑7")
    .see("Revert \"Bump rune to 0.9\"")
    // A line of commits can be bisected or squashed (both still to come) or cherry-picked.
    .tap_text("Tune brass for the light theme")
    .hold("shift")
    .tap_text("Read refs with gix instead of shelling out")
    .hold("")
    .see_in(keys::BISECT, "Bisect")
    .see_in(keys::SQUASH, "Squash")
    .see_in(keys::CHERRY_PICK, "Cherry-pick")
    .mark("range")
    // Checkout on a branch's tip checks the branch out.
    .tap_text("Virtualize commit rows past 10k")
    .tap(keys::CHECKOUT)
    .wait_for(keys::BRANCH_SUMMARY, "feature/graph-virtualization")
    .see_in(keys::repo("atlas-web"), "feature/graph-virtualization")
    // The commit made earlier sent the inspector back to its files.
    .see_in(keys::INSPECTOR_FILES, "CHANGED FILES")
    .mark("checked-out")
    // Discard all sets brand-tokens' changes aside in a stash.
    .tap(keys::repo("brand-tokens"))
    .tap(keys::WORKTREE_ROW)
    .see_in(keys::INSPECTOR, "12 files changed, nothing staged")
    .tap(keys::DISCARD_ALL)
    .wait_for(keys::INSPECTOR, "HEAD OF MAIN")
    .not_see("Uncommitted changes")
    // The stash's own commits stay out of the history.
    .not_see("Discarded in git-ex-machina")
    .see_in(keys::BRANCH_SUMMARY, "main · 6 commits")
    .world_check("the changes are in a stash, not gone", |w| {
        let stashes = w.git(&w.root().join("code/brand-tokens"), &["stash", "list"]);
        if stashes.contains(gxm_core::ops::DISCARD_STASH) { Ok(()) } else { Err(format!("stash list: {stashes:?}")) }
    })
    .mark("discarded")
}
