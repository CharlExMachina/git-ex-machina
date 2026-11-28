//! Tagging a release: beacon releases whatever version tag is pushed (its `release.yml` runs on
//! `v*` tags, and takes the release's notes from the tag), so a commit on main is tagged as the
//! next version, with notes drafted by GitHub and written as rich text, and pushed, which is the
//! release, a screenshot in the notes going up with the tag. A commit off main, and not
//! on origin yet, says what tagging it would mean. field-notes doesn't release from tags, so
//! there it's a plain tag that stays on this computer.

use gxm_app::keys;
use gxm_world::World;
use gxm_world::hub::{self, PullData};

use crate::{Journey, Press};

/// beacon's release workflow, as GitHub reads it: a push of a `v*` tag runs it, and the
/// release's notes are the tag's message.
const RELEASE_YML: &str = "\
name: release

# A version tag releases that version.
on:
  push:
    tags: [\"v*\"]

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: gh release create \"$GITHUB_REF_NAME\" --notes-from-tag --verify-tag
";

/// What v0.1.2's tag says, as GitHub will show it: the summary and highlight written in the
/// sheet, then GitHub's draft folded away. `{files}` is the commit the screenshot went up in.
const NOTES: &str = "\
**v0.1.2** · 2 changes from 2 contributors

- **Fix the --json flag.** It prints JSON again, with every field the old version printed, in the same order, so the scripts that read it keep working.

### Plus a steadier progress bar

Thanks to @kenji-mori for #5 and to everyone who tried the nightlies.

- [x] Linux AppImage
- [ ] macOS disk image

![progress.png](https://github.com/studio/beacon/raw/{files}/progress.png)

<details>
<summary>Full changelog</summary>

## What's Changed

- Print progress while uploading by @kenji-mori in https://github.com/studio/beacon/pull/5
- Fix the --json flag by @lea-brandt in https://github.com/studio/beacon/pull/6

## New Contributors

- @lea-brandt made their first contribution in https://github.com/studio/beacon/pull/6

**Full Changelog**: https://github.com/studio/beacon/compare/v0.1.1...v0.1.2

</details>
";

/// beacon's checks, which run on branches, not tags: it mustn't be taken for the release.
const CI_YML: &str = "\
name: ci
on:
  push:
    branches: [main]
  pull_request:
jobs:
  test:
    runs-on: ubuntu-latest
";

/// The screenshot attached to v0.1.2: a PNG's first bytes, enough to be one.
const SCREENSHOT: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR progress";

/// beacon, released twice (v0.1.0 and v0.1.1, both on origin), two pull requests merged into main
/// since (Lea's first), and a `feature/watch-mode` commit that was never pushed; its pull requests
/// on GitHub, and a screenshot in `~/code/shots`. field-notes has neither tags nor a release
/// workflow.
fn release_world(w: &World) {
    use gxm_world::fixtures::{ANA, KENJI, LEA};
    use gxm_world::{days, hours};
    let beacon = w.repo("github.com", "studio", "beacon");
    beacon.commit(ANA, w.ago(days(30)), "Start beacon", &["src/main.rs"]);
    beacon.commit(KENJI, w.ago(days(25)), "Read the config file", &["src/config.rs"]);
    beacon.tag("v0.1.0");
    beacon.write(".github/workflows/ci.yml", CI_YML);
    beacon.write(".github/workflows/release.yml", RELEASE_YML);
    beacon.commit(ANA, w.ago(days(20)), "Release from version tags", &[]);
    beacon.commit(KENJI, w.ago(days(14)), "Retry failed uploads (#3)", &["src/upload.rs"]);
    beacon.tag("v0.1.1");
    beacon.commit(KENJI, w.ago(days(6)), "Print progress while uploading (#5)", &["src/upload.rs"]);
    beacon.commit(LEA, w.ago(days(2)), "Fix the --json flag (#6)", &["src/cli.rs"]);
    beacon.push("main");
    beacon.git(&["push", "--quiet", "origin", "--tags"]);
    beacon.branch("feature/watch-mode");
    beacon.commit(LEA, w.ago(hours(5)), "Watch the folder for changes", &["src/watch.rs"]);
    beacon.checkout("main");
    w.add_to_workspace(beacon.path(), true);
    for (number, author, subject) in [
        (3, hub::KENJI, "Retry failed uploads"),
        (5, hub::KENJI, "Print progress while uploading"),
        (6, hub::LEA, "Fix the --json flag"),
    ] {
        let sha = beacon.git(&["log", "-1", "--format=%H", &format!("--grep=(#{number})")]).trim().to_owned();
        w.hub().add_pull(
            "studio",
            "beacon",
            PullData {
                number,
                title: subject.to_owned(),
                body: String::new(),
                merged: true,
                draft: false,
                author,
                head_ref: format!("pr-{number}"),
                head_sha: sha.clone(),
                base_ref: "main".to_owned(),
                merge_commit: Some(sha),
                created: w.ago(days(15)),
                commits: 1,
                requested: Vec::new(),
                reviews: Vec::new(),
                comments: Vec::new(),
                threads: Vec::new(),
                checks: Vec::new(),
            },
        );
    }
    std::fs::create_dir_all(w.code().join("shots")).unwrap_or_else(|e| panic!("make ~/code/shots: {e}"));
    std::fs::write(w.code().join("shots/progress.png"), SCREENSHOT)
        .unwrap_or_else(|e| panic!("write the screenshot: {e}"));

    let notes = w.repo("github.com", "personal", "field-notes");
    notes.commit(ANA, w.ago(days(9)), "Start the field notes", &["README.md"]);
    notes.commit(KENJI, w.ago(days(6)), "Notes from the Lisbon workshop", &["2026/lisbon.md"]);
    notes.commit(ANA, w.ago(days(1)), "Draft the review checklist", &["drafts/checklist.md"]);
    notes.push("main");
    w.add_to_workspace(notes.path(), false);
}

pub fn tag_a_release() -> Journey {
    Journey::new(
        "tag_a_release",
        "Someone releases the next version of beacon by tagging a commit on main, writing its notes from GitHub's draft without markdown (a mention, a reference, tasks and a screenshot included), and pushing the tag, sees what tagging a commit off main would mean, and tags a commit in a repository that doesn't release from tags.",
        release_world,
    )
    // beacon releases from version tags, so a commit selected offers to tag a release.
    .tap_text("Fix the --json flag (#6)")
    .see_in(keys::CREATE_TAG, "Tag a release")
    .mark("release-ready")
    // A window of its own opens on the commit: it suggests the version after the newest one,
    // and says what pushing it starts.
    .tap(keys::CREATE_TAG)
    .see_in(keys::TAG_TITLE, "Tag a release")
    .see_in(keys::TAG_COMMIT, "Fix the --json flag (#6)")
    .see_in(keys::TAG_NAME, "v0.1.2")
    .see_in(keys::TAG_PUSH, "Push to origin")
    .see_in(keys::TAG_RELEASE, "Pushing v0.1.2 to origin starts the release workflow (release.yml), with these notes.")
    .gone(keys::TAG_WARNING)
    // The workflow takes the release's notes from the tag, so the window writes them, in the
    // height it has left.
    .see_in(keys::NOTES, "Release notes")
    .gone(keys::TAG_MESSAGE)
    .mark("tag-window")
    // A version that's already out is refused before anything happens.
    .tap(keys::TAG_NAME)
    .press(Press::cmd("a"))
    .type_text("v0.1.1")
    .tap(keys::TAG_CONFIRM)
    .see_in(keys::TAG_ERROR, "v0.1.1 already exists, on 450f3e1")
    .mark("taken")
    // A name the workflow doesn't watch for says it won't release anything.
    .tap(keys::TAG_NAME)
    .press(Press::cmd("a"))
    .type_text("0.1.2")
    .see_in(keys::TAG_RELEASE, "0.1.2 doesn't match v*, so it won't start the release workflow")
    .gone(keys::TAG_ERROR)
    .mark("not-a-release")
    .tap(keys::TAG_NAME)
    .press(Press::cmd("a"))
    .type_text("v0.1.2")
    .not_see("already exists")
    // GitHub drafts what changed since v0.1.1, folded away under a summary still to write.
    .tap(keys::NOTES_GENERATE)
    .wait_for(keys::NOTES, "What's Changed")
    .see_in(keys::NOTES, "v0.1.2 · 2 changes from 2 contributors")
    .see_in(keys::NOTES, "Full changelog")
    .see_in(keys::NOTES, "Fix the --json flag by @lea-brandt in https://github.com/studio/beacon/pull/6")
    .see_in(keys::NOTES, "New Contributors")
    .mark("draft")
    // The highlight, written without markdown: ⌘B for its bold lead-in. It's long enough to wrap.
    .press(Press::cmd("b"))
    .type_text("Fix the --json flag.")
    .press(Press::cmd("b"))
    .type_text(" It prints JSON again, with every field the old version printed, in the same order, so the scripts that read it keep working")
    // Return twice ends the list; the heading button makes the line a heading.
    .press(Press::ENTER)
    .press(Press::ENTER)
    .type_text("Plus a steadier progress bar")
    .tap(keys::NOTES_HEADING)
    // ↑ and ↓ go row by row through the wrapped highlight, and only past its last row to the
    // heading: the full stop lands at the highlight's end.
    .press(Press::UP)
    .press(Press::UP)
    .press(Press::DOWN)
    .press(Press::cmd("Right"))
    .type_text(".")
    .press(Press::DOWN)
    .press(Press::cmd("Right"))
    .mark("notes")
    // Under the heading: @ and # offer people and pull requests to put in.
    .press(Press::ENTER)
    .type_text("Thanks to @ke")
    .see_in(keys::NOTES_PICKER, "@kenji-mori")
    .see_in(keys::NOTES_PICKER, "Kenji Mori")
    .mark("mention")
    .press(Press::ENTER)
    .gone(keys::NOTES_PICKER)
    .type_text("for #")
    .see_in(keys::NOTES_PICKER, "#5")
    .see_in(keys::NOTES_PICKER, "#6")
    .type_text("5")
    .see_in(keys::NOTES_PICKER, "Print progress while uploading")
    .gone(keys::notes_pick(1))
    .press(Press::ENTER)
    .type_text("and to everyone who tried the nightlies.")
    // A task list: the button makes the line a task; a click on its checkbox checks it off.
    .press(Press::ENTER)
    .tap(keys::NOTES_TASKS)
    .type_text("Linux AppImage")
    .tap(keys::notes_task(0))
    .press(Press::ENTER)
    .type_text("macOS disk image")
    .mark("tasks")
    // Return in an empty task ends the list; a screenshot dropped on the notes goes in there.
    .press(Press::ENTER)
    .press(Press::ENTER)
    .drop_file(keys::NOTES, "shots/progress.png")
    .see_in(keys::NOTES, "progress.png")
    .mark("attached")
    // Markdown shows what the tag will carry: what GitHub will show.
    .tap(keys::NOTES_SOURCE)
    .see_in(keys::NOTES_MARKDOWN, "- **Fix the --json flag.** It prints JSON again, with every field")
    .see_in(keys::NOTES_MARKDOWN, "### Plus a steadier progress bar")
    .see_in(keys::NOTES_MARKDOWN, "<summary>Full changelog</summary>")
    .see_in(keys::NOTES_MARKDOWN, "Thanks to @kenji-mori for #5")
    .see_in(keys::NOTES_MARKDOWN, "- [x] Linux AppImage")
    .see_in(keys::NOTES_MARKDOWN, "![progress.png](https://github.com/studio/beacon/raw/")
    .mark("markdown")
    .tap(keys::NOTES_WRITE)
    .see_in(keys::NOTES, "Plus a steadier progress bar")
    // Tag a release tags the commit and pushes the tag: that push is the release.
    .tap(keys::TAG_CONFIRM)
    // The screenshot goes up in the same push as the tag: nothing waits for the workflow. The
    // window closes, and the main window says what went up.
    .wait_for(keys::TOASTS, "Pushed v0.1.2 and progress.png to origin")
    .gone(keys::TAG_WINDOW)
    .see_in(keys::HISTORY, "v0.1.2")
    .world_check("origin has the screenshot, as it was, under refs/release-files/v0.1.2", |w| {
        let remote = w.remote("github.com", "studio", "beacon");
        let files = w.git(&remote, &["ls-tree", "--name-only", "refs/release-files/v0.1.2"]);
        let blob = w.git(&remote, &["rev-parse", "refs/release-files/v0.1.2:progress.png"]);
        let bytes = std::process::Command::new("git")
            .args(["-C", &remote.to_string_lossy(), "cat-file", "blob", blob.trim()])
            .output()
            .map_err(|e| e.to_string())?
            .stdout;
        if files.trim() == "progress.png" && bytes == SCREENSHOT { Ok(()) } else { Err(format!("files {files:?}, {} bytes", bytes.len())) }
    })
    .world_check("origin has v0.1.2 on Fix the --json flag, its message the notes as markdown", |w| {
        let remote = w.remote("github.com", "studio", "beacon");
        let kind = w.git(&remote, &["cat-file", "-t", "v0.1.2"]);
        let subject = w.git(&remote, &["log", "-1", "--format=%s", "v0.1.2^{commit}"]);
        let message = w.git(&remote, &["tag", "-l", "--format=%(contents)", "v0.1.2"]);
        let files = w.git(&remote, &["rev-parse", "refs/release-files/v0.1.2"]);
        let notes = NOTES.replace("{files}", files.trim());
        match (kind.trim(), subject.trim()) {
            ("tag", "Fix the --json flag (#6)") if message == notes => Ok(()),
            other => Err(format!("origin's v0.1.2 (kind, commit) {other:?}, message:\n{message}")),
        }
    })
    .mark("released")
    // A commit off main that was never pushed: the window says what tagging it would mean.
    .tap_text("Watch the folder for changes")
    .tap(keys::CREATE_TAG)
    .see_in(keys::TAG_NAME, "v0.1.3")
    .see_in(keys::TAG_WARNING, "This commit isn't on main.")
    .see_in(keys::TAG_WARNING, "It isn't on origin yet: pushing the tag sends it too.")
    .mark("off-main")
    .tap(keys::TAG_CANCEL)
    .gone(keys::TAG_WINDOW)
    // field-notes doesn't release from tags: a plain tag, which stays on this computer.
    .tap(keys::repo("field-notes"))
    .tap_text("Notes from the Lisbon workshop")
    .see_in(keys::CREATE_TAG, "Create tag")
    .tap(keys::CREATE_TAG)
    .see_in(keys::TAG_TITLE, "Create a tag")
    .see_in(keys::TAG_PUSH, "Push to origin")
    .gone(keys::TAG_RELEASE)
    .type_text("lisbon-2026")
    .mark("plain-tag")
    .tap(keys::TAG_CONFIRM)
    .wait_for(keys::TOASTS, "Tagged lisbon-2026")
    .gone(keys::TAG_WINDOW)
    .see_in(keys::HISTORY, "lisbon-2026")
    .world_check("lisbon-2026 is here, on its commit, and not on origin", |w| {
        let here = w.git(&w.code().join("field-notes"), &["log", "-1", "--format=%s", "lisbon-2026"]);
        let remote = w.remote("github.com", "personal", "field-notes");
        let there = w.git(&remote, &["tag", "--list"]);
        match (here.trim(), there.trim()) {
            ("Notes from the Lisbon workshop", "") => Ok(()),
            other => Err(format!("(here, on origin): {other:?}")),
        }
    })
    .mark("tagged")
}
