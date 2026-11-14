//! The CLI, driven as a person would from a terminal, inside a hermetic world.

use std::path::Path;
use std::process::Command;

use gxm_world::World;

#[expect(clippy::expect_used, reason = "a test helper: a broken binary fails the test loudly")]
fn gxm(w: &World, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_gxm"))
        .args(args)
        .envs(w.env().child_vars())
        .current_dir(w.root().join("code"))
        .output()
        .expect("gxm runs");
    assert!(out.status.success(), "gxm {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("gxm prints UTF-8")
}

#[test]
fn someone_checks_their_workspace_and_reads_a_history_from_the_terminal() {
    let w = World::new();
    gxm_world::fixtures::mockup(&w);

    // The workspace at a glance: pins first, each repository's branch, owner, provider and state.
    let repos = gxm(&w, &["repos"]);
    let line = |name: &str| {
        repos.lines().find(|l| l.contains(name)).unwrap_or_else(|| panic!("{name} missing:\n{repos}")).to_owned()
    };
    assert!(line("atlas-web").starts_with('★'), "atlas-web is pinned");
    assert!(line("atlas-web").contains("↑4 3 changed"));
    assert!(
        line("payments-api").contains("release/2.3")
            && line("payments-api").contains("Bitbucket")
            && line("payments-api").contains("↓4")
    );
    assert!(line("infra-terraform").contains("GitLab") && line("infra-terraform").contains("↓1"));
    assert!(line("brand-tokens").contains("12 changed"));
    assert!(line("dotfiles").contains("personal"));

    // The same, as JSON for scripts.
    let json: serde_json::Value = serde_json::from_str(&gxm(&w, &["repos", "--json"])).expect("valid JSON");
    let atlas = &json.as_array().expect("a list")[0];
    assert_eq!(
        (atlas["name"].as_str(), atlas["ahead"].as_u64(), atlas["provider"].as_str()),
        (Some("atlas-web"), Some(4), Some("GitHub"))
    );

    // The history with its graph, exactly: commit ids are deterministic in a world.
    let log = gxm(&w, &["log", "atlas-web", "-n", "12"]);
    gxm_world::golden(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/atlas-web-log.txt"), &log);
}
