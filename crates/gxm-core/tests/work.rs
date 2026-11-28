//! The work list against a world: atlas-web's worktrees and the world's GitHub, read as Ana,
//! then the provider writes the list offers (running failed checks again, merging) checked on
//! GitHub's side, and a review request coming back after new commits.

use std::sync::Arc;

use gxm_core::forge::model::{CheckState, Submission, Verdict};
use gxm_core::forge::{self, Forge};
use gxm_core::remote::Remote;
use gxm_core::work::rules::{self, Next};
use gxm_core::work::{Item, Section, Work};
use gxm_world::World;
use gxm_world::hub::{self, Hub};

fn github(w: &World) -> Arc<dyn Forge> {
    let mut env = w.env().clone();
    env.forge.github_token = Some(hub::TOKEN.to_owned());
    let remote = Remote::parse("git@github.com:studio/atlas-web.git").unwrap_or_else(|| panic!("the remote parses"));
    forge::connect(&env, &remote).unwrap_or_else(|e| panic!("{}", e.message()))
}

/// The list as `(section, what)`: `#122`, or a worktree's folder name.
fn rows(work: &Work) -> Vec<(Section, String)> {
    work.entries
        .iter()
        .map(|e| {
            let what = match &e.item {
                Item::Pull(n) => format!("#{n}"),
                Item::Worktree(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            };
            (e.section, what)
        })
        .collect()
}

#[test]
fn ana_reads_her_work_reruns_failed_checks_and_merges_what_is_ready() {
    let w = World::new();
    let m = gxm_world::fixtures::mockup(&w);
    let gh = github(&w);
    let read = || Work::read(w.env(), &m.atlas_web, Some(&*gh)).unwrap_or_else(|e| panic!("read the work: {e}"));

    // Three sections, each oldest first; #129 merged, so it's nowhere.
    let work = read();
    let want = [
        (Section::Review, "#122"),
        (Section::Review, "#135"),
        (Section::Review, "#131"),
        (Section::Yours, "#118"),
        (Section::Yours, "#127"),
        (Section::Yours, "#132"),
        (Section::Yours, "#133"),
        (Section::Unproposed, "sidebar-badges"),
    ];
    assert_eq!(rows(&work), want.map(|(s, n)| (s, n.to_owned())));

    // Every checkout, the main one first, with its state.
    let states: Vec<(String, String)> =
        work.local.worktrees.iter().map(|w| (w.name.clone(), w.state(&work.local.base))).collect();
    assert_eq!(states[0], ("atlas-web".to_owned(), "±3 files uncommitted".to_owned()));
    assert!(states.contains(&("filter-count".to_owned(), "clean · 2 unpushed".to_owned())), "{states:?}");
    assert!(states.contains(&("sidebar-badges".to_owned(), "±7 files uncommitted".to_owned())), "{states:?}");

    // #118 conflicts in the file #129 changed and in three main changed without a pull request,
    // and says so with its other reasons.
    let p118 = work.pull(118).unwrap_or_else(|| panic!("#118 is open"));
    let conflict = work.conflict(118).unwrap_or_else(|| panic!("#118 conflicts"));
    let files = ["src/avatars/cache.rs", "src/sidebar.rs", "src/theme/brass.json", "src/theme/motion.rs"];
    assert_eq!((conflict.files.as_slice(), conflict.since), (&files.map(str::to_owned)[..], Some(129)));
    let now = w.env().now();
    let why: Vec<String> = rules::stuck(p118, "ana-ruiz", Some(conflict)).iter().map(|r| r.sentence(now)).collect();
    assert_eq!(
        why,
        [
            "Conflicts with main in src/avatars/cache.rs and 3 more files, since #129 landed",
            "2 checks are failing",
            "You haven't answered 2 of Kenji's comments in 19 days",
        ]
    );
    assert_eq!(rules::next(p118), Next::ResolveConflicts);
    assert_eq!(rules::next(work.pull(132).unwrap_or_else(|| panic!("#132"))), Next::Merge);

    // One failed check runs again: GitHub queues it, and the pull request shows it running.
    let safari = p118.checks.iter().find(|c| c.name == "e2e / safari").unwrap_or_else(|| panic!("safari"));
    gh.rerun_check(safari).unwrap_or_else(|e| panic!("rerun safari: {e}"));
    assert_eq!(Hub::of(&w).reruns("studio", "atlas-web", 118), ["e2e / safari"]);
    let p118 = gh.pull(118).unwrap_or_else(|e| panic!("read #118: {e}"));
    let state = |name: &str| p118.checks.iter().find(|c| c.name == name).map(|c| c.state);
    assert_eq!((state("e2e / safari"), state("e2e / firefox")), (Some(CheckState::Pending), Some(CheckState::Failed)));
    // Then the rest of the failed ones, and only those.
    gh.rerun_failed(&p118).unwrap_or_else(|e| panic!("rerun the failed checks: {e}"));
    assert_eq!(Hub::of(&w).reruns("studio", "atlas-web", 118), ["e2e / safari", "e2e / firefox"]);

    // #132 is approved and green: merging it lands on GitHub, and it leaves the list.
    let p132 = gh.pull(132).unwrap_or_else(|e| panic!("read #132: {e}"));
    gh.squash_merge(&p132, "Group the toolbar's sync buttons (#132)").unwrap_or_else(|e| panic!("merge #132: {e}"));
    assert!(Hub::of(&w).merged("studio", "atlas-web", 132).is_some(), "#132 merged on GitHub");
    assert!(!rows(&read()).contains(&(Section::Yours, "#132".to_owned())));

    // Ana reviews #122: it stops waiting on her.
    let p122 = gh.pull(122).unwrap_or_else(|e| panic!("read #122: {e}"));
    let review =
        Submission { verdict: Verdict::Comment, body: "Looks right.".to_owned(), comments: vec![], replies: vec![] };
    gh.submit(&p122, &review).unwrap_or_else(|e| panic!("review #122: {e}"));
    assert!(!rows(&read()).contains(&(Section::Review, "#122".to_owned())));
    // Asked again with nothing new to see, it stays away.
    Hub::of(&w).request_review("studio", "atlas-web", 122, hub::ANA, now);
    assert!(!rows(&read()).contains(&(Section::Review, "#122".to_owned())));
    // Lea pushes an hour later, and asks again: it's back, aged from the new request.
    let later = now + jiff::SignedDuration::from_hours(1);
    let wt = w.root().join("lea-split");
    let atlas = w.open(&m.atlas_web);
    atlas.git(&["worktree", "add", "--quiet", "--detach", &wt.display().to_string(), "origin/feat/split-memory"]);
    let lea = w.open(&wt);
    let pushed = lea.commit(gxm_world::fixtures::LEA, later, "Keep widths per display", &["src/split.rs"]);
    lea.git(&["push", "--quiet", "origin", "HEAD:refs/heads/feat/split-memory"]);
    Hub::of(&w).moved("studio", "atlas-web", 122, &pushed);
    Hub::of(&w).request_review("studio", "atlas-web", 122, hub::ANA, later);
    let work = read();
    let back = work.entries.iter().find(|e| e.item == Item::Pull(122)).unwrap_or_else(|| panic!("#122 is back"));
    assert_eq!((back.section, back.since), (Section::Review, later));
}
