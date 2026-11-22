//! `gxm settings [PATH]` and `gxm tidy [PATH] [--delete]`: a repository's settings, on its
//! provider and on this computer, and its merged branches: what the app's settings show, as text.

use std::io::Write;
use std::path::PathBuf;

use gxm_core::env::Env;
use gxm_core::forge::admin::{self, Page, Reading};
use gxm_core::forge::{self, model::PullSummary};
use gxm_core::repo::RepoSummary;
use gxm_core::{local, tidy};

/// The environment with a token: the process's (`GH_TOKEN`) first, else the sign-in's.
fn with_token(env: &Env) -> Result<Env, String> {
    let mut env = env.clone();
    if env.forge.github_token.is_none() {
        env.forge.github_token = gxm_core::auth::token(&env).map_err(|e| e.to_string())?;
    }
    Ok(env)
}

fn w(out: &mut impl Write, line: impl std::fmt::Display) -> Result<(), String> {
    writeln!(out, "{line}").map_err(|e| e.to_string())
}

pub fn settings(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    let env = &with_token(env)?;
    let path = args.first().map_or_else(|| PathBuf::from("."), PathBuf::from);
    let summary = RepoSummary::read(env, &path).map_err(|e| e.to_string())?;
    let yes = |b: bool| if b { "on" } else { "off" };
    match summary.origin.as_ref().map(|r| admin::connect(env, r)) {
        Some(Ok(a)) => {
            let Reading::Repo(r) = a.read(Page::Repo).map_err(|e| e.to_string())? else {
                return Err("no repository".into());
            };
            w(
                out,
                format!("{}  ({}, {}; you: {})", a.full_name(), r.visibility.word(), r.default_branch, r.role.word()),
            )?;
            if !r.description.is_empty() {
                w(out, format!("  {}", r.description))?;
            }
            w(
                out,
                format!(
                    "  issues {} · projects {} · wiki {} · discussions {}",
                    yes(r.issues),
                    yes(r.projects),
                    yes(r.wiki),
                    yes(r.discussions)
                ),
            )?;
            let methods: Vec<&str> = [(r.merge_commit, "merge"), (r.squash, "squash"), (r.rebase, "rebase")]
                .iter()
                .filter(|m| m.0)
                .map(|m| m.1)
                .collect();
            w(
                out,
                format!(
                    "  merges by {} · auto-merge {} · delete branches on merge {}",
                    methods.join(", "),
                    yes(r.auto_merge),
                    yes(r.delete_branch_on_merge)
                ),
            )?;
        }
        Some(Err(why)) => w(out, why.message())?,
        None => w(out, "No remote on GitHub, GitLab or Bitbucket.".to_owned())?,
    }
    let l = local::read(env, &path).map_err(|e| e.to_string())?;
    w(out, "On this computer".to_owned())?;
    w(out, format!("  commits as {} <{}>", l.identity.name, l.identity.email))?;
    w(
        out,
        format!(
            "  fetch prunes {} · pull: {} · delete merged branches {}",
            yes(l.fetch_prune),
            l.pull.word(),
            yes(l.delete_merged)
        ),
    )?;
    for r in local::remotes(env, &path).map_err(|e| e.to_string())? {
        w(out, format!("  {} {}{}", r.name, r.url, if r.fetch_only { " (fetch only)" } else { "" }))?;
    }
    Ok(())
}

pub fn tidy(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    let env = &with_token(env)?;
    let delete = args.iter().any(|a| a == "--delete");
    let path = args.iter().find(|a| !a.starts_with("--")).map_or_else(|| PathBuf::from("."), PathBuf::from);
    let summary = RepoSummary::read(env, &path).map_err(|e| e.to_string())?;
    // The provider knows the default branch for sure, and which pull requests merged.
    let (mut default, mut pulls): (String, Vec<PullSummary>) = (tidy::default_branch(env, &path), Vec::new());
    if let Some(remote) = &summary.origin {
        if let Ok(a) = admin::connect(env, remote)
            && let Ok(Reading::Repo(r)) = a.read(Page::Repo)
        {
            default = r.default_branch;
        }
        if let Ok(f) = forge::connect(env, remote) {
            pulls = f.pulls().unwrap_or_default();
        }
    }
    let branches = tidy::survey(env, &path, &default, &pulls).map_err(|e| e.to_string())?;
    for b in &branches {
        let mark = if b.done() { '✓' } else { '·' };
        w(out, format!("{mark} {:<32} {:<17} {}", b.name, b.place(), b.reason()))?;
    }
    if delete {
        let deleted = tidy::delete(env, &path, &branches).map_err(|e| e.to_string())?;
        w(out, deleted.sentence())?;
    }
    Ok(())
}
