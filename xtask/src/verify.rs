//! `cargo xtask verify`: everything CI runs, in the order that fails fastest.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{root, run};

/// A Rust file's non-test lines: past this, split it by capability (CLAUDE.md, "Garden").
const FILE_SOFT_CAP: usize = 600;
const FILE_HARD_CAP: usize = 1000;
const TEST_FILE_CAP: usize = 1000;

pub fn verify(fast: bool) -> Result<(), String> {
    step("rustfmt", || run(cargo().args(["fmt", "--all", "--check"])))?;
    step("garden", garden)?;
    let clippy_targets: &[&str] = if fast { &["--workspace"] } else { &["--workspace", "--all-targets"] };
    step("clippy", || run(cargo().arg("clippy").args(clippy_targets).args(["--quiet", "--", "-D", "warnings"])))?;
    step("tests", || run(cargo().args(["test", "--workspace", "--quiet"])))?;
    if !fast {
        optional("cargo-deny", "cargo-deny", &["check"])?;
        optional("typos", "typos", &[])?;
        optional("cargo-machete", "cargo-machete", &[])?;
    }
    println!("verify: all green");
    Ok(())
}

pub fn bless() -> Result<(), String> {
    run(cargo().args(["test", "--workspace"]).env("GXM_BLESS", "1"))?;
    println!("bless: goldens rewritten. Review `git diff` and show every changed golden in the PR.");
    Ok(())
}

fn cargo() -> Command {
    let mut c = Command::new("cargo");
    c.current_dir(root());
    c
}

fn step(name: &str, f: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    println!("verify: {name}");
    f().map_err(|e| format!("{name}: {e}"))
}

/// Tools CI installs. Locally a missing one is a warning; on CI (`CI=true`) it's a failure.
/// Each runs as its own binary, not as `cargo <tool>`: under `cargo xtask`, cargo-machete
/// takes the subcommand's name for a folder to scan.
fn optional(name: &str, bin: &str, args: &[&str]) -> Result<(), String> {
    let found = Command::new(bin).arg("--version").output().is_ok_and(|o| o.status.success());
    if !found {
        #[expect(clippy::disallowed_methods, reason = "xtask reads CI's own flag")]
        let on_ci = std::env::var("CI").is_ok_and(|v| v == "true");
        if on_ci {
            return Err(format!("{name} isn't installed"));
        }
        println!("verify: {name} skipped (not installed; CI runs it: cargo install {bin})");
        return Ok(());
    }
    step(name, || run(Command::new(bin).args(args).current_dir(root())))
}

/// Checks no linter does: file sizes, folder shapes, and that every journey has a test.
fn garden() -> Result<(), String> {
    let mut problems = Vec::new();
    for file in rust_files(&root().join("crates")).into_iter().chain(rust_files(&root().join("xtask"))) {
        let text = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
        let rel = file.strip_prefix(root()).unwrap_or(&file).display().to_string();
        let is_test = rel.contains("/tests/") || rel.ends_with("tests.rs") || rel.ends_with("_tests.rs");
        let lines = if is_test { text.lines().count() } else { non_test_lines(&text) };
        let cap = if is_test { TEST_FILE_CAP } else { FILE_HARD_CAP };
        if lines > cap {
            problems.push(format!("{rel}: {lines} lines (cap {cap}); split it by capability"));
        } else if !is_test && lines > FILE_SOFT_CAP {
            println!("verify: garden warning: {rel} has {lines} lines (aim for under {FILE_SOFT_CAP})");
        }
        for (n, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if (t.starts_with("// TODO") || t.starts_with("// FIXME")) && !t.contains("(#") {
                problems.push(format!("{rel}:{}: a TODO needs an issue: `// TODO(#123): ...`", n + 1));
            }
        }
    }
    if problems.is_empty() { Ok(()) } else { Err(format!("\n  {}", problems.join("\n  "))) }
}

/// Lines before the first `#[cfg(test)]` module.
fn non_test_lines(text: &str) -> usize {
    text.lines().take_while(|l| !l.trim_start().starts_with("#[cfg(test)]")).count()
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "target") {
                out.extend(rust_files(&p));
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}
