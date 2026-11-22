//! Tagging a release: beacon releases whatever version tag is pushed (its `release.yml` runs on
//! `v*` tags), so a commit on main is tagged as the next version and pushed, which is the
//! release. A commit off main, and not on origin yet, says what tagging it would mean. field-notes
//! doesn't release from tags, so there it's a plain tag that stays on this computer.

use gxm_app::keys;
use gxm_world::World;

use crate::{Journey, Press};

/// beacon's release workflow, as GitHub reads it: a push of a `v*` tag runs it.
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

/// beacon, released twice (v0.1.0 and v0.1.1, both on origin), two commits on main since, and
/// a `feature/watch-mode` commit that was never pushed. field-notes has neither tags nor a
/// release workflow.
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
    beacon.commit(LEA, w.ago(days(14)), "Retry failed uploads", &["src/upload.rs"]);
    beacon.tag("v0.1.1");
    beacon.commit(KENJI, w.ago(days(6)), "Print progress while uploading", &["src/upload.rs"]);
    beacon.commit(ANA, w.ago(days(2)), "Fix the --json flag", &["src/cli.rs"]);
    beacon.push("main");
    beacon.git(&["push", "--quiet", "origin", "--tags"]);
    beacon.branch("feature/watch-mode");
    beacon.commit(LEA, w.ago(hours(5)), "Watch the folder for changes", &["src/watch.rs"]);
    beacon.checkout("main");
    w.add_to_workspace(beacon.path(), true);

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
        "Someone releases the next version of beacon by tagging a commit on main and pushing the tag, sees what tagging a commit off main would mean, and tags a commit in a repository that doesn't release from tags.",
        release_world,
    )
    // beacon releases from version tags, so a commit selected offers to tag a release.
    .tap_text("Fix the --json flag")
    .see_in(keys::CREATE_TAG, "Tag a release")
    .mark("release-ready")
    // The sheet suggests the version after the newest one, and says what pushing it starts.
    .tap(keys::CREATE_TAG)
    .see_in(keys::SHEET, "Tag a release")
    .see_in(keys::TAG_COMMIT, "Fix the --json flag")
    .see_in(keys::TAG_NAME, "v0.1.2")
    .see_in(keys::TAG_PUSH, "Push to origin")
    .see_in(keys::TAG_RELEASE, "Pushing v0.1.2 to origin starts the release workflow (release.yml)")
    .gone(keys::TAG_WARNING)
    .mark("tag-sheet")
    // A version that's already out is refused before anything happens.
    .tap(keys::TAG_NAME)
    .press(Press::cmd("a"))
    .type_text("v0.1.1")
    .tap(keys::TAG_CONFIRM)
    .see_in(keys::SHEET_ERROR, "v0.1.1 already exists, on bef591e")
    .mark("taken")
    // A name the workflow doesn't watch for says it won't release anything.
    .tap(keys::TAG_NAME)
    .press(Press::cmd("a"))
    .type_text("0.1.2")
    .see_in(keys::TAG_RELEASE, "0.1.2 doesn't match v*, so it won't start the release workflow")
    .gone(keys::SHEET_ERROR)
    .mark("not-a-release")
    .tap(keys::TAG_NAME)
    .press(Press::cmd("a"))
    .type_text("v0.1.2")
    .not_see("already exists")
    .tap(keys::TAG_MESSAGE)
    .type_text("Beacon 0.1.2: the --json flag works again")
    .mark("named")
    // Tag a release tags the commit and pushes the tag: that push is the release.
    .tap(keys::TAG_CONFIRM)
    .wait_for(keys::TOASTS, "Pushed v0.1.2 to origin")
    .gone(keys::SHEET)
    .see_in(keys::HISTORY, "v0.1.2")
    .world_check("origin has v0.1.2, annotated with its message, on Fix the --json flag", |w| {
        let remote = w.remote("github.com", "studio", "beacon");
        let kind = w.git(&remote, &["cat-file", "-t", "v0.1.2"]);
        let message = w.git(&remote, &["tag", "-l", "--format=%(contents:subject)", "v0.1.2"]);
        let subject = w.git(&remote, &["log", "-1", "--format=%s", "v0.1.2^{commit}"]);
        match (kind.trim(), message.trim(), subject.trim()) {
            ("tag", "Beacon 0.1.2: the --json flag works again", "Fix the --json flag") => Ok(()),
            other => Err(format!("origin's v0.1.2 (kind, message, commit): {other:?}")),
        }
    })
    .mark("released")
    // A commit off main that was never pushed: the sheet says what tagging it would mean.
    .tap_text("Watch the folder for changes")
    .tap(keys::CREATE_TAG)
    .see_in(keys::TAG_NAME, "v0.1.3")
    .see_in(keys::TAG_WARNING, "This commit isn't on main.")
    .see_in(keys::TAG_WARNING, "It isn't on origin yet: pushing the tag sends it too.")
    .mark("off-main")
    .press(Press::ESCAPE)
    .gone(keys::SHEET)
    // field-notes doesn't release from tags: a plain tag, which stays on this computer.
    .tap(keys::repo("field-notes"))
    .tap_text("Notes from the Lisbon workshop")
    .see_in(keys::CREATE_TAG, "Create tag")
    .tap(keys::CREATE_TAG)
    .see_in(keys::SHEET, "Create a tag")
    .see_in(keys::TAG_PUSH, "Push to origin")
    .gone(keys::TAG_RELEASE)
    .type_text("lisbon-2026")
    .mark("plain-tag")
    .tap(keys::TAG_CONFIRM)
    .wait_for(keys::TOASTS, "Tagged lisbon-2026")
    .gone(keys::SHEET)
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
