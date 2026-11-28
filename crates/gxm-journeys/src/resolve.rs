//! Resolving a pull request's conflicts with its base: the Work row's Resolve conflicts merges
//! main into the pull request's worktree and opens every conflict in a window of its own. The
//! person aborts once and the worktree is as it was; opens it again, walks the conflicts by
//! keyboard and by the sidebar, settles two by hand, hands the rest to their coding agent, then
//! commits the merge and pushes it.

use gxm_app::keys;
use gxm_core::git;
use gxm_world::World;
use gxm_world::fixtures::agents::{self, Agent};

use crate::{Journey, Press};

fn mockup_world(w: &World) {
    gxm_world::fixtures::mockup(w);
}

const ATLAS: &str = "atlas-web";
const BRANCH: &str = "perf/avatar-loading";

/// #118's worktree.
fn worktree(w: &World) -> std::path::PathBuf {
    gxm_world::fixtures::worktrees_dir(w).join("avatar-loading")
}

/// The worktree has no merge under way, nothing uncommitted, and HEAD where GitHub has the
/// branch.
fn untouched(w: &World) -> Result<(), String> {
    let wt = worktree(w);
    let status = w.git(&wt, &["status", "--porcelain"]);
    if !status.trim().is_empty() {
        return Err(format!("the worktree has changes:\n{status}"));
    }
    if git::run(w.env(), &wt, &["rev-parse", "--quiet", "--verify", "MERGE_HEAD"]).is_ok() {
        return Err("a merge is still under way".to_owned());
    }
    let head = w.git(&wt, &["rev-parse", "HEAD"]);
    let pushed = w.remote_head("github.com", "studio", ATLAS, BRANCH);
    if head.trim() == pushed.trim() { Ok(()) } else { Err(format!("HEAD is {head}, GitHub has {pushed}")) }
}

/// A file in the worktree, as it is now.
fn read(w: &World, file: &str) -> Result<String, String> {
    std::fs::read_to_string(worktree(w).join(file)).map_err(|e| format!("{file}: {e}"))
}

pub fn resolve_conflicts() -> Journey {
    let pull = keys::work_pull(ATLAS, 118);
    let action = keys::work_action(ATLAS, 118);
    Journey::new(
        "resolve_conflicts",
        "Someone whose pull request conflicts with main opens every conflict in one window: they abort once and find the worktree untouched, then walk the conflicts, settle two themselves, ask their coding agent to solve the rest, review what it wrote, and commit the merge and push it.",
        mockup_world,
    )
    .wait_for(pull.clone(), "Load avatars in parallel")
    .see_in(action.clone(), "Resolve conflicts")
    // The Work view's toolbar opens Settings too, where Agents lives.
    .see_in(keys::SETTINGS, "Settings")
    // Resolve conflicts merges main into the worktree and opens every conflict at once.
    .tap(action.clone())
    .wait_for(keys::RESOLVE_FILES, "src/theme/motion.rs")
    .see_in(keys::RESOLVE_TITLE, "Resolve conflicts · #118 Load avatars in parallel")
    .see_in(keys::RESOLVE_SUBTITLE, "Merging main into perf/avatar-loading · worktree avatar-loading")
    .see_in(keys::RESOLVE_PROGRESS, "0 of 4 resolved")
    // The sidebar: each file with what's left in it, each conflict at its line and in its
    // function, where there is one.
    .see_in(keys::resolve_file("src/avatars/cache.rs"), "1 left")
    .see_in(keys::resolve_conflict(0), ":49")
    .see_in(keys::resolve_conflict(0), "fn load_missing")
    .see_in(keys::resolve_conflict(1), ":40")
    .see_in(keys::resolve_conflict(1), "fn collapse")
    .see_in(keys::resolve_conflict(2), ":5")
    .see_in(keys::resolve_conflict(3), ":10")
    .see_in(keys::resolve_conflict(3), "mod springs")
    .see_in(keys::RESOLVE_FILES, "⌥↓ next conflict · ⌥↑ previous")
    // The first conflict: both sides, and why main changed it.
    .see_in(keys::RESOLVE_NAV, "Conflict 1 of 4")
    .see_in(keys::RESOLVE_PLACE, "src/avatars/cache.rs:49")
    .see_in(keys::RESOLVE_YOURS, "Yours · perf/avatar-loading")
    .see_in(keys::RESOLVE_YOURS, "let loaded = pool.load_all(missing); // 49")
    .see_in(keys::RESOLVE_THEIRS, "main · #129 Fix avatar cache eviction · Lea Brandt")
    .see_in(keys::RESOLVE_THEIRS, "cache.evict_on_release(); // 1")
    .see_in(keys::RESOLVE_WHY, "Why main changed this: #129 Fix avatar cache eviction. Released as v0.4.0. Merged Nov 9.")
    .see_in(keys::RESOLVE_LEFT, "4 conflicts left")
    .see_in(keys::RESOLVE_MINIMAP, "4 conflicts")
    .mark("conflicts")
    // Abort puts the worktree back exactly as it was.
    .see_in(keys::RESOLVE_FOOTER, "puts the worktree back exactly as it was")
    .tap(keys::ABORT_MERGE)
    .wait_for(keys::TOASTS, "Merge aborted")
    .gone(keys::RESOLVE_WINDOW)
    .world_check("the worktree is exactly as it was before the merge", untouched)
    .see_in(keys::work_tile(&pull), "avatar-loading")
    .mark("aborted")
    // Again, for real this time.
    .tap(action)
    .wait_for(keys::RESOLVE_FILES, "src/theme/motion.rs")
    // ⌥↓ and ⌥↑ step through every conflict, across files; the sidebar jumps.
    .press(Press::alt("Down"))
    .see_in(keys::RESOLVE_NAV, "Conflict 2 of 4")
    .see_in(keys::RESOLVE_PLACE, "src/sidebar.rs:40")
    .press(Press::alt("Down"))
    .see_in(keys::RESOLVE_PLACE, "src/theme/brass.json:5")
    .press(Press::alt("Up"))
    .see_in(keys::RESOLVE_NAV, "Conflict 2 of 4")
    .tap(keys::resolve_conflict(3))
    .see_in(keys::RESOLVE_NAV, "Conflict 4 of 4")
    .see_in(keys::RESOLVE_PLACE, "src/theme/motion.rs:10")
    .see_in(keys::RESOLVE_WHY, "Why main changed this: Spring the sidebar collapse")
    .see_in(keys::RESOLVE_WHY, "no pull request")
    .mark("jumped")
    // Main's eviction fix wins the first conflict.
    .tap(keys::resolve_conflict(0))
    .tap(keys::USE_MAIN)
    .see_in(keys::RESOLVE_PROGRESS, "1 of 4 resolved")
    .see_in(keys::resolve_file("src/avatars/cache.rs"), "done")
    .see_in(keys::RESOLVE_RESULT, "cache.evict_on_release(); // 2")
    .world_check("cache.rs takes main's lines, with no conflict markers left", |w| {
        let text = read(w, "src/avatars/cache.rs")?;
        if text.contains("<<<<<<<") || text.contains("pool.load_all") || !text.contains("evict_on_release(); // 2") {
            return Err(format!("cache.rs holds:\n{text}"));
        }
        Ok(())
    })
    // The motion one, written by hand.
    .tap(keys::resolve_conflict(3))
    .tap(keys::RESOLVE_RESULT)
    .press(Press::cmd("a"))
    .type_text("pub const AVATAR_FADE: Spring = SMOOTH;")
    .see_in(keys::RESOLVE_EDITED, "edited")
    .tap(keys::MARK_RESOLVED)
    .see_in(keys::RESOLVE_PROGRESS, "2 of 4 resolved")
    .see_in(keys::RESOLVE_NAV, "Conflict 2 of 4")
    .world_check("motion.rs holds what was written by hand", |w| {
        let text = read(w, "src/theme/motion.rs")?;
        if text.contains("<<<<<<<") || !text.contains("\npub const AVATAR_FADE: Spring = SMOOTH;\n// motion 12") {
            return Err(format!("motion.rs holds:\n{text}"));
        }
        Ok(())
    })
    .see_in(keys::RESOLVE_LEFT, "2 conflicts left")
    .mark("resolving")
    // No agent on this computer yet: the button says where to add one.
    .see_in(keys::ASK_AGENT, "No agent harness yet. Add Claude Code or Codex in Settings › Agents.")
    .tap(keys::ASK_AGENT)
    .gone(keys::AGENT_STATUS)
    // Claude Code, installed and signed in from a terminal, is found without asking.
    .outside("installs Claude Code and signs in, in a terminal", |w| agents::claude(w, true, Agent::Resolves))
    .wait_for(keys::ASK_AGENT, "Ask Claude Code to solve 2 conflicts")
    // A moment to read it; the video's replay, which installs Claude Code by the clock, gets
    // the same moment to find it.
    .wait(1.5)
    .tap(keys::ASK_AGENT)
    .wait_for(keys::AGENT_STATUS, "Claude Code is resolving 2 conflicts")
    .wait_for(keys::AGENT_STATUS, "cargo check")
    .see_in(keys::AGENT_STATUS, "started")
    .see_in(keys::AGENT_STOP, "Stop")
    .mark("agent-working")
    .outside("Claude Code's check finishes", agents::finish)
    .wait_for(keys::AGENT_STATUS, "Claude Code finished")
    .see_in(keys::RESOLVE_PROGRESS, "4 of 4 resolved")
    .see_in(keys::RESOLVE_LEFT, "No conflicts left")
    // What it wrote, for review.
    .tap(keys::resolve_conflict(1))
    .see_in(keys::resolve_conflict(1), "agent")
    .see_in(keys::RESOLVE_RESULT, "avatars.prefetch(self.visible_rows()); // 44")
    .see_in(keys::RESOLVE_RESULT, "width.animate_to(target).motion(motion::SMOOTH); // 1")
    .mark("agent-done")
    // Commit merge & push: the merge goes to GitHub, and the window closes.
    .tap(keys::COMMIT_MERGE)
    .wait_for(keys::TOASTS, "Merged main into perf/avatar-loading and pushed it")
    .gone(keys::RESOLVE_WINDOW)
    .world_check("GitHub's perf/avatar-loading is a merge of main, with no conflict markers", |w| {
        let head = w.remote_head("github.com", "studio", ATLAS, BRANCH);
        let remote = w.remote("github.com", "studio", ATLAS);
        let parents = w.git(&remote, &["rev-list", "--parents", "-n", "1", head.trim()]);
        let main = w.remote_head("github.com", "studio", ATLAS, "main");
        if parents.split_whitespace().nth(2) != Some(main.trim()) {
            return Err(format!("{BRANCH} is {parents}, not a merge of main ({main})"));
        }
        // git grep finds nothing: it says so by failing.
        match git::run(w.env(), &remote, &["grep", "-n", "-e", "^<<<<<<<", "-e", "^>>>>>>>", head.trim(), "--", "."]) {
            Ok(marked) => Err(format!("conflict markers pushed:\n{marked}")),
            Err(_) => Ok(()),
        }
    })
    // Back on Work, #118 no longer conflicts.
    .wait_for(keys::work_tile(&pull), "up to date")
    .not_see("Resolve conflicts")
    .mark("pushed")
}
