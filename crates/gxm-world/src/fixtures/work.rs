//! atlas-web's open work, as Ana (the viewer) finds it: worktrees in `~/worktrees/atlas-web`,
//! her open pull requests, the ones she's asked to review, and a worktree with no pull request
//! yet. All of it is real git: the branches are pushed to the world's remote, the worktrees are
//! `git worktree add`ed, and #118 really conflicts with main (in `src/avatars/cache.rs`, since
//! #129's merge changed the same lines).
//!
//! The branches' commits are dated in mid-October, one branch per day, each forking from that
//! day's main and never overlapping another: the history's first screens and its lane count
//! stay as they were. What dates the work is the pull requests' own dates, on the world's GitHub.

use std::path::{Path, PathBuf};

use jiff::{SignedDuration, Timestamp};

use super::{ANA, KENJI, LEA, t};
use crate::hub::{self, CheckData, CommentData, Person, PullData, Request, ReviewData, ThreadData, Verdict};
use crate::{Author, RepoBuilder, World};

/// Where atlas-web's worktrees live: `~/worktrees/atlas-web/<name>`.
pub fn worktrees_dir(w: &World) -> PathBuf {
    w.env().home.join("worktrees").join("atlas-web")
}

/// One branch of work: forked from main on `day` (October), committed in a worktree.
struct Branch {
    name: &'static str,
    worktree: &'static str,
    day: u32,
    author: Author,
    commits: &'static [(&'static str, &'static str)],
}

pub fn atlas_work(w: &World, atlas: &Path) {
    let r = w.open(atlas);
    let sha = |rev: &str| r.git(&["rev-parse", rev]).trim().to_owned();

    // #118: loading avatars in parallel, which rewrote the lines #129 changed since.
    let loading = branch(
        w,
        &r,
        &Branch {
            name: "perf/avatar-loading",
            worktree: "avatar-loading",
            day: 17,
            author: ANA,
            commits: &[
                ("src/avatars/loader.rs", "Load avatars on a worker pool"),
                ("src/avatars/loader.rs", "Cap the pool at four workers"),
            ],
        },
    );
    loading.replace_lines(
        "src/avatars/cache.rs",
        49,
        3,
        &["    let loaded = pool.load_all(missing); // 49".to_owned()],
    );
    loading.commit(ANA, oct(17, 11), "Load missing avatars together", &[]);
    loading.push("perf/avatar-loading");

    // #122 and #135: Lea's and Kenji's, on GitHub only, waiting on Ana's review.
    remote_only(
        w,
        &r,
        &Branch {
            name: "feat/split-memory",
            worktree: "split-memory",
            day: 18,
            author: LEA,
            commits: &[
                ("src/split.rs", "Remember split widths per window"),
                ("src/split.rs", "Restore widths on launch"),
            ],
        },
    );
    // #127: changes asked, with work under way in its worktree.
    let fade = branch(
        w,
        &r,
        &Branch {
            name: "graph/fade-merged",
            worktree: "fade-merged",
            day: 19,
            author: ANA,
            commits: &[("src/graph/fade.rs", "Fade the lanes of merged branches")],
        },
    );
    fade.push("graph/fade-merged");
    for (i, file) in ["src/graph/fade.rs", "src/graph/rows.rs", "src/graph/selection.rs"].iter().enumerate() {
        fade.append(file, "work in progress\n");
        touched(&fade, file, w.ago(SignedDuration::from_hours(30 + i as i64)));
    }
    // #132: approved and green.
    branch(
        w,
        &r,
        &Branch {
            name: "ui/sync-group",
            worktree: "sync-group",
            day: 20,
            author: ANA,
            commits: &[("src/toolbar/sync.rs", "Group the toolbar's sync buttons")],
        },
    )
    .push("ui/sync-group");
    remote_only(
        w,
        &r,
        &Branch {
            name: "graph/keep-lane-colours",
            worktree: "keep-lane-colours",
            day: 21,
            author: KENJI,
            commits: &[("src/graph/palette.rs", "Keep lane colours across fetches")],
        },
    );
    // #133: a draft, with two commits not pushed yet.
    let filter = branch(
        w,
        &r,
        &Branch {
            name: "ui/filter-count",
            worktree: "filter-count",
            day: 22,
            author: ANA,
            commits: &[("src/filter.rs", "Show how many repositories the filter matches")],
        },
    );
    filter.push("ui/filter-count");
    filter.commit(ANA, oct(22, 12), "Say when nothing matches", &["src/filter.rs"]);
    filter.commit(ANA, oct(22, 13), "Clear the filter with Escape", &["src/filter.rs"]);
    // No pull request yet: two commits, never pushed, and seven files being edited right now.
    let badges = branch(
        w,
        &r,
        &Branch {
            name: "ui/sidebar-badges",
            worktree: "sidebar-badges",
            day: 23,
            author: ANA,
            commits: &[
                ("src/sidebar_badges.rs", "Count open work in the sidebar"),
                ("src/sidebar_badges.rs", "Colour the count by the oldest pull request"),
            ],
        },
    );
    for i in 0..7 {
        let file = format!("src/badges/{i}.rs");
        badges.write(&file, "// badge\n");
        touched(&badges, &file, w.ago(SignedDuration::from_mins(4 + i)));
    }

    let check = |name: &str, passed, seconds| CheckData {
        name: name.to_owned(),
        passed,
        title: if passed { String::new() } else { "1 test failed".to_owned() },
        seconds,
        queued: false,
    };
    let green = || vec![check("build", true, 123), check("test", true, 221), check("lint", true, 38)];
    let say =
        |author: Person, at: Timestamp, body: &str| CommentData { author, body: body.to_owned(), at, review: None };
    let thread = |id: &str, line, resolved, comments| ThreadData {
        id: id.to_owned(),
        path: "src/avatars/loader.rs".to_owned(),
        line,
        side: "RIGHT",
        resolved,
        comments,
    };
    let reviewed = |id: &str, author, verdict, at| ReviewData {
        id: id.to_owned(),
        author,
        verdict: Some(verdict),
        body: String::new(),
        at,
        sent: Vec::new(),
    };
    let days = |n: i64| w.ago(SignedDuration::from_hours(24 * n));
    let pull = |number, title: &str, author, head: &str, created| PullData {
        number,
        title: title.to_owned(),
        body: String::new(),
        merged: false,
        draft: false,
        author,
        head_ref: head.to_owned(),
        head_sha: sha(&format!("origin/{head}")),
        base_ref: "main".to_owned(),
        merge_commit: None,
        created,
        commits: r.git(&["rev-list", "--count", &format!("origin/main..origin/{head}")]).trim().parse().unwrap_or(1),
        requested: Vec::new(),
        reviews: Vec::new(),
        comments: Vec::new(),
        threads: Vec::new(),
        checks: green(),
    };
    let gh = w.hub();
    let add = |p: PullData| gh.add_pull("studio", "atlas-web", p);

    // The merge that #118 now conflicts with: Lea's fix, squashed into main as v0.4.0's commit.
    let fix = sha("v0.4.0");
    add(PullData {
        number: 129,
        title: "Fix avatar cache eviction".to_owned(),
        body: "Released as v0.4.0.".to_owned(),
        merged: true,
        draft: false,
        author: hub::LEA,
        head_ref: "fix/eviction".to_owned(),
        head_sha: fix.clone(),
        base_ref: "main".to_owned(),
        merge_commit: Some(fix),
        created: t("2026-11-09T07:40:00Z"),
        commits: 1,
        requested: Vec::new(),
        reviews: Vec::new(),
        comments: Vec::new(),
        threads: Vec::new(),
        checks: green(),
    });

    let kenji_at = days(19);
    add(PullData {
        body: "Avatars load on a small worker pool instead of one at a time.".to_owned(),
        reviews: vec![reviewed("PRR_118_kenji", hub::KENJI, Verdict::Comment, kenji_at)],
        threads: vec![
            thread(
                "PRRT_118_1",
                12,
                false,
                vec![say(hub::KENJI, kenji_at, "Should this fall back to one at a time when the pool is busy?")],
            ),
            thread(
                "PRRT_118_2",
                30,
                false,
                vec![say(hub::KENJI, kenji_at, "The pool never shrinks. Drop idle workers after a minute.")],
            ),
            thread(
                "PRRT_118_3",
                41,
                false,
                vec![
                    say(hub::KENJI, kenji_at, "Can we drop the legacy fetch here?"),
                    say(hub::ANA, days(18), "Not yet: the settings page still uses it. Next PR."),
                ],
            ),
            thread("PRRT_118_4", 7, true, vec![say(hub::KENJI, kenji_at, "Typo in the doc comment.")]),
        ],
        checks: vec![
            check("e2e / safari", false, 252),
            check("e2e / firefox", false, 238),
            check("build", true, 123),
            check("test", true, 221),
            check("lint", true, 38),
        ],
        ..pull(118, "Load avatars in parallel", hub::ANA, "perf/avatar-loading", days(23))
    });
    add(PullData {
        requested: vec![Request { person: hub::ANA, at: days(14) }],
        checks: vec![
            check("build", true, 123),
            check("test", true, 221),
            check("lint", true, 38),
            check("e2e", true, 245),
        ],
        ..pull(122, "Persist split widths per window", hub::LEA, "feat/split-memory", days(15))
    });
    add(PullData {
        reviews: vec![ReviewData {
            body: "The fade hides the selected lane too. Keep it at full strength.".to_owned(),
            ..reviewed("PRR_127_lea", hub::LEA, Verdict::RequestChanges, days(3))
        }],
        ..pull(127, "Fade the lanes of merged branches", hub::ANA, "graph/fade-merged", days(6))
    });
    add(PullData {
        reviews: vec![reviewed("PRR_132_kenji", hub::KENJI, Verdict::Approve, days(1))],
        ..pull(132, "Group the toolbar's sync buttons", hub::ANA, "ui/sync-group", days(3))
    });
    add(PullData {
        draft: true,
        checks: vec![check("build", true, 123)],
        ..pull(133, "Show the filter's match count", hub::ANA, "ui/filter-count", days(1))
    });
    add(PullData {
        requested: vec![Request { person: hub::ANA, at: days(2) }],
        ..pull(135, "Keep lane colours across fetches", hub::KENJI, "graph/keep-lane-colours", days(4))
    });
}

/// A time on a day of October 2026, UTC.
fn oct(day: u32, hour: u32) -> Timestamp {
    t(&format!("2026-10-{day:02}T{hour:02}:00:00Z"))
}

/// Makes `b`'s branch in a worktree of its own, forked from main as it was at 06:00 on its day,
/// with its commits an hour apart from 09:00.
fn branch<'w>(w: &'w World, r: &RepoBuilder<'_>, b: &Branch) -> RepoBuilder<'w> {
    let fork = r.git(&["rev-list", "-1", &format!("--before={}", oct(b.day, 7)), "main"]);
    let path = worktrees_dir(w).join(b.worktree);
    r.git(&["worktree", "add", "--quiet", "-b", b.name, &path.display().to_string(), fork.trim()]);
    let wt = w.open(&path);
    for (i, (file, subject)) in b.commits.iter().enumerate() {
        wt.commit(b.author, oct(b.day, 9 + i as u32), subject, &[file]);
    }
    wt
}

/// [`branch`], pushed, then its worktree and local branch removed: only GitHub has it.
fn remote_only(w: &World, r: &RepoBuilder<'_>, b: &Branch) {
    let wt = branch(w, r, b);
    wt.push(b.name);
    r.git(&["worktree", "remove", "--force", &wt.path().display().to_string()]);
    r.git(&["branch", "--quiet", "-D", b.name]);
}

/// Dates a file's last edit, as the person's editor left it.
fn touched(r: &RepoBuilder<'_>, file: &str, at: Timestamp) {
    let f = std::fs::File::options()
        .write(true)
        .open(r.path().join(file))
        .unwrap_or_else(|e| panic!("open {file} to date it: {e}"));
    f.set_modified(at.into()).unwrap_or_else(|e| panic!("date {file}: {e}"));
}
