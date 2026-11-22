//! The settings client against the world's GitHub: every page read, every kind of change
//! written, and what reached "GitHub" checked on its side.

use gxm_core::forge::ForgeError;
use gxm_core::forge::admin::{self, *};
use gxm_core::remote::Remote;
use gxm_world::World;
use gxm_world::hub::{Hub, TOKEN, TOKEN_DELETE};

fn connect(w: &World, token: &str) -> std::sync::Arc<dyn Admin> {
    let mut env = w.env().clone();
    env.forge.github_token = Some(token.to_owned());
    let remote = Remote::parse("git@github.com:studio/atlas-web.git").unwrap_or_else(|| panic!("the remote parses"));
    admin::connect(&env, &remote).unwrap_or_else(|e| panic!("{}", e.message()))
}

fn read<T>(a: &dyn Admin, page: Page, pick: impl FnOnce(Reading) -> Option<T>) -> T {
    pick(a.read(page).unwrap_or_else(|e| panic!("read {page:?}: {e}")))
        .unwrap_or_else(|| panic!("{page:?} read as another page"))
}

#[test]
fn an_admin_reads_and_changes_every_setting_from_the_client() {
    let w = World::new();
    gxm_world::fixtures::atlas_web(&w);
    gxm_world::fixtures::atlas_settings(&w);
    let a = connect(&w, TOKEN);
    let hub = || Hub::of(&w).settings("studio", "atlas-web").unwrap();

    // The repository as its settings pages show it.
    let repo = read(&*a, Page::Repo, |r| match r {
        Reading::Repo(r) => Some(*r),
        _ => None,
    });
    assert_eq!((repo.role, repo.owner_is_org, repo.default_branch.as_str()), (Role::Admin, true, "main"));
    assert_eq!(repo.topics, ["rust", "git", "desktop-app"]);
    assert_eq!((repo.squash_message, repo.rebase, repo.auto_merge), (Message::Title, false, true));

    // Changes to the repository itself land field by field.
    for c in [
        Change::Description("History, graph and reviews.".to_owned()),
        Change::Wiki(true),
        Change::Rebase(true),
        Change::SquashMessage(Message::TitleAndDescription),
        Change::DeleteBranchOnMerge(true),
        Change::Visibility(Visibility::Public),
    ] {
        a.write(&Write::Change(c)).unwrap();
    }
    a.write(&Write::Topics(vec!["rust".to_owned(), "graph".to_owned()])).unwrap();
    let s = hub();
    assert_eq!(s.description, "History, graph and reviews.");
    assert!(s.has_wiki && s.allow_rebase_merge && s.delete_branch_on_merge);
    assert_eq!((s.squash_merge_commit_title.as_str(), s.squash_merge_commit_message.as_str()), ("PR_TITLE", "PR_BODY"));
    assert_eq!((s.visibility.as_str(), s.topics.join(",")), ("public", "rust,graph".to_owned()));

    // People: invite, change a role, remove, cancel an invitation, a team's role.
    let people = read(&*a, Page::People, |r| match r {
        Reading::People(p) => Some(p),
        _ => None,
    });
    assert_eq!(people.collaborators.len(), 3);
    assert_eq!((people.invitations[0].login.as_str(), people.base), ("jo-park", Some(Role::Read)));
    assert_eq!(people.teams[0].role, Role::Triage);
    a.write(&Write::Invite { login: "sam-lee".to_owned(), role: Role::Write }).unwrap();
    a.write(&Write::SetRole { who: Grantee::Person("lea-brandt".to_owned()), role: Role::Maintain }).unwrap();
    a.write(&Write::Remove(Grantee::Person("kenji-mori".to_owned()))).unwrap();
    a.write(&Write::CancelInvite(people.invitations[0].id)).unwrap();
    a.write(&Write::SetRole { who: Grantee::Team("design".to_owned()), role: Role::Write }).unwrap();
    let s = hub();
    assert_eq!(s.role_of("lea-brandt"), "maintain");
    assert_eq!(s.role_of("kenji-mori"), "read");
    assert_eq!(s.invitations.iter().map(|i| i.login.as_str()).collect::<Vec<_>>(), ["sam-lee"]);
    assert_eq!(s.teams[0].2, "push");

    // Rule sets: read in full, changed, created and deleted.
    let mut rules = read(&*a, Page::Rules, |r| match r {
        Reading::Rules(r) => Some(r),
        _ => None,
    });
    assert_eq!(
        rules.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["Protect main", "Release branches", "Version tags"]
    );
    assert_eq!(rules[0].include, [DEFAULT_BRANCH]);
    assert_eq!(rules[0].summary(), ["Pull request (1 approval)", "2 checks", "Block force-pushes", "Block deleting"]);
    assert_eq!((rules[2].target, rules[2].enforcement), (Target::Tag, Enforcement::Evaluate));
    rules[0].rules.pull_request = Some(2);
    rules[0].rules.signed_commits = true;
    a.write(&Write::SaveRuleset(Box::new(rules[0].clone()))).unwrap();
    a.write(&Write::SaveRuleset(Box::new(Ruleset::new_for_default_branch()))).unwrap();
    a.write(&Write::DeleteRuleset(rules[2].id)).unwrap();
    let after = read(&*a, Page::Rules, |r| match r {
        Reading::Rules(r) => Some(r),
        _ => None,
    });
    assert_eq!(after[0].rules.pull_request, Some(2));
    assert!(after[0].rules.signed_commits);
    assert_eq!(
        after.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["Protect main", "Release branches", "Protect the default branch"]
    );

    // Actions, and a secret sealed here that only the repository's key opens.
    a.write(&Write::Actions(Actions { enabled: true, allowed: Allowed::Selected, write: true, approve_reviews: true }))
        .unwrap();
    a.write(&Write::PutSecret { scope: Scope::Actions, name: "CODECOV_TOKEN".to_owned(), value: "cc-123".to_owned() })
        .unwrap();
    a.write(&Write::DeleteSecret { scope: Scope::Actions, name: "SENTRY_AUTH_TOKEN".to_owned() }).unwrap();
    a.write(&Write::PutVariable { name: "RELEASE_CHANNEL".to_owned(), value: "beta".to_owned(), new: true }).unwrap();
    a.write(&Write::PutVariable { name: "NODE_VERSION".to_owned(), value: "24".to_owned(), new: false }).unwrap();
    let s = hub();
    assert_eq!(
        (s.allowed_actions.as_str(), s.workflow_permissions.as_str(), s.approve_reviews),
        ("selected", "write", true)
    );
    assert_eq!(s.secret("actions", "CODECOV_TOKEN"), Some("cc-123"));
    assert_eq!(s.secret("actions", "SENTRY_AUTH_TOKEN"), None);
    let vars: Vec<(String, String)> = s.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect();
    assert_eq!(vars, [("NODE_VERSION".to_owned(), "24".to_owned()), ("RELEASE_CHANNEL".to_owned(), "beta".to_owned())]);
    let secrets = read(&*a, Page::Secrets(Scope::Dependabot), |r| match r {
        Reading::Secrets(Scope::Dependabot, s) => Some(s),
        _ => None,
    });
    assert_eq!(secrets[0].name, "NPM_TOKEN");

    // An environment with a reviewer, a webhook pinged, Pages from docs/ on a custom domain.
    let envs = read(&*a, Page::Environments, |r| match r {
        Reading::Environments(e) => Some(e),
        _ => None,
    });
    assert_eq!(
        (envs[0].wait_minutes, envs[0].reviewers.join(","), envs[0].protected_only),
        (10, "ana-ruiz".to_owned(), true)
    );
    let staging = Environment {
        name: "staging".to_owned(),
        wait_minutes: 5,
        reviewers: vec!["lea-brandt".to_owned()],
        protected_only: false,
    };
    a.write(&Write::SaveEnvironment(staging)).unwrap();
    a.write(&Write::AddHook(NewHook {
        url: "https://chat.example.com/hook".to_owned(),
        secret: "s".to_owned(),
        events: vec!["release".to_owned()],
    }))
    .unwrap();
    let hooks = read(&*a, Page::Hooks, |r| match r {
        Reading::Hooks(h) => Some(h),
        _ => None,
    });
    assert_eq!(hooks[0].last.as_deref(), Some("200 OK"));
    assert_eq!(hooks[1].last, None);
    a.write(&Write::PingHook(hooks[1].id)).unwrap();
    a.write(&Write::HookActive { id: hooks[0].id, active: false }).unwrap();
    a.write(&Write::PagesSource(Some(PagesSource::Branch { branch: "main".to_owned(), docs: true }))).unwrap();
    a.write(&Write::PagesDomain { cname: Some("docs.atlas.studio".to_owned()), https: true }).unwrap();
    let s = hub();
    assert_eq!(s.environments[1].reviewers, ["lea-brandt"]);
    assert_eq!((s.hooks[0].active, s.hooks[1].last.clone()), (false, Some((200, "OK".to_owned()))));
    let p = s.pages.unwrap();
    assert_eq!((p.branch.as_str(), p.path.as_str(), p.cname.as_deref()), ("main", "/docs", Some("docs.atlas.studio")));
    let pages = read(&*a, Page::Pages, |r| match r {
        Reading::Pages(p) => p,
        _ => None,
    });
    assert_eq!(pages.url, "https://docs.atlas.studio/");

    // Security features, a deploy key and an autolink.
    a.write(&Write::Security(SecurityFeature::PushProtection, true)).unwrap();
    a.write(&Write::Security(SecurityFeature::DependabotAlerts, false)).unwrap();
    let security = read(&*a, Page::Security, |r| match r {
        Reading::Security(s) => Some(s),
        _ => None,
    });
    assert_eq!((security.push_protection, security.secret_scanning, security.dependabot_alerts), (true, true, false));
    let key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBd5yHtM2Vz2bO9U0ubbfNsu9xS7lHvbHQi1vMuJxKqA ci";
    a.write(&Write::AddDeployKey { title: "CI".to_owned(), key: key.to_owned(), read_only: true }).unwrap();
    let refused = a.write(&Write::AddDeployKey { title: "x".to_owned(), key: "not a key".to_owned(), read_only: true });
    assert!(
        matches!(refused, Err(ForgeError::Api(m)) if m.contains("OpenSSH public key format")),
        "GitHub's reason comes through"
    );
    a.write(&Write::AddAutolink { prefix: "TICKET-".to_owned(), template: "https://t.example.com/<num>".to_owned() })
        .unwrap();
    let s = hub();
    assert_eq!(s.deploy_keys.iter().map(|k| k.title.as_str()).collect::<Vec<_>>(), ["Release bot", "CI"]);
    assert_eq!(s.autolinks[1].prefix, "TICKET-");

    // Renaming keeps the repository reachable by its old name, as GitHub redirects.
    a.write(&Write::Change(Change::Name("atlas-desktop".to_owned()))).unwrap();
    assert!(Hub::of(&w).settings("studio", "atlas-desktop").is_some());
    let renamed = read(&*a, Page::Repo, |r| match r {
        Reading::Repo(r) => Some(*r),
        _ => None,
    });
    assert_eq!(renamed.name, "atlas-desktop");

    // Deleting needs a token granted delete_repo; the sign-in's ordinary token is refused.
    assert_eq!(a.write(&Write::Delete), Err(ForgeError::Status(403)));
    connect(&w, TOKEN_DELETE).write(&Write::Delete).unwrap();
    assert!(Hub::of(&w).settings("studio", "atlas-desktop").is_some_and(|s| s.deleted));
}

#[test]
fn someone_without_admin_rights_reads_the_settings_but_changes_nothing() {
    let w = World::new();
    gxm_world::fixtures::atlas_web(&w);
    gxm_world::fixtures::atlas_settings(&w);
    Hub::of(&w).set_viewer(gxm_world::hub::KENJI);
    let a = connect(&w, TOKEN);
    let repo = read(&*a, Page::Repo, |r| match r {
        Reading::Repo(r) => Some(*r),
        _ => None,
    });
    assert_eq!(repo.role, Role::Maintain);
    assert_eq!(a.write(&Write::Change(Change::Wiki(true))), Err(ForgeError::Status(403)));
    assert!(!Hub::of(&w).settings("studio", "atlas-web").unwrap().has_wiki);
}
