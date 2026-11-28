//! Asking a harness about itself, briefly: its `--version`, or a short conversation over its
//! own protocol (a handshake that says who's signed in), each with a deadline, as T3 Code's
//! provider probes do (`snapshotProbe.ts`).

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::env::Env;

/// How long `--version` may take (T3 Code allows 20 seconds; a cold `npx` shim is slow).
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(20);
/// How long a handshake that reads the account may take.
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(25);

/// Why a probe got no answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The program couldn't be started.
    Spawn(String),
    /// It didn't answer in time.
    Timeout,
    /// It exited before answering, with this code and the end of what it said.
    Exited(Option<i32>, String),
}

/// What a conversation does after a line: send these, and whether it has what it needs.
pub type Turn = (Vec<String>, bool);

/// The harness's environment: the person's own, or (isolated) only the sandbox's home and path,
/// so a test's harness can't see the developer's keys, tools or `~/.claude`.
pub fn child_env(env: &Env, cmd: &mut Command) {
    if env.isolated {
        cmd.env_clear().envs(env.git_vars.iter().map(|(k, v)| (k, v)));
    }
    cmd.env("HOME", &env.home);
    if let Ok(path) = std::env::join_paths(&env.path) {
        cmd.env("PATH", path);
    }
}

fn command(env: &Env, program: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    child_env(env, &mut cmd);
    cmd.current_dir(&env.home);
    cmd
}

/// Runs `program --version`: its output, stdout then stderr, when it exits 0.
pub fn version(env: &Env, program: &Path, timeout: Duration) -> Result<String, Fault> {
    env.off_ui_thread(&format_args!("`{} --version`", program.display()));
    let mut child = command(env, program, &["--version"]).spawn().map_err(|e| Fault::Spawn(e.to_string()))?;
    drop(child.stdin.take());
    let (mut out, mut err) = (child.stdout.take(), child.stderr.take());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(o) = out.as_mut() {
            o.read_to_string(&mut text).ok();
        }
        if let Some(e) = err.as_mut() {
            e.read_to_string(&mut text).ok();
        }
        tx.send(text).ok();
    });
    let text = rx.recv_timeout(timeout).map_err(|_| {
        child.kill().ok();
        Fault::Timeout
    })?;
    match child.wait().map(|s| s.code()) {
        Ok(Some(0)) => Ok(text),
        Ok(code) => Err(Fault::Exited(code, text)),
        Err(e) => Err(Fault::Spawn(e.to_string())),
    }
}

/// Starts `program args`, writes `opening`, and hands each line it prints to `on` until `on`
/// says it has what it needs; then the program is stopped.
pub fn converse(
    env: &Env,
    program: &Path,
    args: &[&str],
    opening: Vec<String>,
    timeout: Duration,
    mut on: impl FnMut(&str) -> Turn,
) -> Result<(), Fault> {
    env.off_ui_thread(&format_args!("asking {} about itself", program.display()));
    let mut child = command(env, program, args).spawn().map_err(|e| Fault::Spawn(e.to_string()))?;
    let mut stdin = child.stdin.take();
    let (tx, rx) = mpsc::channel();
    if let Some(out) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    let mut err = child.stderr.take();
    let stderr = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(e) = err.as_mut() {
            e.read_to_string(&mut text).ok();
        }
        text
    });
    let send = |lines: Vec<String>, stdin: &mut Option<std::process::ChildStdin>| {
        for l in lines {
            if let Some(s) = stdin.as_mut()
                && writeln!(s, "{l}").and_then(|()| s.flush()).is_err()
            {
                *stdin = None;
            }
        }
    };
    send(opening, &mut stdin);
    let deadline = Instant::now() + timeout;
    let answer = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(line) => {
                let (reply, done) = on(&line);
                send(reply, &mut stdin);
                if done {
                    break Ok(());
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break Err(Fault::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                drop(stdin.take());
                let code = child.wait().ok().and_then(|s| s.code());
                break Err(Fault::Exited(code, stderr.join().unwrap_or_default()));
            }
        }
    };
    drop(stdin);
    child.kill().ok();
    child.wait().ok();
    answer
}

/// The first `x.y.z` in what `--version` printed (`2.1.3 (Claude Code)`, `codex-cli 0.156.1`).
pub fn parse_version(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '/' || c == '(' || c == ')').find_map(|word| {
        let word = word.strip_prefix('v').unwrap_or(word);
        let core = word.split(['-', '+']).next().unwrap_or_default();
        let parts: Vec<&str> = core.split('.').collect();
        let numeric = parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
        numeric.then(|| word.trim_end_matches(['.', ',']).to_owned())
    })
}

/// The last thing a failed program said, for a reason a person can act on.
pub fn last_words(text: &str) -> Option<String> {
    text.lines().rev().map(str::trim).find(|l| !l.is_empty()).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_from_what_each_cli_prints() {
        assert_eq!(parse_version("2.1.3 (Claude Code)\n").as_deref(), Some("2.1.3"));
        assert_eq!(parse_version("codex-cli 0.156.1\n").as_deref(), Some("0.156.1"));
        assert_eq!(parse_version("t3code_desktop/0.156.1 (Debian 13.0.0; x86_64)").as_deref(), Some("0.156.1"));
        assert_eq!(parse_version("v1.2.3-beta.1").as_deref(), Some("1.2.3-beta.1"));
        assert_eq!(parse_version("version 12"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn the_last_words_skip_blank_lines() {
        assert_eq!(last_words("warming up\nInvalid API key\n\n").as_deref(), Some("Invalid API key"));
        assert_eq!(last_words("  \n"), None);
    }
}
