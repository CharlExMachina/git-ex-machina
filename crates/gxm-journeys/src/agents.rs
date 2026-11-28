//! Bringing your own agent harness: Settings › Agents finds the coding agents installed on this
//! computer (Claude Code, Codex), says why one can't be used yet, and keeps the one the person
//! prefers; `gxm agents run` then streams a session in a folder to its end.

use std::time::Duration;

use gxm_app::keys::{self, setting, setting_item};
use gxm_core::harness::{self, Event, Harness, RuntimeMode, Start};
use gxm_world::World;
use gxm_world::fixtures::agents::{self, Agent};

use crate::Journey;

/// The demo workspace, on a computer with no agent harness installed yet.
fn agents_world(w: &World) {
    gxm_world::fixtures::mockup(w);
}

fn agents_page() -> String {
    keys::settings_section("Agents")
}

fn agent(id: &str) -> String {
    setting_item("agent", id)
}

pub fn agents_settings() -> Journey {
    Journey::new(
        "agents_settings",
        "Someone who codes with an agent opens Settings › Agents: with nothing installed it says how to add one; it explains why Claude Code isn't usable until they sign in; with Claude Code and Codex both ready they pick Codex, which the app remembers; and a run from the terminal streams Codex's session to its end.",
        agents_world,
    )
    // Nothing installed: the page says what an agent is for here and how to add one.
    .tap(keys::SETTINGS)
    .wait_for(keys::SETTINGS_DIALOG, "studio/atlas-web")
    .tap(agents_page())
    .wait_for(keys::AGENTS_EMPTY, "No coding agent found")
    .see_in(keys::AGENTS_EMPTY, "npm install -g @anthropic-ai/claude-code")
    .see_in(keys::AGENTS_EMPTY, "npm install -g @openai/codex")
    .gone(agent("claude-code"))
    .mark("empty")
    // Claude Code arrives, signed out: it's listed, with why it can't be used yet.
    .outside("installs Claude Code in a terminal, without signing in", |w| agents::claude(w, false, Agent::Works))
    .tap(setting("check-agents"))
    .wait_for(agent("claude-code"), "Run `claude auth login`")
    .see_in(agent("claude-code"), agents::CLAUDE_VERSION)
    .see_in(agent("codex"), "(`codex`) was not found on PATH")
    .gone(keys::AGENTS_EMPTY)
    .gone(setting("prefer-claude-code"))
    .mark("signed-out")
    // Signed in to Claude Code, and Codex installed and signed in: both are ready, and the
    // first is used until the person picks.
    .outside("signs in to Claude Code, and installs Codex signed in", |w| {
        agents::claude(w, true, Agent::Works);
        agents::codex(w, true);
    })
    .tap(setting("check-agents"))
    .wait_for(agent("codex"), agents::AGENT_EMAIL)
    .see_in(agent("claude-code"), agents::AGENT_EMAIL)
    .see_in(agent("codex"), agents::CODEX_VERSION)
    .see_in(agent("claude-code"), "Preferred")
    .see_in(setting("prefer-codex"), "Use Codex")
    .gone(setting("prefer-claude-code"))
    .mark("both-ready")
    // Picking Codex saves it, in the app's own settings.
    .tap(setting("prefer-codex"))
    .wait_for(keys::SETTINGS_STATUS, "Saved for every repository")
    .see_in(agent("codex"), "Preferred")
    .see_in(setting("prefer-claude-code"), "Use Claude Code")
    .world_check("the app's config keeps Codex as the preferred agent", |w| {
        let saved = std::fs::read_to_string(w.env().config_dir.join("agents.json")).map_err(|e| e.to_string())?;
        let preferred = harness::preferred(w.env());
        if preferred == Some(Harness::Codex) { Ok(()) } else { Err(format!("preferred {preferred:?} from {saved}")) }
    })
    // Closed and opened again, the choice holds.
    .tap(keys::SETTINGS_CLOSE)
    .gone(keys::SETTINGS_DIALOG)
    .tap(keys::SETTINGS)
    .wait_for(keys::SETTINGS_DIALOG, "studio/atlas-web")
    .tap(agents_page())
    .wait_for(agent("codex"), "Preferred")
    .mark("codex-preferred")
    // From the terminal, the preferred agent runs a prompt in a folder, streaming to its end.
    .world_check("`gxm agents run` streams Codex's session in atlas-web to its end", |w| {
        let lines = run_lines(w)?;
        let want = [
            "● started session 01a0d5eb-f0c0-7481-a417-5e8871805cbc",
            "→ command: git status",
            "✎ edited README.md",
            "Resolved: README.md says so.",
            "✓ done",
        ];
        if lines != want {
            return Err(format!("it streamed {lines:#?}"));
        }
        let readme = std::fs::read_to_string(w.code().join("atlas-web/README.md")).map_err(|e| e.to_string())?;
        if readme.ends_with("Resolved by Codex.\n") { Ok(()) } else { Err(format!("README.md holds {readme:?}")) }
    })
}

/// What `gxm agents run --in ~/code/atlas-web "…"` prints: each event of the preferred agent's
/// session on its line, as the CLI prints them.
fn run_lines(w: &World) -> Result<Vec<String>, String> {
    let env = w.env();
    let h = harness::preferred(env).ok_or("no agent is usable")?;
    let start = Start {
        dir: w.code().join("atlas-web"),
        prompt: "Resolve the merge conflicts".to_owned(),
        mode: RuntimeMode::ApprovalRequired,
    };
    let session = harness::start(env, h, &start, Box::new(|| {}))?;
    let mut lines = Vec::new();
    while let Some(e) = session.next_event(Duration::from_secs(10)) {
        lines.push(e.line());
        if matches!(e, Event::Ended(_)) {
            return Ok(lines);
        }
    }
    Err(format!("the session never ended: {lines:?}"))
}
