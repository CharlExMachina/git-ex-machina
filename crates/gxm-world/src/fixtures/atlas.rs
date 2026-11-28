//! `atlas-web`, the repository the demo workspace opens on.
//!
//! Each listed commit has its author, time and message body, on a small but complete topology:
//! `feature/graph-virtualization` merged into `main`, `fix/avatar-cache` forked from the
//! `v0.4.0` commit and still open, 1,284 commits in all (the mockup's worktree branches add 13). Files change by exact line counts, so
//! the merge reads `4 files · +232 −93` and its `rows.rs` hunk is short and readable.
//! `origin/main` is on `Spring the sidebar collapse`, leaving `main` four ahead: the merge and
//! the three feature commits it brought.

use std::path::PathBuf;

use jiff::{SignedDuration, Timestamp};

use super::{ANA, KENJI, LEA, t};
use crate::{Author, RepoBuilder, World};

/// Commits before the listed ones, so the summary reads `1,284 commits`.
const OLDER: usize = 1260;

pub fn atlas_web(w: &World) -> PathBuf {
    let r = w.repo("github.com", "studio", "atlas-web");
    r.fast_import(&older_history(), "main");

    let linear: &[(Author, &str, &str, &str)] = &[
        (
            LEA,
            "2026-10-25T11:40:00Z",
            "Read config from the global gitconfig",
            "Author name and email come from your git config.",
        ),
        (
            KENJI,
            "2026-10-26T18:21:00Z",
            "Relative dates in rows, absolute on hover",
            "Rows show 2h or Mon; the inspector shows the full date.",
        ),
        (
            ANA,
            "2026-10-27T09:58:00Z",
            "Open repositories from the command line",
            "Running the app with a path opens that repository.",
        ),
        (
            LEA,
            "2026-10-28T13:30:00Z",
            "Recessed search field in the sidebar",
            "The filter field sits in a well, as the system specifies.",
        ),
        (
            KENJI,
            "2026-10-29T10:15:00Z",
            "Cache lane layout between fetches",
            "Unchanged history keeps its lane layout after a fetch.",
        ),
        (
            ANA,
            "2026-10-29T16:44:00Z",
            "Ahead and behind counts per repository",
            "Each repository row shows commits to push and pull.",
        ),
        (
            LEA,
            "2026-10-30T09:05:00Z",
            "Load history in pages of 500 behind the scenes",
            "Older commits stream in as you scroll.",
        ),
        (
            KENJI,
            "2026-10-31T10:20:00Z",
            "Diff view wraps at 100 columns",
            "Long lines wrap in the full-width diff view.",
        ),
        (
            ANA,
            "2026-11-01T17:36:00Z",
            "Group repositories by owner",
            "Owner names now sit beside the branch in each row.",
        ),
        (
            LEA,
            "2026-11-02T11:02:00Z",
            "Watch .git for external changes",
            "Commits made in a terminal show up without a refresh.",
        ),
        (KENJI, "2026-11-02T15:10:00Z", "Persist pinned repositories", "Pinned repositories survive a relaunch."),
        (
            ANA,
            "2026-11-03T09:44:00Z",
            "Window chrome and traffic-light inset",
            "Toolbar sits level with the traffic lights.",
        ),
        (
            LEA,
            "2026-11-04T17:15:00Z",
            "Read refs with gix instead of shelling out",
            "Ref listing no longer spawns git.",
        ),
        (ANA, "2026-11-05T12:30:00Z", "Split view remembers column widths", "Column widths persist per window."),
        (
            KENJI,
            "2026-11-06T14:48:00Z",
            "Tune brass for the light theme",
            "Darkens accent and lanes so text and marks pass contrast on paper.",
        ),
        (ANA, "2026-11-07T10:01:00Z", "Bump rune to 0.9", "Picks up the new spring API."),
    ];
    for (i, &(who, at, subject, body)) in linear.iter().enumerate() {
        r.commit(who, t(at), &format!("{subject}\n\n{body}"), &[&format!("notes/{:02}.md", i % 20)]);
    }

    r.branch("feature/graph-virtualization");
    r.write("src/graph/virtual_list.rs", &numbered("    // virtual list", 1, 60));
    commit(&r, KENJI, "2026-11-08T16:20:00Z", "Start graph virtualization", "Scaffolding for the virtual list.");

    r.checkout("main");
    r.replace_lines("src/avatars/cache.rs", 50, 1, &lines("    cache.evict_on_release();", 2));
    commit(&r, LEA, "2026-11-09T08:12:00Z", "Fix avatar cache eviction", "Released as v0.4.0.");
    r.tag("v0.4.0");

    r.branch("fix/avatar-cache");
    r.replace_lines("src/avatars/cache.rs", 56, 1, &lines("    counters.record(hit, miss, evicted);", 3));
    commit(
        &r,
        LEA,
        "2026-11-09T09:30:00Z",
        "Measure avatar cache pressure",
        "Adds counters for hits, misses and evictions.",
    );

    r.checkout("feature/graph-virtualization");
    r.replace_lines(
        "src/graph/rows.rs",
        42,
        1,
        &["    let x = PAD + lane.col as f32 * PITCH;".to_owned(), "    draw_rail(x, y, H, lane.color);".to_owned()],
    );
    commit(
        &r,
        KENJI,
        "2026-11-09T11:05:00Z",
        "Draw lanes with cubic joins",
        "Branches peel away from a node with a cubic curve rather than a kink.",
    );

    r.checkout("main");
    r.replace_lines("src/sidebar.rs", 40, 12, &lines("    width.animate_to(target).motion(motion::SMOOTH);", 38));
    r.replace_lines("src/theme/motion.rs", 10, 2, &lines("pub const SMOOTH: Spring = Spring::new(0.38, 1.0);", 9));
    r.git(&["mv", "src/brass.json", "src/theme/brass.json"]);
    r.replace_lines("src/theme/brass.json", 5, 3, &lines("  \"spring\": \"smooth\",", 3));
    // Ana wrote it with Lea and Kenji committed it, so the inspector has a committer and
    // trailers to show.
    r.commit_by(
        ANA,
        KENJI,
        t("2026-11-10T18:42:00Z"),
        "Spring the sidebar collapse\n\nThe sidebar now collapses on the smooth spring instead of snapping.\n\n\
         Co-authored-by: Lea Brandt <lea.brandt@example.com>\nRefs: #121",
    );
    r.push("main");

    r.checkout("fix/avatar-cache");
    r.replace_lines("src/avatars/cache.rs", 5, 31, &lines("    let oldest = self.by_last_paint.pop_min();", 27));
    r.write("src/avatars/metrics.rs", &numbered("// avatar cache metrics", 1, 54));
    commit(
        &r,
        LEA,
        "2026-11-11T10:00:00Z",
        "Evict avatars by last paint, not insert",
        "Avatars that are on screen stay cached even when they were loaded first.",
    );

    r.checkout("feature/graph-virtualization");
    r.replace_lines("src/graph/rows.rs", 61, 20, &lines("    rows.mount_visible(viewport);", 82));
    r.append("src/graph/virtual_list.rs", &numbered("    // window", 61, 82));
    r.replace_lines("src/sidebar.rs", 5, 9, &lines("    header.item(row);", 6));
    r.git(&["rm", "--quiet", "src/graph/legacy_layout.rs"]);
    commit(
        &r,
        KENJI,
        "2026-11-11T12:00:00Z",
        "Virtualize commit rows past 10k",
        "Rows outside the viewport are no longer laid out. Lane state is carried per row, so scrolling stays smooth on large histories.",
    );

    r.checkout("main");
    r.merge(
        ANA,
        t("2026-11-11T13:00:00Z"),
        "feature/graph-virtualization",
        "Merge feature/graph-virtualization\n\nBrings row virtualization into main. No conflicts.",
    );

    r.write("src/toolbar/context.rs", &numbered("// toolbar context", 1, 58));
    r.replace_lines("src/graph/selection.rs", 10, 4, &lines("    selection.extend_to(row);", 12));
    r.replace_lines("src/theme/brass.json", 12, 2, &lines("  \"accent\": \"#e2ad62\",", 2));

    r.fetched(w.ago(SignedDuration::from_mins(2)));
    r.path().to_path_buf()
}

fn commit(r: &RepoBuilder<'_>, who: Author, at: &str, subject: &str, body: &str) {
    r.commit(who, t(at), &format!("{subject}\n\n{body}"), &[]);
}

/// `n` distinct lines: `text // 1`, `text // 2`, …
fn lines(text: &str, n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("{text} // {i}")).collect()
}

/// Lines `from..from + n` of a numbered file, newline-terminated.
fn numbered(prefix: &str, from: usize, n: usize) -> String {
    (from..from + n).map(|i| format!("{prefix} {i}\n")).collect()
}

/// The files the listed commits edit, as they stood before them.
fn base_files() -> Vec<(&'static str, String)> {
    let mut rows = numbered("// rows", 1, 39);
    rows.push_str(
        "fn layout_row(row: &Row)\nfor lane in lanes.active() {\n    draw_line(lane.x, y, lane.x, y + H);\n}\n",
    );
    rows.push_str(&numbered("    // row step", 44, 16));
    rows.push_str(&numbered("    legacy_row_step", 60, 20));
    rows.push_str("// end of rows\n");
    vec![
        ("README.md", "# atlas-web\n\nA git client's history view.\n".to_owned()),
        ("Cargo.toml", "[package]\nname = \"atlas-web\"\nversion = \"0.3.0\"\n".to_owned()),
        ("src/graph/rows.rs", rows),
        ("src/graph/legacy_layout.rs", numbered("// legacy lane layout", 1, 63)),
        ("src/graph/selection.rs", numbered("    // selection", 1, 30)),
        ("src/sidebar.rs", numbered("    // sidebar", 1, 70)),
        ("src/theme/motion.rs", numbered("// motion", 1, 20)),
        ("src/brass.json", numbered("  // brass token", 1, 20)),
        ("src/avatars/cache.rs", numbered("    // avatar cache", 1, 60)),
    ]
}

/// The history before the first listed commit, as a `git fast-import` stream: the
/// base files first, then commits six hours apart, each rewriting one of twenty notes.
fn older_history() -> String {
    const VERBS: [&str; 10] =
        ["Tidy", "Rename", "Document", "Test", "Speed up", "Simplify", "Fix", "Polish", "Split", "Cache"];
    const THINGS: [&str; 12] = [
        "the repository list",
        "ref parsing",
        "the diff view",
        "status reads",
        "lane colours",
        "the toolbar",
        "keyboard focus",
        "date formatting",
        "config loading",
        "the commit graph",
        "avatar loading",
        "workspace saves",
    ];
    let people = [ANA, KENJI, LEA];
    let last: Timestamp = t("2026-10-24T18:00:00Z");
    let mut s = String::new();
    for i in 0..OLDER {
        let at = last - SignedDuration::from_hours(6 * (OLDER - 1 - i) as i64);
        let who = people[i % 3];
        let subject = if i == 0 {
            "Initial commit".to_owned()
        } else {
            format!("{} {}", VERBS[i % VERBS.len()], THINGS[(i / VERBS.len()) % THINGS.len()])
        };
        let sig = format!("{} <{}> {} +0000", who.name, who.email, at.as_second());
        s.push_str(&format!("commit refs/heads/main\nmark :{}\nauthor {sig}\ncommitter {sig}\n", i + 1));
        s.push_str(&format!("data {}\n{subject}\n", subject.len() + 1));
        if i > 0 {
            s.push_str(&format!("from :{i}\n"));
        }
        let files = if i == 0 { base_files() } else { vec![(NOTES[i % NOTES.len()], format!("{subject} ({i})\n"))] };
        for (path, body) in files {
            s.push_str(&format!("M 100644 inline {path}\ndata {}\n{body}\n", body.len()));
        }
    }
    s
}

const NOTES: [&str; 20] = [
    "notes/00.md",
    "notes/01.md",
    "notes/02.md",
    "notes/03.md",
    "notes/04.md",
    "notes/05.md",
    "notes/06.md",
    "notes/07.md",
    "notes/08.md",
    "notes/09.md",
    "notes/10.md",
    "notes/11.md",
    "notes/12.md",
    "notes/13.md",
    "notes/14.md",
    "notes/15.md",
    "notes/16.md",
    "notes/17.md",
    "notes/18.md",
    "notes/19.md",
];
