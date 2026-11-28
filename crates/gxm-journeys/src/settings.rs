//! Repository settings: everything GitHub's settings pages hold, and what lives on this
//! computer, changed from the app, with each change checked where it landed (the world's
//! GitHub, the repository's `.git/config`, its remotes).

use gxm_app::keys::{self, setting, setting_field, setting_item};
use gxm_world::World;
use gxm_world::hub::Hub;

use crate::{Journey, Press};

/// The demo workspace, with atlas-web's settings on GitHub and branches to tidy up.
fn settings_world(w: &World) {
    let m = gxm_world::fixtures::mockup(w);
    gxm_world::fixtures::atlas_settings(w);
    gxm_world::fixtures::atlas_branches(w, &m.atlas_web);
}

fn hub(w: &World) -> gxm_world::hub::admin::Settings {
    Hub::of(w).settings("studio", "atlas-web").unwrap_or_else(|| panic!("the world has no studio/atlas-web"))
}

fn atlas(w: &World) -> std::path::PathBuf {
    w.code().join("atlas-web")
}

/// `git config --get key` in atlas-web, or "" when unset.
fn config(w: &World, key: &str) -> String {
    gxm_core::git::run(w.env(), &atlas(w), &["config", "--get", key]).unwrap_or_default().trim().to_owned()
}

fn expect(what: &str, ok: bool, got: impl std::fmt::Debug) -> Result<(), String> {
    if ok { Ok(()) } else { Err(format!("{what}: {got:?}")) }
}

const SAVED: &str = "Saved on GitHub";

pub fn repository_settings() -> Journey {
    Journey::new(
        "repository_settings",
        "Someone who runs atlas-web changes its settings from the app, every page GitHub has and what lives on their computer, tidies up its merged branches, and finally deletes it on GitHub.",
        settings_world,
    )
    // The app opens on Work; this journey is about the history, one click away.
    .tap(keys::VIEW_HISTORY)
    // The button beside commit search opens the settings, on General.
    .tap(keys::SETTINGS)
    .wait_for(keys::SETTINGS_DIALOG, "studio/atlas-web")
    .wait_for(setting_field("description"), "The desktop client for Atlas")
    .see_in(setting_field("name"), "atlas-web")
    .see_in(setting_field("homepage"), "https://atlas.studio")
    .see_in(setting_item("topic", "rust"), "rust")
    .see_in(keys::settings_section("Danger zone"), "Danger zone")
    .gone(keys::SETTINGS_NOTICE)
    .gone(setting("save-about"))
    // Text waits for Save; a topic goes in with Return; a box saves the moment it's ticked.
    .tap(setting_field("description"))
    .press(Press::cmd("a"))
    .type_text("History, the commit graph and reviews, on your desktop.")
    .see_in(setting("save-about"), "Save")
    .mark("general")
    .tap(setting("save-about"))
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .gone(setting("save-about"))
    .tap(setting_field("topic"))
    .type_text("graph")
    .press(Press::ENTER)
    .wait_for(setting_item("topic", "graph"), "graph")
    .tap(setting("remove-topic-desktop-app"))
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .gone(setting_item("topic", "desktop-app"))
    .tap(setting("wiki"))
    .tap(setting("discussions"))
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .world_check("GitHub has the new description, topics, wiki and discussions", |w| {
        let s = hub(w);
        expect(
            "settings",
            s.description == "History, the commit graph and reviews, on your desktop."
                && s.topics == ["rust", "git", "graph"]
                && s.has_wiki
                && s.has_discussions,
            (&s.description, &s.topics, s.has_wiki, s.has_discussions),
        )
    })
    // Branches: the default branch, deleting on merge here and there, and the tidy-up.
    .tap(keys::settings_section("Branches"))
    .wait_for(setting_item("branch", "feature/graph-virtualization"), "#128 merged")
    .see_in(setting_item("default-branch", "main"), "main")
    .see_in(setting("delete-branch-on-merge"), "On GitHub")
    .see_in(setting("delete-local-merged"), "This computer")
    .see_in(setting_item("branch", "sidebar-spring"), "same changes, rebased")
    .see_in(setting_item("branch", "tune/brass-light"), "in the default branch")
    .see_in(setting("delete-branches"), "Delete 3 branches")
    .see_in(keys::SETTINGS_PAGE, "2 on GitHub and 2 on this computer")
    .gone(setting_item("branch", "wip/lane-colors"))
    .tap(setting("show-kept"))
    .see_in(setting_item("branch", "fix/avatar-cache"), "#131 is still open")
    .see_in(setting_item("branch", "spike/canvas-text"), "1 commit not in the default branch")
    .see_in(setting_item("branch", "wip/lane-colors"), "2 commits only on this computer")
    .tap(setting("delete-branch-on-merge"))
    .tap(setting("delete-local-merged"))
    .tap(setting("fetch-prune"))
    .wait_for(keys::SETTINGS_STATUS, "Saved in .git/config")
    .mark("branches")
    // Unticking a branch keeps it.
    .tap(setting_item("branch", "sidebar-spring"))
    .see_in(setting("delete-branches"), "Delete 2 branches")
    .scroll_to(setting("delete-branches"))
    .tap(setting("delete-branches"))
    .wait_for(keys::SETTINGS_STATUS, "Deleted 2 branches")
    .gone(setting_item("branch", "feature/graph-virtualization"))
    .see_in(setting_item("branch", "sidebar-spring"), "same changes, rebased")
    .world_check("the done branches are gone from GitHub and here, the rest stayed", |w| {
        let remote = w.git(&w.remote("github.com", "studio", "atlas-web"), &["branch", "--format=%(refname:short)"]);
        let local = w.git(&atlas(w), &["branch", "--format=%(refname:short)"]);
        let s = hub(w);
        expect(
            "branches and settings",
            remote.lines().collect::<Vec<_>>()
                == [
                    "feat/split-memory",
                    "fix/avatar-cache",
                    "graph/fade-merged",
                    "graph/keep-lane-colours",
                    "main",
                    "perf/avatar-loading",
                    "spike/canvas-text",
                    "ui/filter-count",
                    "ui/sync-group",
                ]
                && local.lines().collect::<Vec<_>>()
                    == [
                        "fix/avatar-cache",
                        "graph/fade-merged",
                        "main",
                        "perf/avatar-loading",
                        "sidebar-spring",
                        "ui/filter-count",
                        "ui/sidebar-badges",
                        "ui/sync-group",
                        "wip/lane-colors",
                    ]
                && s.delete_branch_on_merge
                && config(w, "gxm.deleteMergedBranches") == "true"
                && config(w, "fetch.prune") == "true",
            (remote, local, s.delete_branch_on_merge),
        )
    })
    // Pull requests: how they merge and what the commits say.
    .tap(keys::settings_section("Pull requests"))
    .wait_for(setting("squash"), "Squash")
    .tap(setting("rebase"))
    .tap(setting("squash-message-title-and-description"))
    .tap(setting("auto-merge"))
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .mark("pull-requests")
    .world_check("GitHub allows rebasing, squashes with the description, and no auto-merge", |w| {
        let s = hub(w);
        expect(
            "merging",
            s.allow_rebase_merge && !s.allow_auto_merge && s.squash_merge_commit_message == "PR_BODY",
            (s.allow_rebase_merge, s.allow_auto_merge, &s.squash_merge_commit_message),
        )
    })
    // Rules: one opened and changed, a new one made and deleted.
    .tap(keys::settings_section("Rules"))
    .wait_for(setting_item("rule", "Protect main"), "Pull request (1 approval)")
    .see_in(setting_item("rule", "Version tags"), "Trial")
    .tap(setting_item("rule", "Protect main"))
    .see_in(setting_item("check", "build"), "build")
    .tap(setting("rule-approvals-more"))
    .wait_for(keys::SETTINGS_PAGE, "2 approvals")
    .tap(setting("rule-signed"))
    .tap(setting_field("rule-check"))
    .type_text("journeys")
    .press(Press::ENTER)
    .wait_for(setting_item("check", "journeys"), "journeys")
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .mark("rules")
    .tap(setting("new-rule"))
    .wait_for(setting_item("rule", "Protect the default branch"), "Pull request (1 approval)")
    .tap(setting("delete-rule"))
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .gone(setting_item("rule", "Protect the default branch"))
    .world_check("main needs 2 approvals, signed commits and the journeys check", |w| {
        let s = hub(w);
        let main = &s.rulesets[0];
        let text = main.to_string();
        expect(
            "Protect main",
            s.rulesets.len() == 3 && text.contains("\"required_approving_review_count\":2") && text.contains("required_signatures") && text.contains("journeys"),
            text,
        )
    })
    // People: an invitation sent, a role changed, an invitation cancelled.
    .tap(keys::settings_section("People"))
    .wait_for(setting_item("person", "kenji-mori"), "kenji-mori")
    .see_in(setting_item("team", "design"), "studio/design")
    .see_in(setting_item("invitation", "jo-park"), "hasn’t accepted yet")
    .see_in(keys::SETTINGS_PAGE, "Everyone in studio can read it")
    .tap(setting_field("invite"))
    .type_text("sam-lee")
    .tap(setting("invite-role-triage"))
    .tap(setting("invite"))
    .wait_for(setting_item("invitation", "sam-lee"), "Triage")
    .tap(setting("role-lea-brandt-maintain"))
    .tap(setting("cancel-invite-jo-park"))
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .gone(setting_item("invitation", "jo-park"))
    .mark("people")
    .world_check("Sam is invited to triage, Lea maintains, Jo's invitation is gone", |w| {
        let s = hub(w);
        let invites: Vec<(&str, &str)> = s.invitations.iter().map(|i| (i.login.as_str(), i.permission.as_str())).collect();
        expect("people", invites == [("sam-lee", "triage")] && s.role_of("lea-brandt") == "maintain", (invites, s.role_of("lea-brandt")))
    })
    // Actions.
    .tap(keys::settings_section("Actions"))
    .wait_for(setting("allowed-all"), "Any action")
    .tap(setting("allowed-local"))
    .tap(setting("workflow-write"))
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .mark("actions")
    .world_check("workflows run only studio's actions, with a token that writes", |w| {
        let s = hub(w);
        expect("actions", s.allowed_actions == "local_only" && s.workflow_permissions == "write", (&s.allowed_actions, &s.workflow_permissions))
    })
    // Secrets and variables: a secret sealed here, one deleted, a variable added.
    .tap(keys::settings_section("Secrets and variables"))
    .wait_for(setting_item("secret", "NPM_TOKEN"), "NPM_TOKEN")
    .see_in(setting_item("variable", "NODE_VERSION"), "22")
    .tap(setting_field("secret-name"))
    .type_text("CODECOV_TOKEN")
    .tap(setting_field("secret-value"))
    .type_text("cc-7f3a")
    .not_see("cc-7f3a")
    .tap(setting("add-secret"))
    .wait_for(setting_item("secret", "CODECOV_TOKEN"), "CODECOV_TOKEN")
    .tap(setting("delete-secret-sentry_auth_token"))
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .gone(setting_item("secret", "SENTRY_AUTH_TOKEN"))
    .tap(setting_field("variable-name"))
    .type_text("RELEASE_CHANNEL")
    .tap(setting_field("variable-value"))
    .type_text("beta")
    .tap(setting("add-variable"))
    .wait_for(setting_item("variable", "RELEASE_CHANNEL"), "beta")
    .mark("secrets")
    .tap(setting("scope-dependabot"))
    .wait_for(setting_item("secret", "NPM_TOKEN"), "NPM_TOKEN")
    .gone(setting_item("secret", "CODECOV_TOKEN"))
    .gone(setting_item("variable", "NODE_VERSION"))
    .world_check("GitHub unsealed the secret the app sealed; the variable is there", |w| {
        let s = hub(w);
        expect(
            "secrets",
            s.secret("actions", "CODECOV_TOKEN") == Some("cc-7f3a")
                && s.secret("actions", "SENTRY_AUTH_TOKEN").is_none()
                && s.variables.iter().any(|v| v.name == "RELEASE_CHANNEL" && v.value == "beta"),
            (s.secret("actions", "CODECOV_TOKEN"), s.variables.len()),
        )
    })
    // Environments: staging added, waiting 5 minutes, deploying from protected branches.
    .tap(keys::settings_section("Environments"))
    .wait_for(setting_item("environment", "production"), "Waits 10 minutes")
    .see_in(setting_item("environment", "production"), "ana-ruiz")
    .tap(setting_field("environment-name"))
    .type_text("staging")
    .tap(setting("add-environment"))
    .wait_for(setting_item("environment", "staging"), "Starts at once")
    .tap(setting("wait-more-staging"))
    .wait_for(setting_item("environment", "staging"), "Waits 1 minute")
    .tap(setting("wait-more-staging"))
    .tap(setting("protected-staging"))
    .wait_for(setting_item("environment", "staging"), "Waits 5 minutes")
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .mark("environments")
    .world_check("staging waits 5 minutes and takes protected branches only", |w| {
        let s = hub(w);
        let staging = s.environments.iter().find(|e| e.name == "staging");
        expect("staging", staging.is_some_and(|e| e.wait_timer == 5 && e.protected_branches), staging)
    })
    // Webhooks: one added and pinged, the old one paused.
    .tap(keys::settings_section("Webhooks"))
    .wait_for(setting_item("hook", "https://ci.atlas.studio/github"), "Last delivery: 200 OK")
    .tap(setting_field("hook-url"))
    .type_text("https://chat.atlas.studio/releases")
    .tap(setting("hook-events-everything"))
    .tap(setting("add-hook"))
    .wait_for(setting_item("hook", "https://chat.atlas.studio/releases"), "Not delivered yet")
    .tap(setting("ping-https://chat.atlas.studio/releases"))
    .wait_for(setting_item("hook", "https://chat.atlas.studio/releases"), "Last delivery: 200 OK")
    .tap(setting("active-https://ci.atlas.studio/github"))
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .mark("webhooks")
    .world_check("the new hook hears everything and was delivered; the CI hook is paused", |w| {
        let s = hub(w);
        let new = s.hooks.iter().find(|h| h.url.contains("chat"));
        expect(
            "hooks",
            new.is_some_and(|h| h.events == ["*"] && h.last.is_some()) && !s.hooks[0].active,
            &s.hooks,
        )
    })
    // Pages: from main's docs folder, on a custom domain.
    .tap(keys::settings_section("Pages"))
    .wait_for(keys::SETTINGS_PAGE, "Published at https://studio.github.io/atlas-web/")
    .tap(setting_item("pages-branch", "main"))
    .tap(setting("pages-docs"))
    .tap(setting_field("cname"))
    .type_text("docs.atlas.studio")
    .tap(setting("save-domain"))
    .wait_for(keys::SETTINGS_PAGE, "Published at https://docs.atlas.studio/")
    .mark("pages")
    .world_check("Pages publishes main's docs folder on docs.atlas.studio", |w| {
        let p = hub(w).pages.ok_or("Pages is off")?;
        expect("pages", p.branch == "main" && p.path == "/docs" && p.cname.as_deref() == Some("docs.atlas.studio"), p)
    })
    // Code security, a deploy key and an autolink.
    .tap(keys::settings_section("Code security"))
    .wait_for(setting("secret-scanning"), "Secret scanning")
    .tap(setting("push-protection"))
    .tap(setting("dependabot-updates"))
    .wait_for(keys::SETTINGS_STATUS, SAVED)
    .mark("code-security")
    .tap(keys::settings_section("Deploy keys"))
    .wait_for(setting_item("deploy-key", "Release bot"), "Read and write")
    .tap(setting_field("key-title"))
    .type_text("Docs builder")
    .tap(setting_field("key"))
    .type_text("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBd5yHtM2Vz2bO9U0ubbfNsu9xS7lHvbHQi1vMuJxKqA docs")
    .tap(setting("add-key"))
    .wait_for(setting_item("deploy-key", "Docs builder"), "Read only")
    .mark("deploy-keys")
    .tap(keys::settings_section("Autolinks"))
    .wait_for(setting_item("autolink", "ATL-"), "tracker.atlas.studio")
    .tap(setting_field("autolink-prefix"))
    .type_text("DESIGN-")
    .tap(setting_field("autolink-template"))
    .type_text("https://design.atlas.studio/issue/<num>")
    .tap(setting("add-autolink"))
    .wait_for(setting_item("autolink", "DESIGN-"), "design.atlas.studio")
    .mark("autolinks")
    .world_check("push protection and security updates are on; the key and autolink are there", |w| {
        let s = hub(w);
        expect(
            "security",
            s.push_protection && s.dependabot_updates && s.deploy_keys.len() == 2 && s.deploy_keys[1].read_only && s.autolinks.len() == 2,
            (s.push_protection, s.dependabot_updates, s.deploy_keys.len(), s.autolinks.len()),
        )
    })
    // This computer: origin over HTTPS, pulls that rebase, a repository-only email, LFS.
    .tap(keys::settings_section("Remotes"))
    .wait_for(setting_item("remote", "origin"), "git@github.com:studio/atlas-web.git")
    .tap(setting("origin-https"))
    .wait_for(setting_item("remote", "origin"), "https://github.com/studio/atlas-web.git")
    .tap(setting("pull-rebase"))
    .wait_for(keys::SETTINGS_STATUS, "Saved in .git/config")
    .mark("remotes")
    .tap(keys::settings_section("Identity"))
    .wait_for(setting_field("user-email"), "ana@example.com")
    .tap(setting_field("user-email"))
    .press(Press::cmd("a"))
    .type_text("ana@atlas.studio")
    .tap(setting("save-identity"))
    .wait_for(keys::SETTINGS_PAGE, "Set for this repository")
    .mark("identity")
    .tap(keys::settings_section("Large files"))
    .wait_for(keys::SETTINGS_PAGE, "Nothing goes to Git LFS yet")
    .tap(setting_field("lfs-pattern"))
    .type_text("*.psd")
    .tap(setting("track"))
    .wait_for(setting_item("lfs", "*.psd"), "*.psd")
    .mark("large-files")
    .world_check("origin is HTTPS, pulls rebase, commits use the studio email, PSDs go to LFS", |w| {
        let attrs = std::fs::read_to_string(atlas(w).join(".gitattributes")).unwrap_or_default();
        expect(
            "this computer",
            config(w, "remote.origin.url") == "https://github.com/studio/atlas-web.git"
                && config(w, "pull.rebase") == "true"
                && gxm_core::git::run(w.env(), &atlas(w), &["config", "--local", "user.email"]).is_ok_and(|e| e.trim() == "ana@atlas.studio")
                && attrs.contains("*.psd filter=lfs diff=lfs merge=lfs -text"),
            (config(w, "remote.origin.url"), config(w, "pull.rebase"), attrs),
        )
    })
    // The danger zone: archive and back, then delete, which needs one more permission.
    .tap(keys::settings_section("Danger zone"))
    .wait_for(setting("archive"), "Archive")
    .tap(setting("archive"))
    .see_in(setting("confirm-archive"), "Archive studio/atlas-web")
    .tap(setting("confirm-archive"))
    .wait_for(setting("archive"), "Unarchive")
    .tap(setting("archive"))
    .tap(setting("confirm-archive"))
    .wait_for(setting("archive"), "Archive")
    .tap(setting_field("confirm-delete"))
    .type_text("studio/atlas-we")
    .see_in(setting("delete-repository"), "Delete studio/atlas-web")
    .type_text("b")
    .mark("danger-zone")
    .tap(setting("delete-repository"))
    .wait_for(keys::SETTINGS_PAGE, "WDJB-MJHT")
    .see_in(keys::SETTINGS_PAGE, "delete_repo")
    .mark("one-more-permission")
    .wait_for(keys::SETTINGS_STATUS, "Deleted studio/atlas-web on GitHub")
    .see_in(keys::SETTINGS_PAGE, "Your copy on this computer stays")
    .mark("deleted")
    .world_check("GitHub deleted it, and the copy on this computer is still there", |w| {
        let s = hub(w);
        expect("deleted", s.deleted && atlas(w).join(".git").exists(), s.deleted)
    })
    .tap(keys::SETTINGS_CLOSE)
    .gone(keys::SETTINGS_DIALOG)
}

/// Kenji, who maintains atlas-web but isn't an admin.
fn write_access_world(w: &World) {
    settings_world(w);
    Hub::of(w).set_viewer(gxm_world::hub::KENJI);
}

pub fn settings_without_admin() -> Journey {
    Journey::new(
        "settings_without_admin",
        "Someone who maintains atlas-web but isn't an admin opens its settings: GitHub's are there to read, what lives on their computer is theirs to change, and a GitLab repository shows what it can.",
        write_access_world,
    )
    // The app opens on Work; this journey is about the history, one click away.
    .tap(keys::VIEW_HISTORY)
    .tap(keys::SETTINGS)
    .wait_for(keys::SETTINGS_NOTICE, "Only admins change studio/atlas-web's settings on GitHub")
    .see_in(keys::SETTINGS_NOTICE, "You have Maintain access")
    .wait_for(setting_field("description"), "The desktop client for Atlas")
    .tap(setting("wiki"))
    .not_see("Saved on GitHub")
    .tap(keys::settings_section("Branches"))
    .wait_for(setting("delete-branches"), "Delete 3 branches")
    .tap(setting("delete-branch-on-merge"))
    .tap(setting("fetch-prune"))
    .wait_for(keys::SETTINGS_STATUS, "Saved in .git/config")
    .mark("read-only")
    .world_check("GitHub's settings are untouched; fetch.prune is on here", |w| {
        let s = hub(w);
        expect("settings", !s.has_wiki && !s.delete_branch_on_merge && config(w, "fetch.prune") == "true", (s.has_wiki, s.delete_branch_on_merge))
    })
    .tap(keys::settings_section("Danger zone"))
    .wait_for(keys::SETTINGS_PAGE, "Only admins")
    .gone(setting("delete-repository"))
    .tap(keys::SETTINGS_CLOSE)
    // A GitLab repository: what lives on this computer, and word of the rest.
    .tap(keys::repo("infra-terraform"))
    .wait_for(keys::BRANCH_SUMMARY, "main")
    .tap(keys::SETTINGS)
    .wait_for(keys::SETTINGS_NOTICE, "GitLab's settings are coming next")
    .tap(keys::settings_section("Remotes"))
    .wait_for(setting_item("remote", "origin"), "git@gitlab.com:studio/infra-terraform.git")
    .mark("gitlab")
}
