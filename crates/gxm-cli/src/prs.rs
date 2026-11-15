//! `gxm prs [PATH]`: a repository's pull requests on its provider, and how many of their
//! commits the history has: what the app's inspector and graph show, as text.

use std::io::Write;
use std::path::PathBuf;

use gxm_core::env::Env;
use gxm_core::forge::{self, local};
use gxm_core::repo::RepoSummary;

pub fn prs(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    // A token from the environment (`GH_TOKEN`) comes first; else the sign-in's.
    let mut env = env.clone();
    if env.forge.github_token.is_none() {
        env.forge.github_token = gxm_core::auth::token(&env).map_err(|e| e.to_string())?;
    }
    let env = &env;
    let path = args.first().map_or_else(|| PathBuf::from("."), PathBuf::from);
    let summary = RepoSummary::read(env, &path).map_err(|e| e.to_string())?;
    let remote = summary.origin.ok_or("this repository has no origin")?;
    let forge = forge::connect(env, &remote).map_err(|why| why.message())?;
    let name = forge.provider().name();
    let pulls = forge.pulls().map_err(|e| format!("{name}: {e}"))?;
    let by_commit = local::commit_pulls(env, &path, &pulls);
    for p in &pulls {
        let here = by_commit.values().filter(|n| **n == p.number).count();
        let commits = if here == 1 { "1 commit here".to_owned() } else { format!("{here} commits here") };
        writeln!(
            out,
            "#{:<5} {:<7} {} → {}  {}  · {} · {commits}",
            p.number,
            p.state.word(),
            p.head_ref,
            p.base_ref,
            p.title,
            p.author.name
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
