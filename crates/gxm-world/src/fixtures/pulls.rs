//! atlas-web's pull requests on the world's GitHub, as `design/pr-flow` shows them: Lea's
//! `fix/avatar-cache` open as #131 (one failing check, Kenji's approval, Ana's review
//! requested) and Kenji's `feature/graph-virtualization`, merged as #128 by HEAD's merge commit.
//! The commits they point at are the repository's own.

use std::path::Path;

use super::t;
use crate::World;
use crate::hub::{ANA, CheckData, CommentData, KENJI, LEA, PullData, ReviewData, ThreadData, Verdict};

/// #131's description, as Lea wrote it (GitHub markdown).
pub const DESCRIPTION: &str = "Avatars were evicted in the order they were loaded, so the ones on screen were often the first to go and flickered back in while scrolling.

- Evict by `last_paint` instead of insertion order
- Count hits, misses and evictions in `AvatarMetrics`

Fixes #117 · History stutters past 10k commits";

pub fn atlas_pulls(w: &World, atlas: &Path) {
    let sha = |rev: &str| w.git(atlas, &["rev-parse", rev]).trim().to_owned();
    let comment = |author, at: &str, body: &str| CommentData { author, body: body.to_owned(), at: t(at), review: None };
    let review = |id: &str, author, at: &str| ReviewData {
        id: id.to_owned(),
        author,
        verdict: Some(if id == "PRR_kenji" { Verdict::Approve } else { Verdict::Comment }),
        body: String::new(),
        at: t(at),
        sent: Vec::new(),
    };
    let check = |name: &str, passed, title: &str, seconds| CheckData {
        name: name.to_owned(),
        passed,
        title: title.to_owned(),
        seconds,
    };
    let mut kenjis = comment(KENJI, "2026-11-11T12:00:00Z", "Does `pop_min` stay O(log n) once the heap holds 10k avatars?");
    kenjis.review = Some("PRR_kenji".to_owned());
    let mut leas = comment(LEA, "2026-11-11T13:00:00Z", "Yes, a binary heap keyed by paint time. There's a bench in metrics.rs.");
    leas.review = Some("PRR_lea".to_owned());
    w.hub().add_pull(
        "studio",
        "atlas-web",
        PullData {
            number: 131,
            title: "Evict avatars by last paint, not insert".to_owned(),
            body: DESCRIPTION.to_owned(),
            merged: false,
            author: LEA,
            head_ref: "fix/avatar-cache".to_owned(),
            head_sha: sha("fix/avatar-cache"),
            base_ref: "main".to_owned(),
            merge_commit: None,
            created: t("2026-11-11T10:00:00Z"),
            commits: 2,
            requested: vec![ANA],
            reviews: vec![review("PRR_kenji", KENJI, "2026-11-11T12:00:00Z"), review("PRR_lea", LEA, "2026-11-11T13:00:00Z")],
            comments: vec![
                comment(LEA, "2026-11-11T10:02:00Z", "Ready for review: this is the flicker from #117."),
                comment(KENJI, "2026-11-11T12:01:00Z", "Approving. One question about the heap, inline."),
            ],
            threads: vec![ThreadData {
                id: "PRRT_1".to_owned(),
                path: "src/avatars/cache.rs".to_owned(),
                line: 5,
                side: "RIGHT",
                resolved: false,
                comments: vec![kenjis, leas],
            }],
            checks: vec![
                check("lint", true, "", 38),
                check("unit tests", true, "", 72),
                check("journeys · first_look", false, "failed at step 20", 123),
                check("design fidelity", true, "", 41),
            ],
        },
    );
    w.hub().add_pull(
        "studio",
        "atlas-web",
        PullData {
            number: 128,
            title: "Virtualize commit rows past 10k".to_owned(),
            body: "Rows outside the viewport are no longer laid out.".to_owned(),
            merged: true,
            author: KENJI,
            head_ref: "feature/graph-virtualization".to_owned(),
            head_sha: sha("feature/graph-virtualization"),
            base_ref: "main".to_owned(),
            merge_commit: Some(sha("main")),
            created: t("2026-11-08T16:30:00Z"),
            commits: 3,
            requested: Vec::new(),
            reviews: vec![ReviewData {
                id: "PRR_ana".to_owned(),
                author: ANA,
                verdict: Some(Verdict::Approve),
                body: String::new(),
                at: t("2026-11-11T12:50:00Z"),
                sent: Vec::new(),
            }],
            comments: Vec::new(),
            threads: Vec::new(),
            checks: vec![check("lint", true, "", 35), check("unit tests", true, "", 70)],
        },
    );
}
