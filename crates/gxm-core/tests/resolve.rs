//! Resolving #118's conflicts with main against a world: the merge in its worktree, each
//! conflict where git put it and why main changed it (from blame and the world's GitHub),
//! aborting back to exactly where it was, and committing and pushing a resolution.

use std::sync::Arc;

use gxm_core::forge::{self, Forge};
use gxm_core::remote::Remote;
use gxm_core::resolve::{Side, Target, merge};
use gxm_world::World;
use gxm_world::hub;

fn github(w: &World) -> Arc<dyn Forge> {
    let mut env = w.env().clone();
    env.forge.github_token = Some(hub::TOKEN.to_owned());
    let remote = Remote::parse("git@github.com:studio/atlas-web.git").unwrap_or_else(|| panic!("the remote parses"));
    forge::connect(&env, &remote).unwrap_or_else(|e| panic!("{}", e.message()))
}

fn target(w: &World, repo: &std::path::Path) -> Target {
    Target {
        repo: repo.to_path_buf(),
        worktree: gxm_world::fixtures::worktrees_dir(w).join("avatar-loading"),
        worktree_name: "avatar-loading".to_owned(),
        branch: "perf/avatar-loading".to_owned(),
        base: "main".to_owned(),
        remote: "origin".to_owned(),
        number: 118,
        title: "Load avatars in parallel".to_owned(),
    }
}

#[test]
fn ana_merges_main_into_118_aborts_then_resolves_and_pushes() {
    let w = World::new();
    let m = gxm_world::fixtures::mockup(&w);
    let gh = github(&w);
    let t = target(&w, &m.atlas_web);
    let env = w.env();
    let head = w.git(&t.worktree, &["rev-parse", "HEAD"]).trim().to_owned();

    let merge = merge::begin(env, &t, Some(&*gh)).unwrap_or_else(|e| panic!("begin: {e}"));
    let places: Vec<(String, usize, Option<String>)> = merge
        .files
        .iter()
        .flat_map(|f| (0..f.conflicts.len()).map(move |i| (f.path.clone(), f.line_of(i), f.conflicts[i].scope.clone())))
        .collect();
    let want = [
        ("src/avatars/cache.rs", 49, Some("fn load_missing")),
        ("src/sidebar.rs", 40, Some("fn collapse")),
        ("src/theme/brass.json", 5, None),
        ("src/theme/motion.rs", 10, Some("mod springs")),
    ];
    assert_eq!(places, want.map(|(p, l, s)| (p.to_owned(), l, s.map(str::to_owned))));
    assert_eq!(merge.head, head);
    // Main's side of the first came with #129; the others with a commit straight on main.
    let (_, cache) = merge.conflict(0).unwrap_or_else(|| panic!("a first conflict"));
    let why = cache.why.as_ref().unwrap_or_else(|| panic!("why main changed cache.rs"));
    let pull = why.pull.as_ref().unwrap_or_else(|| panic!("#129 behind {why:?}"));
    assert_eq!(
        (pull.number, pull.title.as_str(), pull.author.as_str()),
        (129, "Fix avatar cache eviction", "Lea Brandt")
    );
    assert_eq!(pull.reason.as_deref(), Some("Released as v0.4.0."));
    for n in 1..4 {
        let (f, c) = merge.conflict(n).unwrap_or_else(|| panic!("conflict {n}"));
        let why = c.why.as_ref().unwrap_or_else(|| panic!("why main changed {}", f.path));
        assert_eq!((why.subject.as_str(), why.pull.is_none()), ("Spring the sidebar collapse", true), "{}", f.path);
    }

    // Started again while it's under way, it picks the same merge up.
    let again = merge::begin(env, &t, None).unwrap_or_else(|e| panic!("begin again: {e}"));
    assert_eq!(again.total(), 4);

    merge::abort(env, &t, &merge.head).unwrap_or_else(|e| panic!("abort: {e}"));
    assert_eq!(w.git(&t.worktree, &["status", "--porcelain"]), "");
    assert_eq!(w.git(&t.worktree, &["rev-parse", "HEAD"]).trim(), head);

    // Resolved, committed and pushed: a merge of main on GitHub.
    let mut merge = merge::begin(env, &t, None).unwrap_or_else(|e| panic!("begin: {e}"));
    for n in 0..merge.total() {
        let c = merge.conflict_mut(n).unwrap_or_else(|| panic!("conflict {n}"));
        c.resolution =
            Some(gxm_core::resolve::Resolution { text: c.side(Side::Both), by: gxm_core::resolve::By::Hand });
    }
    assert!(merge::commit_and_push(env, &t, &merge.files).is_err(), "nothing written yet: markers are still there");
    merge::write(env, &t, &merge.files).unwrap_or_else(|e| panic!("write: {e}"));
    merge::commit_and_push(env, &t, &merge.files).unwrap_or_else(|e| panic!("commit and push: {e:?}"));
    let pushed = w.remote_head("github.com", "studio", "atlas-web", "perf/avatar-loading");
    let parents = w.git(&w.remote("github.com", "studio", "atlas-web"), &["rev-list", "--parents", "-n", "1", &pushed]);
    let main = w.remote_head("github.com", "studio", "atlas-web", "main");
    assert_eq!(parents.split_whitespace().collect::<Vec<_>>(), [pushed.as_str(), head.as_str(), main.as_str()]);
}
