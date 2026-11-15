//! `gxm`: git-ex-machina on the command line.
//!
//! The CLI and the app share `gxm-core`, so anything the app shows can be checked here as text.

mod prs;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use gxm_core::env::Env;
use gxm_core::graph::{self, Edge, GraphRow};
use gxm_core::history::{self, Commit, RefKind};
use gxm_core::repo::RepoSummary;
use gxm_core::time;
use gxm_core::workspace::{self, Workspace};

const USAGE: &str = "\
gxm: git-ex-machina on the command line

Usage:
  gxm repos [--json]              The workspace's repositories and their state
  gxm log [PATH] [-n N] [--json]  History with its graph (PATH defaults to .)
  gxm prs [PATH]                  Pull requests on the repository's provider (a token: GH_TOKEN)
  gxm add PATH                    Add a repository on disk to the workspace
  gxm clone URL [INTO]            Clone over SSH into INTO (default ~/code) and add it
  gxm help                        This message
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let env = Env::from_process();
    let mut out = io::stdout().lock();
    let result = match args.first().map(String::as_str) {
        Some("repos") => repos(&env, &args[1..], &mut out),
        Some("log") => log(&env, &args[1..], &mut out),
        Some("prs") => prs::prs(&env, &args[1..], &mut out),
        Some("add") => add(&env, &args[1..], &mut out),
        Some("clone") => clone(&env, &args[1..], &mut out),
        Some("help" | "--help" | "-h") | None => out.write_all(USAGE.as_bytes()).map_err(|e| e.to_string()),
        Some(other) => Err(format!("unknown command {other:?}\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            // Nothing more to do if even stderr is gone.
            writeln!(io::stderr().lock(), "gxm: {message}").ok();
            ExitCode::FAILURE
        }
    }
}

fn repos(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    let json = args.iter().any(|a| a == "--json");
    let ws = Workspace::load(env).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for r in &ws.repos {
        let s = RepoSummary::read(env, &r.path).map_err(|e| format!("{}: {e}", r.path.display()))?;
        rows.push((r.pinned, s));
    }
    if json {
        let list: Vec<_> = rows
            .iter()
            .map(|(pinned, s)| {
                serde_json::json!({
                    "name": s.name,
                    "path": s.path,
                    "pinned": pinned,
                    "owner": s.owner(),
                    "provider": s.origin.as_ref().map(|o| o.provider.name()),
                    "branch": s.status.branch,
                    "upstream": s.status.upstream,
                    "ahead": s.status.ahead,
                    "behind": s.status.behind,
                    "changed": s.status.changed,
                })
            })
            .collect();
        let text = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
        return writeln!(out, "{text}").map_err(|e| e.to_string());
    }
    for (pinned, s) in &rows {
        let mut state = Vec::new();
        if s.status.ahead > 0 {
            state.push(format!("↑{}", s.status.ahead));
        }
        if s.status.behind > 0 {
            state.push(format!("↓{}", s.status.behind));
        }
        if s.status.changed > 0 {
            state.push(format!("{} changed", s.status.changed));
        }
        let pin = if *pinned { "★" } else { " " };
        let branch = s.status.branch.as_deref().unwrap_or("(detached)");
        let provider = s.origin.as_ref().map_or("local", |o| o.provider.name());
        writeln!(out, "{pin} {:<18} {:<16} {:<10} {:<10} {}", s.name, branch, s.owner(), provider, state.join(" "))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn add(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    let [path] = args else { return Err(format!("add takes one path\n\n{USAGE}")) };
    let top = workspace::add_repo(env, &env.expand(path)).map_err(|e| e.to_string())?;
    writeln!(out, "Added {}", env.tilde(&top)).map_err(|e| e.to_string())
}

fn clone(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    let (typed, into) = match args {
        [typed] => (typed, env.home.join("code")),
        [typed, into] => (typed, env.expand(into)),
        _ => return Err(format!("clone takes a URL and, optionally, where to clone it\n\n{USAGE}")),
    };
    let source = gxm_core::clone::source(typed).ok_or_else(|| format!("{typed:?} isn't a repository URL"))?;
    let dest = gxm_core::clone::destination(&into, &source);
    writeln!(out, "Cloning {} into {}", source.url, env.tilde(&dest)).map_err(|e| e.to_string())?;
    gxm_core::clone::run(env, &source.url, &dest, &mut |_| {}, &|| false).map_err(|e| e.to_string())?;
    workspace::add_repo(env, &dest).map_err(|e| e.to_string())?;
    writeln!(out, "Added {}", env.tilde(&dest)).map_err(|e| e.to_string())
}

fn log(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    let mut path = PathBuf::from(".");
    let mut limit = 50;
    let mut json = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => json = true,
            "-n" => limit = it.next().and_then(|n| n.parse().ok()).ok_or("-n takes a number")?,
            p => path = PathBuf::from(p),
        }
    }
    let commits = history::log(env, &path, limit).map_err(|e| e.to_string())?;
    let rows = graph::layout(commits.iter().map(|c| (c.id.as_str(), c.parents.iter().map(String::as_str))));
    if json {
        let list: Vec<_> = commits
            .iter()
            .zip(&rows)
            .map(|(c, g)| {
                serde_json::json!({
                    "id": c.id,
                    "parents": c.parents,
                    "author": c.author,
                    "time": c.time.to_string(),
                    "subject": c.subject,
                    "refs": c.refs.iter().map(|r| &r.name).collect::<Vec<_>>(),
                    "lane": g.lane,
                    "color": g.color,
                })
            })
            .collect();
        let text = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
        return writeln!(out, "{text}").map_err(|e| e.to_string());
    }
    let width = rows.iter().map(|r| r.width).max().unwrap_or(1);
    for (c, g) in commits.iter().zip(&rows) {
        let date = time::relative(c.time, env.now(), &env.tz);
        writeln!(
            out,
            "{}  {}  {}{}  · {} · {}",
            graph_cells(g, width, c),
            c.short_id(),
            refs(c),
            c.subject,
            c.author,
            date
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn refs(c: &Commit) -> String {
    let names: Vec<String> = c
        .refs
        .iter()
        .map(|r| match r.kind {
            RefKind::Head => format!("HEAD → {}", r.name),
            RefKind::Tag => format!("tag {}", r.name),
            RefKind::LocalBranch | RefKind::RemoteBranch => r.name.clone(),
        })
        .collect();
    if names.is_empty() { String::new() } else { format!("({}) ", names.join(", ")) }
}

/// One row of the graph as text: `●` the commit, `│` lanes passing, and corners where lanes
/// join or leave the commit (`├` `┤` where a join meets a passing lane, `┼` where a line crosses one).
fn graph_cells(g: &GraphRow, width: usize, c: &Commit) -> String {
    let mut cells = vec![' '; width * 2];
    for e in &g.edges {
        if let Edge::Pass { lane, .. } = *e {
            cells[lane * 2] = '│';
        }
    }
    let corner = |at: usize, right_of_node: bool, from_top: bool, cells: &mut Vec<char>| {
        let x = at * 2;
        cells[x] = match (cells[x] == '│', right_of_node, from_top) {
            (true, true, _) => '┤',
            (true, false, _) => '├',
            (false, true, true) => '╯',
            (false, true, false) => '╮',
            (false, false, true) => '╰',
            (false, false, false) => '╭',
        };
        let lo = at.min(g.lane) * 2 + 1;
        let hi = at.max(g.lane) * 2;
        for cell in &mut cells[lo..hi] {
            *cell = match *cell {
                ' ' => '─',
                '│' => '┼',
                other => other,
            };
        }
    };
    for e in &g.edges {
        match *e {
            Edge::In { from, .. } if from != g.lane => corner(from, from > g.lane, true, &mut cells),
            Edge::Out { to, .. } if to != g.lane => corner(to, to > g.lane, false, &mut cells),
            Edge::Pass { .. } | Edge::In { .. } | Edge::Out { .. } => {}
        }
    }
    cells[g.lane * 2] = if c.refs.iter().any(|r| r.kind == RefKind::Head) { '◉' } else { '●' };
    cells.into_iter().collect()
}
