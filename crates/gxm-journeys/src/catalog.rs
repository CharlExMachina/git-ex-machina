//! Every journey. Each one is a test (`tests/journeys.rs`), a video (`cargo xtask journey NAME
//! --video`) and a set of screenshots at its marks.

use gxm_app::keys;
use gxm_world::World;

use crate::{Journey, Press};

pub fn all() -> Vec<Journey> {
    vec![
        get_started(),
        sign_in(),
        first_look(),
        push_and_pull(),
        shape_history(),
        crate::review::review_pull_request(),
        crate::merge::merge_pull_request(),
        crate::update::update_app(),
    ]
}

pub fn find(name: &str) -> Option<Journey> {
    all().into_iter().find(|j| j.name == name)
}

fn mockup_world(w: &World) {
    gxm_world::fixtures::mockup(w);
}

/// A first run: nothing in the workspace yet. On disk, `~/code/lantern` is a Godot project
/// that was never put in git (its `.godot/` cache and binary assets included; tracking those
/// with LFS needs `git-lfs` installed, as CI's runners have it), and `~/code/field-notes` is on a
/// `drafts` branch that was never pushed. `studio/site` is only on GitHub.
fn first_run_world(w: &World) {
    use gxm_world::fixtures::{ANA, KENJI, LEA};
    use gxm_world::{days, hours};
    // A first run: not signed in to GitHub either.
    w.sign_out();
    let notes = w.repo("github.com", "personal", "field-notes");
    notes.commit(ANA, w.ago(days(9)), "Start the field notes", &["README.md"]);
    notes.commit(KENJI, w.ago(days(6)), "Notes from the Lisbon workshop", &["2026/lisbon.md"]);
    notes.push("main");
    notes.branch("drafts");
    notes.commit(ANA, w.ago(days(2)), "Draft the review checklist", &["drafts/checklist.md"]);
    notes.commit(ANA, w.ago(hours(3)), "Sketch the onboarding notes", &["drafts/onboarding.md"]);
    let lantern = w.code().join("lantern");
    for (file, text) in [
        (
            "project.godot",
            "config_version=5\n\n[application]\nconfig/name=\"Lantern\"\nrun/main_scene=\"res://main.tscn\"\n",
        ),
        ("main.tscn", "[gd_scene format=3]\n\n[node name=\"Main\" type=\"Node2D\"]\n"),
        ("player.gd", "extends CharacterBody2D\n\nconst SPEED = 220.0\n"),
        (".godot/editor/main.tscn-folding.cfg", "[folding]\n"),
        (".godot/imported/icon.svg-218a8f2b.ctex", "GST2\n"),
    ] {
        let path = lantern.join(file);
        std::fs::create_dir_all(path.parent().unwrap_or(&lantern))
            .unwrap_or_else(|e| panic!("make {file}'s folder: {e}"));
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("write {file}: {e}"));
    }
    // Assets: binary, as their headers make them.
    std::fs::create_dir_all(lantern.join("music")).unwrap_or_else(|e| panic!("make music/: {e}"));
    for (file, bytes) in
        [("icon.png", &b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"[..]), ("music/theme.ogg", b"OggS\x00\x02")]
    {
        std::fs::write(lantern.join(file), bytes).unwrap_or_else(|e| panic!("write {file}: {e}"));
    }
    let site = w.repo("github.com", "studio", "site");
    site.commit(KENJI, w.ago(days(20)), "Scaffold the site", &["index.html"]);
    site.commit(LEA, w.ago(days(12)), "Add the pricing page", &["pricing.html"]);
    site.commit(KENJI, w.ago(days(1)), "Publish the landing page", &["index.html"]);
    site.push("main");
    // Only GitHub has it: the working copy that built it goes.
    std::fs::remove_dir_all(site.path()).unwrap_or_else(|e| panic!("remove site's working copy: {e}"));
}

/// Starting out: an empty workspace, a Godot project made a repository where it is and
/// committed, a repository added from disk, a repository cloned from GitHub (and a second clone refused), and a branch that only
/// existed locally published with Push.
pub fn get_started() -> Journey {
    Journey::new(
        "get_started",
        "Someone opens git-ex-machina for the first time, turns a Godot project into a repository, adds a repository they already have, clones one from GitHub, and publishes a branch that was only on their machine.",
        first_run_world,
    )
    // An empty workspace says how to start, and offers both ways in.
    .see_in(keys::EMPTY_STATE, "Start with a repository")
    .see_in(keys::EMPTY_STATE, "Add one you have, or clone one.")
    .see_in(keys::EMPTY_ADD, "Add repository")
    .see_in(keys::EMPTY_CLONE, "Clone repository")
    // Nothing that can't act yet: no sidebar to filter, no toolbar, no inspector.
    .gone(keys::REPO_FILTER)
    .gone(keys::FETCH)
    .gone(keys::INSPECTOR)
    // Signing in can wait (cloning goes over SSH), so it's offered quietly, apart.
    .see_in(keys::ACCOUNT, "Sign in to GitHub")
    .mark("empty")
    // Add asks for a folder, starting where repositories usually live.
    .tap(keys::EMPTY_ADD)
    .see_in(keys::SHEET, "Add a repository")
    .see_in(keys::ADD_PATH, "~/code/")
    // A path with nothing there is refused, with the reason.
    .type_text("lanter")
    .press(Press::ENTER)
    .wait_for(keys::SHEET_ERROR, "Nothing is at ~/code/lanter")
    // A folder without git is offered as a new repository, right where it is.
    .type_text("n")
    .press(Press::ENTER)
    .wait_for(keys::SHEET, "Make lantern a repository")
    .see_in(keys::INIT_NAME, "lantern")
    .see_in(keys::INIT_README_TEXT, "# lantern")
    .see_in(keys::INIT_IGNORE, "Ignore Godot's generated files")
    // Git LFS is offered for the binary files that are there, and is the person's choice.
    .see_in(keys::INIT_LFS, "Track binary files with Git LFS (.ogg, .png)")
    .tap(keys::INIT_LFS)
    .see_in(keys::INIT_LOCAL, "Local only")
    .see_in(keys::INIT_GITHUB, "On GitHub")
    // The README's heading follows the name until the README is edited.
    .tap(keys::INIT_NAME)
    .press(Press::cmd("a"))
    .type_text("Lantern")
    .see_in(keys::INIT_README_TEXT, "# Lantern")
    .mark("make-it-a-repo")
    .tap(keys::INIT_CONFIRM)
    .wait_for(keys::BRANCH_SUMMARY, "main · no commits yet")
    .gone(keys::SHEET)
    .see_in(keys::repo("lantern"), "main · local")
    // Everything waits uncommitted, to look over first; Godot's cache stays out.
    .see_in(keys::INSPECTOR, "WORKING TREE")
    .see_in(keys::changed_file("README.md"), "A")
    .see_in(keys::changed_file(".gitignore"), "A")
    .see_in(keys::changed_file("project.godot"), "A")
    .see_in(keys::changed_file(".gitattributes"), "A")
    .gone(keys::changed_file(".godot/editor/main.tscn-folding.cfg"))
    .world_check("lantern is a repository with its files where they were, nothing committed", |w| {
        let dir = w.code().join("lantern");
        let readme = std::fs::read_to_string(dir.join("README.md")).map_err(|e| e.to_string())?;
        let ignore = std::fs::read_to_string(dir.join(".gitignore")).map_err(|e| e.to_string())?;
        let commits = gxm_core::history::count(w.env(), &dir).map_err(|e| e.to_string())?;
        match (readme.as_str(), ignore.contains(".godot/"), dir.join("player.gd").exists(), commits) {
            ("# Lantern\n", true, true, 0) => Ok(()),
            _ => Err(format!("README {readme:?}, .gitignore {ignore:?}, {commits} commits")),
        }
    })
    .mark("new-repo")
    // Committing it is the usual stage and commit.
    .tap(keys::STAGE_ALL)
    .wait_for(keys::COMMIT, "Commit")
    .tap(keys::COMMIT)
    .type_text("First playable")
    .tap(keys::COMMIT)
    .wait_for(keys::BRANCH_SUMMARY, "main · 1 commit")
    .see_in(keys::INSPECTOR_TITLE, "First playable")
    .world_check("the assets went in through LFS", |w| {
        let files = w.git(&w.code().join("lantern"), &["lfs", "ls-files", "--name-only"]);
        let mut files: Vec<&str> = files.lines().collect();
        files.sort_unstable();
        if files == ["icon.png", "music/theme.ogg"] { Ok(()) } else { Err(format!("LFS has {files:?}")) }
    })
    .mark("first-commit")
    // A repository that's already there is added as it is.
    .tap(keys::ADD_REPO)
    .see_in(keys::ADD_PATH, "~/code/")
    .type_text("field-notes")
    .tap(keys::ADD_CONFIRM)
    .wait_for(keys::BRANCH_SUMMARY, "drafts")
    .gone(keys::SHEET)
    .see_in(keys::repo("field-notes"), "drafts · personal")
    .see_in(keys::BRANCH_SUMMARY, "not pushed yet")
    .see_in(keys::INSPECTOR_TITLE, "Sketch the onboarding notes")
    .world_check("the workspace keeps both", |w| {
        let ws = gxm_core::workspace::Workspace::load(w.env()).map_err(|e| e.to_string())?;
        let has = |name: &str| ws.repos.iter().any(|r| r.path == w.code().join(name));
        if has("lantern") && has("field-notes") { Ok(()) } else { Err(format!("workspace: {:?}", ws.repos)) }
    })
    .mark("added")
    // Clone takes the URL as GitHub's page shows it, and clones over SSH beside the others.
    .tap(keys::CLONE)
    .see_in(keys::SHEET, "Clone a repository")
    .see_in(keys::CLONE_INTO, "~/code")
    .type_text("https://github.com/studio/site")
    .see_in(keys::CLONE_PLAN, "git@github.com:studio/site.git")
    .see_in(keys::CLONE_PLAN, "~/code/site")
    .mark("clone")
    .tap(keys::CLONE_CONFIRM)
    .wait_for(keys::BRANCH_SUMMARY, "main · 3 commits")
    .gone(keys::SHEET)
    .see_in(keys::repo("site"), "main · studio")
    .see_in(keys::INSPECTOR_TITLE, "Publish the landing page")
    .world_check("site is cloned, with the SSH URL as its origin", |w| {
        let url = w.git(&w.code().join("site"), &["config", "--get", "remote.origin.url"]);
        if url.trim() == "git@github.com:studio/site.git" { Ok(()) } else { Err(format!("origin is {url:?}")) }
    })
    .mark("cloned")
    // Cloning it again would land on the copy that's there, so it says so.
    .tap(keys::CLONE)
    .type_text("git@github.com:studio/site.git")
    .tap(keys::CLONE_CONFIRM)
    .wait_for(keys::SHEET_ERROR, "~/code/site already exists")
    .press(Press::ESCAPE)
    .gone(keys::SHEET)
    // drafts was never pushed: Push publishes it to origin and tracks it there.
    .tap(keys::repo("field-notes"))
    .see_in(keys::BRANCH_SUMMARY, "drafts not pushed yet")
    .tap(keys::PUSH)
    .wait_for(keys::BRANCH_SUMMARY, "drafts · 4 commits")
    .not_see("not pushed yet")
    .world_check("origin has drafts, and drafts tracks it", |w| {
        let local = w.git(&w.code().join("field-notes"), &["rev-parse", "drafts"]).trim().to_owned();
        let remote = w.remote_head("github.com", "personal", "field-notes", "drafts");
        let upstream = w.git(&w.code().join("field-notes"), &["rev-parse", "--abbrev-ref", "drafts@{upstream}"]);
        match (local == remote, upstream.trim()) {
            (true, "origin/drafts") => Ok(()),
            _ => Err(format!("remote drafts {remote}, local {local}, upstream {upstream:?}")),
        }
    })
    .mark("published")
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
        let local = w.git(&w.code().join("atlas-web"), &["rev-parse", "main"]).trim().to_owned();
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
        let subject = w.git(&w.code().join("atlas-web"), &["log", "-1", "--format=%s"]);
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
        let stashes = w.git(&w.code().join("brand-tokens"), &["stash", "list"]);
        if stashes.contains(gxm_core::ops::DISCARD_STASH) { Ok(()) } else { Err(format!("stash list: {stashes:?}")) }
    })
    .mark("discarded")
}

/// The demo workspace, signed out of GitHub. The first sign-in is approved on github.com, the
/// second declined.
fn signed_out_world(w: &World) {
    gxm_world::fixtures::mockup(w);
    w.sign_out();
    w.hub().answer_sign_ins(&[gxm_world::hub::Answer::Approve, gxm_world::hub::Answer::Decline]);
}

/// Signing in to GitHub with a one-time code, seeing who's signed in, signing out, and a
/// sign-in declined on github.com.
pub fn sign_in() -> Journey {
    Journey::new(
        "sign_in",
        "Someone signs in to GitHub with a one-time code, sees who they're signed in as, signs out, and sees a declined sign-in explained.",
        signed_out_world,
    )
    .see_in(keys::ACCOUNT, "Sign in to GitHub")
    // Signed out, a commit in a pull request can't say which one.
    .tap_text("Evict avatars by last paint, not insert")
    .tap(keys::TAB_OVERVIEW)
    .see_in(keys::PULL_NOTICE, "Sign in to GitHub to see pull requests.")
    .mark("signed-out")
    // Sign in asks GitHub for a code and shows it, with where to enter it.
    .tap(keys::ACCOUNT)
    .wait_for(keys::SIGNIN_CODE, gxm_world::hub::USER_CODE)
    .see_in(keys::SHEET, "Sign in to GitHub")
    .see_in(keys::SHEET, "github.com/login/device")
    .see_in(keys::SIGNIN_OPEN, "Approve on GitHub")
    .tap(keys::SIGNIN_OPEN)
    .opened("https://github.com/login/device")
    .mark("code")
    // Ana enters the code on github.com and approves; the app notices on its own.
    .wait_for(keys::ACCOUNT, "ana-ruiz")
    .gone(keys::SHEET)
    // The pull requests come in with the sign-in.
    .wait_for(keys::PULL_REQUEST_ROW, "#131")
    .world_check("the token is in the sandbox's secrets, and the account beside it", |w| {
        let token = gxm_core::auth::token(w.env()).map_err(|e| e.to_string())?;
        let account = gxm_core::auth::signed_in(w.env());
        match (token.as_deref(), account.map(|a| a.login)) {
            (Some(gxm_world::hub::TOKEN), Some(login)) if login == "ana-ruiz" => Ok(()),
            (token, login) => Err(format!("token {token:?}, account {login:?}")),
        }
    })
    .mark("signed-in")
    // The account control says who it is, and signs out.
    .tap(keys::ACCOUNT)
    .see_in(keys::SHEET, "Signed in to GitHub as ana-ruiz")
    .see_in(keys::SHEET, "Ana Ruiz")
    .mark("account")
    .tap(keys::SIGN_OUT)
    .wait_for(keys::ACCOUNT, "Sign in to GitHub")
    .gone(keys::SHEET)
    .wait_for(keys::PULL_NOTICE, "Sign in to GitHub to see pull requests.")
    .world_check("the token is gone", |w| match gxm_core::auth::token(w.env()) {
        Ok(None) => Ok(()),
        other => Err(format!("the token is still there: {other:?}")),
    })
    // A sign-in declined on github.com says so, and offers another go.
    .tap(keys::ACCOUNT)
    .wait_for(keys::SHEET_ERROR, "declined")
    .see_in(keys::SIGNIN_RETRY, "Try again")
    .mark("declined")
    .press(Press::ESCAPE)
    .gone(keys::SHEET)
    .see_in(keys::ACCOUNT, "Sign in to GitHub")
}
