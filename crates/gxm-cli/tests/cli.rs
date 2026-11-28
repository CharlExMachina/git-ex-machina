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

#[test]
fn someone_reads_the_settings_of_a_repository_and_tidies_its_merged_branches_from_the_terminal() {
    let w = World::new();
    let m = gxm_world::fixtures::mockup(&w);
    gxm_world::fixtures::atlas_settings(&w);
    gxm_world::fixtures::atlas_branches(&w, &m.atlas_web);
    let atlas = m.atlas_web.display().to_string();

    let settings = gxm(&w, &["settings", &atlas]);
    assert!(settings.contains("studio/atlas-web  (private, main; you: Admin)"), "{settings}");
    assert!(settings.contains("merges by merge, squash · auto-merge on · delete branches on merge off"), "{settings}");
    assert!(settings.contains("commits as Ana Ruiz <ana@example.com>"), "{settings}");
    assert!(settings.contains("origin git@github.com:studio/atlas-web.git"), "{settings}");

    // Done branches first, each with why; the rest with why they stay.
    let tidy = gxm(&w, &["tidy", &atlas]);
    let lines: Vec<String> = tidy.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    assert_eq!(
        lines,
        [
            "✓ feature/graph-virtualization local and origin #128 merged",
            "✓ sidebar-spring local same changes, rebased",
            "✓ tune/brass-light origin in the default branch",
            "· fix/avatar-cache local and origin #131 is still open",
            "· spike/canvas-text origin 1 commit not in the default branch",
            "· wip/lane-colors local 2 commits only on this computer",
        ],
        "{tidy}"
    );

    // Deleting takes the done ones, wherever they are, and nothing else.
    let deleted = gxm(&w, &["tidy", &atlas, "--delete"]);
    assert!(deleted.ends_with("Deleted 3 branches\n"), "{deleted}");
    let remote = w.git(&w.remote("github.com", "studio", "atlas-web"), &["branch", "--format=%(refname:short)"]);
    assert_eq!(remote.lines().collect::<Vec<_>>(), ["fix/avatar-cache", "main", "spike/canvas-text"]);
    let local = w.git(&m.atlas_web, &["branch", "--format=%(refname:short)"]);
    assert_eq!(local.lines().collect::<Vec<_>>(), ["fix/avatar-cache", "main", "wip/lane-colors"]);
    let again = gxm(&w, &["tidy", &atlas]);
    assert!(!again.contains('✓'), "nothing left to tidy:\n{again}");
}

#[test]
fn someone_releases_the_next_version_by_pushing_a_tag_from_the_terminal() {
    use gxm_world::fixtures::{ANA, KENJI};
    let w = World::new();
    let beacon = w.repo("github.com", "studio", "beacon");
    beacon.write(
        ".github/workflows/release.yml",
        "name: release\non:\n  push:\n    tags: ['v*']\njobs:\n  a:\n    steps:\n      - run: gh release create \"$GITHUB_REF_NAME\" --notes-from-tag\n",
    );
    beacon.commit(ANA, w.ago(gxm_world::days(3)), "Release from version tags", &["src/main.rs"]);
    beacon.tag("v0.1.0");
    beacon.commit(KENJI, w.ago(gxm_world::days(1)), "Fix the --json flag", &["src/cli.rs"]);
    beacon.push("main");
    let repo = w.code().join("beacon");
    let repo = repo.to_str().unwrap();

    // Whether a pushed tag releases, and what comes next.
    let tags = gxm(&w, &["tags", repo]);
    assert_eq!(
        tags,
        "Releases: pushing a tag matching v* starts the release workflow (.github/workflows/release.yml)\nNotes: the tag's message (gxm tag NAME -m \"$(cat notes.md)\")\nNext tag: v0.1.1\n"
    );

    // Tagging HEAD and pushing it is the release.
    // The notes are the tag's message, markdown headings and all.
    let notes = "## Fixed\n\n- The --json flag\n";
    let tagged = gxm(&w, &["tag", "v0.1.1", "-C", repo, "-m", notes, "--push"]);
    assert!(tagged.starts_with("Tagged v0.1.1 on "), "{tagged}");
    assert!(
        tagged.ends_with("Pushed v0.1.1 to origin\nThat starts the release workflow (.github/workflows/release.yml)\n"),
        "{tagged}"
    );
    let remote = w.remote("github.com", "studio", "beacon");
    assert_eq!(w.git(&remote, &["tag", "-l", "--format=%(contents)", "v0.1.1"]), notes);

    // A name that's taken, on another commit, is refused with where it is.
    let taken = gxm_run(&w, &["tag", "v0.1.1", "HEAD~1", "-C", repo]);
    assert!(!taken.status.success());
    assert!(String::from_utf8_lossy(&taken.stderr).contains("v0.1.1 already exists, on "), "{taken:?}");
}

#[test]
fn someone_checks_their_agents_and_runs_a_prompt_from_the_terminal() {
    use gxm_world::fixtures::agents::{self, Agent};
    let w = World::new();

    // Nothing installed: it says so, and how to add one.
    let none = gxm(&w, &["agents"]);
    assert!(none.contains("claude-code  Claude Code  -          not installed"), "{none}");
    assert!(none.contains("No coding agent found."), "{none}");
    let refused = gxm_run(&w, &["agents", "run", "Resolve the conflicts"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("no agent harness is ready"));

    // Claude Code signed out, Codex ready: Codex is the one to use, and Claude says why not.
    agents::claude(&w, false, Agent::Works);
    agents::codex(&w, true);
    let listed = gxm(&w, &["agents"]);
    let lines: Vec<&str> = listed.lines().collect();
    assert_eq!(lines[0], "  claude-code  Claude Code  2.1.111    not usable", "{listed}");
    assert_eq!(lines[1], "    Claude Code is not authenticated. Run `claude auth login` and try again.");
    assert_eq!(lines[2], "★ codex        Codex        0.156.1    ready, signed in as ana@example.com");
    let json: serde_json::Value = serde_json::from_str(&gxm(&w, &["agents", "--json"])).expect("valid JSON");
    assert_eq!((json[1]["id"].as_str(), json[1]["preferred"].as_bool()), (Some("codex"), Some(true)));

    // A run streams the session, line by line, to its end, in the folder it's given.
    let site = w.code().join("site");
    std::fs::create_dir_all(&site).expect("make site");
    std::fs::write(site.join("README.md"), "# site\n").expect("write README");
    let ran = gxm(&w, &["agents", "run", "--in", "~/code/site", "Resolve", "the", "conflicts"]);
    assert_eq!(
        ran,
        "● started session 01a0d5eb-f0c0-7481-a417-5e8871805cbc\n→ command: git status\n✎ edited README.md\nResolved: README.md says so.\n✓ done\n"
    );
    assert!(std::fs::read_to_string(site.join("README.md")).expect("read README").ends_with("Resolved by Codex.\n"));

    // Asking for a harness that can't run says why it couldn't start.
    agents::claude(&w, true, Agent::Works);
    assert_eq!(gxm(&w, &["agents", "prefer", "claude-code"]), "Claude Code is preferred\n");
    assert!(gxm(&w, &["agents"]).lines().next().is_some_and(|l| l.starts_with("★ claude-code")));
}
