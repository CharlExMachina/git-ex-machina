//! The Work view, where the app opens: what's waiting, oldest first, in three sections, the
//! inspector's reasons, checks run again and a green pull request merged on the provider, the
//! checkout picked for Fetch, Pull, Push and History, every repository's work at once, and a
//! review opened from the list.

use gxm_app::keys;
use gxm_world::World;
use gxm_world::hub::Hub;

use crate::Journey;

fn mockup_world(w: &World) {
    gxm_world::fixtures::mockup(w);
}

const ATLAS: &str = "atlas-web";

/// What Copy agent brief puts on the clipboard for #118.
const BRIEF: &str = "# #118 Load avatars in parallel

https://github.com/studio/atlas-web/pull/118

Branch `perf/avatar-loading` into `main`, checked out in `~/worktrees/atlas-web/avatar-loading`.

## Why it's stuck

- Conflicts with main in src/avatars/cache.rs, since #129 landed
- 2 checks are failing
- You haven't answered 2 of Kenji's comments in 19 days

## Failing checks

- e2e / safari: 1 test failed
- e2e / firefox: 1 test failed

## Review comments to answer

- `src/avatars/loader.rs:12`
  - Kenji Mori: Should this fall back to one at a time when the pool is busy?
- `src/avatars/loader.rs:30`
  - Kenji Mori: The pool never shrinks. Drop idle workers after a minute.
";

pub fn work_queue() -> Journey {
    let pull = |n| keys::work_pull(ATLAS, n);
    let action = |n| keys::work_action(ATLAS, n);
    Journey::new(
        "work_queue",
        "Someone opens git-ex-machina to their open work, oldest first: reads why their oldest pull request is stuck, runs its failed checks again, merges the one that's ready, picks a worktree to work in, looks across every repository, and opens a review that's waiting on them.",
        mockup_world,
    )
    // Work is where the app opens: what's waiting on Ana, in three sections, each oldest
    // first, and nothing that's merged.
    .wait_for(pull(118), "Load avatars in parallel")
    .see_in(keys::VIEW_WORK, "Work")
    .see_in(keys::work_section("review"), "NEEDS YOUR REVIEW")
    .see_in(keys::work_section("yours"), "YOUR OPEN PULL REQUESTS")
    .see_in(keys::work_section("worktrees"), "WORKTREES WITH NO PR YET")
    .see_in(keys::WORK_SUMMARY, "atlas-web · 3 to review · 4 yours · 1 worktree with no PR yet")
    .not_see("Fix avatar cache eviction")
    // Reviews asked of her: how long ago, by whom, and no worktree yet.
    .see_in(pull(122), "14d")
    .see_in(pull(122), "Lea Brandt")
    .see_in(keys::work_tile(&pull(122)), "No worktree")
    .see_in(action(122), "Review")
    // Her own: the age since it opened, the review, the worktree and what's next.
    .see_in(pull(118), "23d")
    .see_in(pull(118), "Kenji reviewed")
    .see_in(keys::work_tile(&pull(118)), "avatar-loading")
    .see_in(action(118), "Resolve conflicts")
    .see_in(pull(127), "Lea asked for changes")
    .see_in(keys::work_tile(&pull(127)), "±3 files uncommitted")
    .see_in(action(127), "Changes asked")
    .see_in(pull(132), "Kenji approved")
    .see_in(action(132), "Merge")
    .see_in(action(133), "Draft")
    .see_in(keys::work_tile(&pull(133)), "clean · 2 unpushed")
    // A worktree with no pull request yet, edited minutes ago.
    .see_in(keys::work_worktree("sidebar-badges"), "4m")
    .see_in(keys::work_worktree("sidebar-badges"), "Colour the count by the oldest pull request")
    // The sidebar: every repository's open work at once, and each one's oldest.
    .see_in(keys::ALL_WORK, "All open work")
    .see_in(keys::ALL_WORK, "9")
    .see_in(keys::repo(ATLAS), "oldest PR 23d")
    .see_in(keys::repo("dotfiles"), "oldest PR 3h")
    // The inspector opens on the one stuck the longest, and says why.
    .see_in(keys::WORK_INSPECTOR, "STUCK THE LONGEST")
    .see_in(keys::WORK_TITLE, "#118 Load avatars in parallel")
    .see_in(keys::WORK_STUCK, "Conflicts with main in src/avatars/cache.rs, since #129 landed")
    .see_in(keys::WORK_STUCK, "2 checks are failing")
    .see_in(keys::WORK_STUCK, "You haven't answered 2 of Kenji's comments in 19 days")
    .see_in(keys::WORK_CHECKS, "2 failing · 3 passed")
    // Comments: the two that need her, the one waiting on Kenji dimmed; the resolved one gone.
    .see_in(keys::WORK_THREADS, "2 need you")
    .see_in(keys::WORK_THREADS, "The pool never shrinks.")
    .see_in(keys::WORK_THREADS, "You replied Oct 24 · waiting on Kenji")
    .not_see("Typo in the doc comment.")
    .see_in(keys::RESOLVE_CONFLICTS, "Resolve conflicts")
    .mark("work")
    // An agent can pick it up from a brief: where the work is and what's in its way.
    .scroll_to(keys::COPY_BRIEF)
    .tap(keys::COPY_BRIEF)
    .copied(BRIEF)
    .see_in(keys::TOASTS, "Agent brief copied")
    // One failed check runs again, then the rest of the failed ones.
    .scroll_to(keys::rerun("e2e / safari"))
    .tap(keys::rerun("e2e / safari"))
    .wait_for(keys::WORK_CHECKS, "1 failing · 1 running · 3 passed")
    .world_check("GitHub runs e2e / safari again", |w| {
        let reruns = Hub::of(w).reruns("studio", ATLAS, 118);
        if reruns == ["e2e / safari"] { Ok(()) } else { Err(format!("re-run: {reruns:?}")) }
    })
    .tap(keys::RERUN_FAILED)
    .wait_for(keys::WORK_CHECKS, "2 running · 3 passed")
    .world_check("GitHub runs the other failed check again, and only that", |w| {
        let reruns = Hub::of(w).reruns("studio", ATLAS, 118);
        if reruns == ["e2e / safari", "e2e / firefox"] { Ok(()) } else { Err(format!("re-run: {reruns:?}")) }
    })
    .mark("rerun")
    // A review asked of her two weeks ago: green, and no one has reviewed it.
    .tap(pull(122))
    .see_in(keys::WORK_INSPECTOR, "NEEDS YOUR REVIEW")
    .see_in(keys::WORK_STUCK, "You were asked to review it 14 days ago")
    .see_in(keys::WORK_CHECKS, "4 passed")
    .see_in(keys::WORK_CHECKS, "build · test · lint · e2e")
    .not_see("Re-run failed checks")
    .see_in(keys::WORK_THREADS, "This PR hasn't been reviewed yet.")
    .mark("review-asked")
    // #132 is approved and green: Merge squashes it on GitHub, and it leaves the list.
    .tap(action(132))
    .wait_for(keys::TOASTS, "Merged #132")
    .wait_for(keys::WORK_SUMMARY, "3 to review · 3 yours")
    .world_check("GitHub merged #132 into main", |w| {
        let commit = Hub::of(w).merged("studio", ATLAS, 132).ok_or("#132 isn't merged")?;
        let main = w.remote_head("github.com", "studio", ATLAS, "main");
        if main == commit { Ok(()) } else { Err(format!("main is {main}, the merge made {commit}")) }
    })
    .mark("merged")
    // The checkout menu: the main checkout, then every worktree with its branch and PR.
    .see_in(keys::CHECKOUT_PICKER, "main")
    .see_in(keys::CHECKOUT_PICKER, "~/code/atlas-web · ±3 · ↑4")
    .tap(keys::CHECKOUT_PICKER)
    .see_in(keys::CHECKOUT_MENU, "MAIN CHECKOUT")
    .see_in(keys::checkout_item(ATLAS), "~/code/atlas-web")
    .see_in(keys::checkout_item("fade-merged"), "graph/fade-merged · #127")
    .see_in(keys::checkout_item("avatar-loading"), "perf/avatar-loading · #118")
    .see_in(keys::checkout_item("sidebar-badges"), "±7 · ↑2")
    .see_in(keys::CHECKOUT_NEW_WORKTREE, "New worktree…")
    .see_in(keys::CHECKOUT_SWITCH_BRANCH, "Switch branch in main checkout…")
    .mark("checkouts")
    // Picking a worktree makes it the target: its row's tile is ringed, and History is its.
    .tap(keys::checkout_item("fade-merged"))
    .gone(keys::CHECKOUT_MENU)
    .see_in(keys::CHECKOUT_PICKER, "fade-merged")
    .see_in(keys::CHECKOUT_PICKER, "~/worktrees/atlas-web/fade-merged · ±3")
    .mark("picked")
    .tap(keys::VIEW_HISTORY)
    .wait_for(keys::BRANCH_SUMMARY, "graph/fade-merged")
    .see_in(keys::INSPECTOR, "HEAD OF GRAPH/FADE-MERGED")
    .see_in(keys::HISTORY, "3 files changed")
    .gone(keys::WORK_LIST)
    .mark("history")
    .tap(keys::VIEW_WORK)
    // Every repository at once: dotfiles' pull request joins atlas-web's.
    .tap(keys::ALL_WORK)
    .see_in(keys::WORK_SUMMARY, "All open work · 2 repositories")
    .see_in(keys::work_pull("dotfiles", 12), "Shorter zsh prompt")
    .see_in(keys::work_pull("dotfiles", 12), "dotfiles")
    .see_in(pull(118), "atlas-web")
    .mark("everywhere")
    // Review opens the pull request's review, on its history.
    .tap(action(131))
    .wait_for(keys::COCKPIT_BACK, "Pull request #131")
    .see_in(keys::REVIEW_PANEL, "YOUR REVIEW")
    .mark("review")
}
