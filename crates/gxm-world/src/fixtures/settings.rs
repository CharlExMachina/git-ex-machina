//! atlas-web's settings on the world's GitHub, as a team that has run it for a while leaves
//! them: studio's people and a team, rules on main, release branches and tags, secrets and a
//! variable for its workflows, a production environment, a CI webhook, a Pages site, a deploy
//! key and an autolink to its tracker.

use serde_json::json;

use super::t;
use crate::World;
use crate::hub::admin::{
    AutolinkData, DeployKeyData, EnvironmentData, HookData, InvitationData, PagesData, SecretData, VariableData,
};

pub fn atlas_settings(w: &World) {
    w.hub().edit_settings("studio", "atlas-web", |s| {
        s.owner_is_org = true;
        s.description = "The desktop client for Atlas: history, the commit graph and pull request reviews.".to_owned();
        s.homepage = "https://atlas.studio".to_owned();
        s.topics = ["rust", "git", "desktop-app"].map(str::to_owned).to_vec();
        s.has_wiki = false;
        s.allow_rebase_merge = false;
        s.allow_auto_merge = true;
        s.allow_update_branch = true;
        s.squash_merge_commit_title = "PR_TITLE".to_owned();
        s.squash_merge_commit_message = "BLANK".to_owned();
        s.collaborators = [("ana-ruiz", "admin"), ("kenji-mori", "maintain"), ("lea-brandt", "write")]
            .map(|(l, r)| (l.to_owned(), r.to_owned()))
            .to_vec();
        s.invitations = vec![InvitationData {
            id: 1001,
            login: "jo-park".to_owned(),
            permission: "write".to_owned(),
            created: t("2026-11-09T10:00:00Z"),
        }];
        s.teams = vec![("design".to_owned(), "studio/design".to_owned(), "triage".to_owned())];
        s.base_role = Some("read".to_owned());
        let admins = json!([{ "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" }]);
        s.rulesets = vec![
            json!({
                "id": 1101, "name": "Protect main", "target": "branch", "enforcement": "active",
                "conditions": { "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] } },
                "rules": [
                    { "type": "pull_request", "parameters": { "required_approving_review_count": 1 } },
                    { "type": "required_status_checks", "parameters": { "required_status_checks": [{ "context": "build" }, { "context": "test" }] } },
                    { "type": "non_fast_forward" },
                    { "type": "deletion" },
                ],
                "bypass_actors": admins,
            }),
            json!({
                "id": 1102, "name": "Release branches", "target": "branch", "enforcement": "active",
                "conditions": { "ref_name": { "include": ["refs/heads/release/*"], "exclude": [] } },
                "rules": [
                    { "type": "pull_request", "parameters": { "required_approving_review_count": 2 } },
                    { "type": "non_fast_forward" },
                    { "type": "deletion" },
                ],
                "bypass_actors": [],
            }),
            json!({
                "id": 1103, "name": "Version tags", "target": "tag", "enforcement": "evaluate",
                "conditions": { "ref_name": { "include": ["refs/tags/v*"], "exclude": [] } },
                "rules": [{ "type": "deletion" }, { "type": "non_fast_forward" }],
                "bypass_actors": [],
            }),
        ];
        let secret = |name: &str, at: &str| SecretData { name: name.to_owned(), value: "fixture".to_owned(), updated: t(at) };
        s.secrets = vec![
            ("actions".to_owned(), vec![secret("NPM_TOKEN", "2026-11-08T09:00:00Z"), secret("SENTRY_AUTH_TOKEN", "2026-11-02T16:00:00Z")]),
            ("dependabot".to_owned(), vec![secret("NPM_TOKEN", "2026-10-20T09:00:00Z")]),
        ];
        s.variables = vec![VariableData { name: "NODE_VERSION".to_owned(), value: "22".to_owned(), updated: t("2026-11-09T12:00:00Z") }];
        s.environments = vec![EnvironmentData {
            name: "production".to_owned(),
            wait_timer: 10,
            reviewers: vec!["ana-ruiz".to_owned()],
            protected_branches: true,
        }];
        s.hooks = vec![HookData {
            id: 1201,
            url: "https://ci.atlas.studio/github".to_owned(),
            events: vec!["push".to_owned(), "pull_request".to_owned()],
            active: true,
            secret: "fixture".to_owned(),
            last: Some((200, "OK".to_owned())),
        }];
        s.pages = Some(PagesData {
            build_type: "legacy".to_owned(),
            branch: "gh-pages".to_owned(),
            path: "/".to_owned(),
            cname: None,
            https_enforced: true,
        });
        s.dependabot_alerts = true;
        s.secret_scanning = true;
        s.deploy_keys = vec![DeployKeyData {
            id: 1301,
            title: "Release bot".to_owned(),
            key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl release-bot".to_owned(),
            read_only: false,
            created: t("2026-09-14T11:00:00Z"),
        }];
        s.autolinks = vec![AutolinkData {
            id: 1401,
            prefix: "ATL-".to_owned(),
            template: "https://tracker.atlas.studio/browse/ATL-<num>".to_owned(),
        }];
    });
}

/// atlas-web's branches, every kind tidying up meets, pushed where they belong:
///
/// - `feature/graph-virtualization`, here and on origin: #128 merged it.
/// - `tune/brass-light`, on origin only: its commit is in main.
/// - `sidebar-spring`, here only: a cherry-pick of a commit main has (same changes, new id).
/// - `fix/avatar-cache`, here and on origin: #131 is still open.
/// - `spike/canvas-text`, on origin only: a commit main doesn't have.
/// - `wip/lane-colors`, here only: two commits that are nowhere else.
pub fn atlas_branches(w: &World, atlas: &std::path::Path) {
    use super::{KENJI, LEA};
    let r = w.open(atlas);
    r.git(&["push", "--quiet", "origin", "feature/graph-virtualization", "fix/avatar-cache"]);
    let brass = r.git(&["log", "--format=%H", "--grep=^Tune brass for the light theme", "-1", "origin/main"]);
    r.git(&["push", "--quiet", "origin", &format!("{}:refs/heads/tune/brass-light", brass.trim())]);
    // Branches made in a scratch worktree: atlas-web's own has uncommitted work.
    let scratch = w.root().join("atlas-scratch");
    let branch = |name: &str, from: &str, make: &dyn Fn(&crate::RepoBuilder<'_>)| {
        r.git(&["worktree", "add", "--quiet", "-b", name, &scratch.display().to_string(), from]);
        make(&w.open(&scratch));
        r.git(&["worktree", "remove", "--force", &scratch.display().to_string()]);
    };
    branch("sidebar-spring", "origin/main~4", &|s| {
        s.git(&["cherry-pick", "--allow-empty", "origin/main~3"]);
    });
    branch("spike/canvas-text", "origin/main", &|s| {
        s.commit(LEA, t("2026-11-10T09:30:00Z"), "Try canvas text rendering", &["src/text/canvas.rs"]);
        s.git(&["push", "--quiet", "origin", "spike/canvas-text"]);
    });
    r.git(&["branch", "-D", "spike/canvas-text"]);
    branch("wip/lane-colors", "origin/main", &|s| {
        s.commit(KENJI, t("2026-11-10T14:00:00Z"), "Lane colours from the theme", &["src/graph/lanes.rs"]);
        s.commit(KENJI, t("2026-11-11T09:10:00Z"), "Contrast for the sixth lane", &["src/graph/lanes.rs"]);
    });
    r.fetched(w.ago(jiff::SignedDuration::from_mins(2)));
}
