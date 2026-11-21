//! `cargo xtask`: the project's automation. Every check the CI runs is one of these, so local and
//! CI never disagree. Run `cargo xtask help`.

#![expect(clippy::print_stdout, clippy::print_stderr, reason = "xtask is a terminal tool")]

mod journey;
mod pr;
mod verify;
mod visual;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use gxm_world::World;

const USAGE: &str = "\
cargo xtask <command>

Checks
  verify [--fast]           Everything CI runs: fmt, clippy, garden checks, tests (--fast: no clippy on tests, no deny)
  bless                     Re-run tests, rewriting golden files. Show every changed golden in the PR.

Worlds (hermetic fixture workspaces in target/worlds/)
  world [NAME]              Build a world (default: mockup) and print how to use it
  gxm [NAME] -- ARGS        Run the gxm CLI inside a world
  app [NAME] -- ARGS        Run the app binary inside a world (any Rune flag: --snap, --tree, --script, --video)

Seeing it
  snap [NAME] [--theme dark|light] [--size WxH]   Render the app headless to target/proof/snap-NAME-THEME.png (+ tree JSON)
  journey NAME [--video]    Run a journey headless, then render its marks (and a keycapped video) to target/proof/NAME/
  journeys                  List the journeys

Pull requests
  pr open BODY.md [--base BRANCH] [--title TITLE]   Check the body's proof, then create or update this branch's PR with every file it references attached
  pr check BODY.md [--base REF]                     Check a PR body carries proof (what CI runs on every PR)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
    let result = match args.first().map(String::as_str) {
        Some("verify") => verify::verify(rest.iter().any(|a| a == "--fast")),
        Some("bless") => verify::bless(),
        Some("world") => world_cmd(rest),
        Some("gxm") => in_world("gxm", rest),
        Some("app") => in_world("app", rest),
        Some("snap") => visual::snap(rest),
        Some("journey") => journey::journey(rest),
        Some("journeys") => journey::list(),
        Some("pr") => pr::pr(rest),
        Some("help" | "--help" | "-h") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command {other:?}\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The workspace root.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default()
}

pub fn target() -> PathBuf {
    root().join("target")
}

pub fn proof_dir() -> PathBuf {
    target().join("proof")
}

/// Builds a named world in `target/worlds/NAME`.
pub fn build_world(name: &str) -> Result<World, String> {
    build_world_in(name, &target().join("worlds").join(name))
}

/// World `name`, built in `dir`: replays that run side by side each need their own.
pub fn build_world_in(name: &str, dir: &std::path::Path) -> Result<World, String> {
    let w = World::create_in(dir);
    match name {
        "mockup" => {
            gxm_world::fixtures::mockup(&w);
        }
        "empty" => {}
        other => {
            let j = gxm_journeys::catalog::find(other).ok_or_else(|| format!("no world or journey named {other:?}"))?;
            (j.world)(&w);
        }
    }
    Ok(w)
}

fn world_cmd(args: &[String]) -> Result<(), String> {
    let name = args.first().map_or("mockup", String::as_str);
    let w = build_world(name)?;
    println!("World {name:?} at {}", w.root().display());
    println!("  repositories: {}", w.root().join("code").display());
    println!("  try: cargo xtask gxm {name} -- repos");
    println!("       cargo xtask app {name}            (opens the window inside the world)");
    Ok(())
}

/// `gxm [NAME] -- ARGS` or `app [NAME] -- ARGS`.
fn in_world(what: &str, args: &[String]) -> Result<(), String> {
    let split = args.iter().position(|a| a == "--").unwrap_or(args.len());
    let name = args[..split].first().map_or("mockup", String::as_str);
    let passthrough = args.get(split + 1..).unwrap_or_default();
    let w = build_world(name)?;
    let bin = cargo_bin(
        if what == "gxm" { "gxm-cli" } else { "gxm-app" },
        if what == "gxm" { "gxm" } else { "git-ex-machina" },
    )?;
    let status = Command::new(bin)
        .args(passthrough)
        .envs(w.env().child_vars())
        .current_dir(w.root().join("code"))
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("{what} exited with {status}")) }
}

/// Builds a workspace binary (debug) and returns its path.
pub fn cargo_bin(package: &str, bin: &str) -> Result<PathBuf, String> {
    run(Command::new("cargo").args(["build", "--quiet", "-p", package, "--bin", bin]).current_dir(root()))?;
    Ok(target().join("debug").join(bin))
}

/// Runs a command, failing with its name when it fails.
pub fn run(cmd: &mut Command) -> Result<(), String> {
    let shown = format!("{cmd:?}");
    let status = cmd.status().map_err(|e| format!("{shown}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{shown} failed: {status}")) }
}
