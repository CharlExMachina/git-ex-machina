//! The CLI, driven as a person would from a terminal, inside a hermetic world.

use std::path::Path;
use std::process::Command;

use gxm_world::World;

#[expect(clippy::expect_used, reason = "a test helper: a broken binary fails the test loudly")]
fn gxm(w: &World, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_gxm"))
        .args(args)
        .envs(w.env().child_vars())
        .current_dir(w.code())
        .output()
        .expect("gxm runs");
    assert!(out.status.success(), "gxm {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("gxm prints UTF-8")
}

/// Runs gxm, whatever it exits with.
#[expect(clippy::expect_used, reason = "a test helper: a broken binary fails the test loudly")]
fn gxm_run(w: &World, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gxm"))
        .args(args)
        .envs(w.env().child_vars())
        .current_dir(w.code())
        .output()
        .expect("gxm runs")
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

    // Its pull requests on GitHub, matched to the commits in the history.
    let prs = gxm(&w, &["prs", "atlas-web"]);
    assert!(
        prs.contains("#131   Open    fix/avatar-cache → main  Evict avatars by last paint, not insert  · Lea Brandt · 2 commits here"),
        "{prs}"
    );
    assert!(prs.contains("#128   Merged  feature/graph-virtualization → main"), "{prs}");
}

#[test]
fn someone_adds_and_clones_repositories_from_the_terminal() {
    use gxm_world::fixtures::ANA;
    let w = World::new();
    let notes = w.repo("github.com", "personal", "field-notes");
    notes.commit(ANA, w.ago(gxm_world::days(2)), "Start the field notes", &["README.md"]);
    notes.push("main");
    let site = w.repo("github.com", "studio", "site");
    site.commit(ANA, w.ago(gxm_world::days(1)), "Scaffold the site", &["index.html"]);
    site.push("main");
    std::fs::remove_dir_all(site.path()).unwrap();

    // A folder inside a repository adds the repository.
    std::fs::create_dir_all(w.code().join("field-notes/drafts")).unwrap();
    assert_eq!(gxm(&w, &["add", "~/code/field-notes/drafts"]), "Added ~/code/field-notes\n");
    // A GitHub page clones over SSH, into ~/code.
    let cloned = gxm(&w, &["clone", "https://github.com/studio/site"]);
    assert_eq!(cloned, "Cloning git@github.com:studio/site.git into ~/code/site\nAdded ~/code/site\n");
    let repos = gxm(&w, &["repos"]);
    assert!(repos.contains("field-notes") && repos.contains("site"), "{repos}");
}

#[test]
fn someone_signs_in_to_github_from_the_terminal() {
    use std::io::{BufRead as _, BufReader};
    let w = World::new();
    w.sign_out();
    // Not signed in yet: status says so, and fails, for scripts.
    assert!(!gxm_run(&w, &["auth", "status"]).status.success());

    // Login shows the code and where to enter it, then waits until it's approved on GitHub.
    let mut login = Command::new(env!("CARGO_BIN_EXE_gxm"))
        .args(["auth", "login"])
        .envs(w.env().child_vars())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(login.stdout.take().unwrap()).lines().map(Result::unwrap);
    assert_eq!(lines.next().unwrap(), format!("Your one-time code: {}", gxm_world::hub::USER_CODE));
    assert_eq!(lines.next().unwrap(), "Enter it at https://github.com/login/device to sign in. Waiting…");
    // The world's GitHub approves after a couple of asks.
    assert_eq!(lines.next().unwrap(), "Signed in to github.com as ana-ruiz");
    assert!(login.wait().unwrap().success());

    // A separate run finds the sign-in, checks it with GitHub, and hands out the token.
    assert_eq!(gxm(&w, &["auth", "status"]), "Signed in to github.com as ana-ruiz\n");
    assert_eq!(gxm(&w, &["auth", "token"]), format!("{}\n", gxm_world::hub::TOKEN));
    assert_eq!(gxm(&w, &["auth", "logout"]), "Signed out of github.com (ana-ruiz)\n");
    let status = gxm_run(&w, &["auth", "status"]);
    assert!(!status.status.success());
    assert_eq!(String::from_utf8_lossy(&status.stderr), "gxm: not signed in to github.com: gxm auth login\n");
}
