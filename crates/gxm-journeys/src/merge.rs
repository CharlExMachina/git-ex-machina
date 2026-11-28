//! Merging a pull request from the app: the person's own green pull request, squashed into
//! main on the provider, then fetched, so the squash commit shows in the history as #12's. The
//! repository is set to delete merged branches, on GitHub and here, so the branch goes from both.

use gxm_app::keys::{self, setting};
use gxm_world::World;
use gxm_world::hub::Hub;

use crate::Journey;

fn mockup_world(w: &World) {
    gxm_world::fixtures::mockup(w);
}

const SQUASHED: &str = "Shorter zsh prompt (#12)";

pub fn merge_pull_request() -> Journey {
    Journey::new(
        "merge_pull_request",
        "Someone opens their own green pull request from its commit, squashes and merges it, and sees the squash commit arrive in the history.",
        mockup_world,
    )
    // The app opens on Work; this journey is about the history, one click away.
    .tap(keys::VIEW_HISTORY)
    // First, dotfiles deletes merged branches: on GitHub, and the local copy too.
    .tap(keys::repo("dotfiles"))
    .tap(keys::SETTINGS)
    .tap(keys::settings_section("Branches"))
    .wait_for(setting("delete-branch-on-merge"), "On GitHub")
    .tap(setting("delete-branch-on-merge"))
    .tap(setting("delete-local-merged"))
    .wait_for(keys::SETTINGS_STATUS, "Saved")
    .tap(keys::SETTINGS_CLOSE)
    // The branch's tip belongs to #12, which is ready: every check green, no conflicts.
    .tap_text("Add the branch to the right prompt")
    .tap(keys::TAB_OVERVIEW)
    .wait_for(keys::PULL_REQUEST_ROW, "#12 Shorter zsh prompt")
    .tap(keys::SEE_PULL_REQUEST)
    .wait_for(keys::MERGE_BOX, "All checks passed")
    .see_in(keys::MERGE_BOX, "No conflicts with main")
    .see_in(keys::MERGE_BOX, "No approvals yet · ready to merge")
    .see_in(keys::PULL_BANNER, "#12 2 commits · Ana Ruiz")
    .not_see("failing")
    .mark("ready")
    // Squash and merge asks for the commit's message first, then merges.
    .tap(keys::MERGE_START)
    .see_in(keys::MERGE_BOX, "Confirm squash and merge")
    .see_in(keys::MERGE_HEADLINE, SQUASHED)
    .tap(keys::CANCEL_MERGE)
    .gone(keys::CONFIRM_MERGE)
    .tap(keys::SQUASH_AND_MERGE)
    .see_in(keys::MERGE_HEADLINE, SQUASHED)
    .mark("confirm")
    .tap(keys::CONFIRM_MERGE)
    .wait_for(keys::PULL_SUMMARY, "Squashed and merged into main")
    .see_in(keys::MERGE_BOX, "Merged into main")
    .see_in(keys::PULL_INSPECTOR, "Merged")
    .world_check("GitHub's main is one new commit holding the branch's changes", |w| {
        let commit = Hub::of(w).merged("personal", "dotfiles", 12).ok_or("#12 isn't merged")?;
        let bare = w.root().join("remotes/github.com/personal/dotfiles.git");
        let main = w.git(&bare, &["rev-parse", "main"]).trim().to_owned();
        let subject = w.git(&bare, &["log", "-1", "--format=%s", "main"]).trim().to_owned();
        let prompt = w.git(&bare, &["show", "main:zsh/prompt.zsh"]);
        if main == commit && subject == SQUASHED && prompt.contains("RPROMPT") {
            Ok(())
        } else {
            Err(format!("main {main} ({subject:?}), merge {commit}, prompt {prompt:?}"))
        }
    })
    // The app fetched, and read the pull requests again: the squash commit is #12's.
    .wait_for(keys::HISTORY, SQUASHED)
    .wait_for(keys::PULL_BANNER, "#12")
    .mark("merged")
    .tap(keys::CLOSE_PULL_REQUEST)
    .see_in(keys::BRANCH_SUMMARY, "main ↓1")
    .tap_text(SQUASHED)
    .wait_for(keys::PULL_REQUEST_ROW, "Merged")
    .see_in(keys::PULL_REQUEST_ROW, "#12 Shorter zsh prompt")
    .mark("history")
    .world_check("prompt is gone from GitHub and from this computer", |w| {
        let bare = w.root().join("remotes/github.com/personal/dotfiles.git");
        let remote = w.git(&bare, &["branch", "--format=%(refname:short)"]);
        let local = w.git(&w.code().join("dotfiles"), &["branch", "--format=%(refname:short)"]);
        if remote.lines().any(|b| b == "prompt") || local.lines().any(|b| b == "prompt") {
            Err(format!("remote {remote:?}, local {local:?}"))
        } else {
            Ok(())
        }
    })
}
