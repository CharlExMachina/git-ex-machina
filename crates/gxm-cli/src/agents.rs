//! `gxm agents`: the agent harnesses on this computer, and running a prompt with one, its session
//! streamed line by line.

use std::io::Write;
use std::time::Duration;

use gxm_core::env::Env;
use gxm_core::harness::{self, Auth, Ending, Event, Harness, RuntimeMode, Start};

pub const USAGE: &str = "\
Usage:
  gxm agents [--json]             The agent harnesses on this computer, and which is preferred
  gxm agents prefer NAME          Use NAME (claude-code, codex) from now on
  gxm agents run [--in DIR] [--agent NAME] [--full-access] PROMPT
                                  Run PROMPT in DIR (default .) and stream the session
";

pub fn agents(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None | Some("--json") => list(env, !args.is_empty(), out),
        Some("prefer") => {
            let [_, name] = args else { return Err(format!("prefer takes one name\n\n{USAGE}")) };
            let h = named(name)?;
            harness::prefer(env, h)?;
            writeln!(out, "{} is preferred", h.name()).map_err(|e| e.to_string())
        }
        Some("run") => run(env, &args[1..], out),
        Some(other) => Err(format!("unknown agents command {other:?}\n\n{USAGE}")),
    }
}

fn named(name: &str) -> Result<Harness, String> {
    Harness::from_id(name).ok_or_else(|| format!("no agent harness named {name:?} (claude-code, codex)"))
}

fn list(env: &Env, json: bool, out: &mut impl Write) -> Result<(), String> {
    let statuses = harness::available(env);
    let preferred = harness::pick(harness::choice(env), &statuses);
    let w = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    if json {
        let list: Vec<_> = statuses
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.harness.id(),
                    "name": s.harness.name(),
                    "path": s.path,
                    "version": s.version,
                    "signedIn": matches!(s.auth, Auth::SignedIn(_)),
                    "account": if let Auth::SignedIn(who) = &s.auth { who.clone() } else { None },
                    "usable": s.usable(),
                    "preferred": preferred == Some(s.harness),
                    "problem": s.problem,
                })
            })
            .collect();
        return w(out, serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?);
    }
    for s in &statuses {
        let mark = if preferred == Some(s.harness) { "★" } else { " " };
        let state = match (&s.auth, s.usable()) {
            (Auth::SignedIn(Some(who)), true) => format!("ready, signed in as {who}"),
            (_, true) => "ready".to_owned(),
            (_, false) if s.installed() => "not usable".to_owned(),
            _ => "not installed".to_owned(),
        };
        let version = s.version.as_deref().unwrap_or("-");
        w(out, format!("{mark} {:<12} {:<12} {:<10} {state}", s.harness.id(), s.harness.name(), version))?;
        if let Some(why) = &s.problem {
            w(out, format!("    {why}"))?;
        }
    }
    if statuses.iter().all(|s| !s.installed()) {
        w(
            out,
            "No coding agent found. Install Claude Code or Codex, sign in with its CLI, then run gxm agents again."
                .to_owned(),
        )?;
    }
    Ok(())
}

fn run(env: &Env, args: &[String], out: &mut impl Write) -> Result<(), String> {
    let (mut dir, mut agent, mut mode, mut prompt) = (None, None, RuntimeMode::ApprovalRequired, Vec::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--in" => dir = Some(it.next().ok_or("--in takes a folder")?.clone()),
            "--agent" => agent = Some(named(it.next().ok_or("--agent takes a name")?)?),
            "--full-access" => mode = RuntimeMode::FullAccess,
            _ => prompt.push(a.clone()),
        }
    }
    let prompt = prompt.join(" ");
    if prompt.trim().is_empty() {
        return Err(format!("run takes a prompt\n\n{USAGE}"));
    }
    let dir = match dir {
        Some(d) => env.expand(&d),
        None => std::env::current_dir().map_err(|e| e.to_string())?,
    };
    let h = match agent {
        Some(h) => h,
        None => harness::preferred(env).ok_or("no agent harness is ready: gxm agents says why")?,
    };
    let session = harness::start(env, h, &Start { dir, prompt, mode }, Box::new(|| {}))?;
    loop {
        let Some(e) = session.next_event(Duration::from_secs(3600)) else {
            return Err("the agent stopped answering".to_owned());
        };
        writeln!(out, "{}", e.line()).and_then(|()| out.flush()).map_err(|e| e.to_string())?;
        match e {
            Event::Ended(Ending::Completed) => return Ok(()),
            // The last line said how; the exit code says it didn't finish.
            Event::Ended(_) => return Err(format!("{} didn't finish", h.name())),
            _ => {}
        }
    }
}
