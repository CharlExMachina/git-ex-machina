//! Agent harnesses on the world's path: fake `claude` and `codex` executables that speak the
//! real wire formats (Claude Code's stream-json, Codex's app-server JSON-RPC) with lines
//! recorded from T3 Code's fixtures, so the app's real drivers talk to them.
//!
//! They're `#!/bin/sh` scripts using only the shell's builtins: the world's path holds nothing
//! else, so they run the same on every machine.

use crate::World;

/// Claude Code's version in the world.
pub const CLAUDE_VERSION: &str = "2.1.111";
/// Codex's version in the world.
pub const CODEX_VERSION: &str = "0.156.1";
/// Who's signed in to both, when someone is.
pub const AGENT_EMAIL: &str = "ana@example.com";

/// What a fake session does with its prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    /// Reads, edits `README.md` (for real) and finishes.
    Works,
    /// Starts, says it's looking, and keeps going until it's stopped.
    Lingers,
}

/// Single-quoted for the shell.
fn quoted(line: &str) -> String {
    format!("'{}'", line.replace('\'', r"'\''"))
}

/// Installs `claude`, signed in as [`AGENT_EMAIL`] or signed out.
pub fn claude(w: &World, signed_in: bool, agent: Agent) {
    let account = if signed_in {
        format!(
            r#"{{"email":"{AGENT_EMAIL}","subscriptionType":"max","tokenSource":"claude.ai","apiKeySource":"none","apiProvider":"firstParty"}}"#
        )
    } else {
        r#"{"tokenSource":"none","apiKeySource":"none","apiProvider":"firstParty"}"#.to_owned()
    };
    let session = "7d295979-f2e5-43cb-a9ee-b05f140f3ed8";
    let init = format!(
        r#"{{"type":"system","subtype":"init","apiKeySource":"none","claude_code_version":"{CLAUDE_VERSION}","cwd":"'"$PWD"'","tools":["Read","Edit","Bash"],"model":"claude-sonnet-4-6","permissionMode":"default","session_id":"{session}"}}"#
    );
    let say = |text: &str| {
        quoted(&format!(
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"{text}"}}]}},"parent_tool_use_id":null,"session_id":"{session}"}}"#
        ))
    };
    let tool = |id: &str, name: &str, input: &str| {
        quoted(&format!(
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"{id}","name":"{name}","input":{input}}}]}},"session_id":"{session}"}}"#
        ))
    };
    let result = |id: &str, text: &str| {
        quoted(&format!(
            r#"{{"type":"user","message":{{"role":"user","content":[{{"tool_use_id":"{id}","type":"tool_result","content":"{text}"}}]}},"parent_tool_use_id":null,"session_id":"{session}"}}"#
        ))
    };
    let work = match agent {
        Agent::Works => [
            say("I'll look at the README first."),
            tool("toolu_01Read", "Read", r#"{"file_path":"README.md"}"#),
            result("toolu_01Read", "1\t# atlas-web"),
            tool("toolu_02Edit", "Edit", r#"{"file_path":"README.md","old_string":"","new_string":"Resolved by Claude."}"#),
            // The edit happens, as Claude's Edit tool would make it.
            "printf '%s\\n' 'Resolved by Claude.' >> README.md".to_owned(),
            result("toolu_02Edit", "The file README.md has been updated."),
            say("Done: README.md says how the conflict was resolved."),
            quoted(&format!(
                r#"{{"type":"result","subtype":"success","is_error":false,"api_error_status":null,"duration_ms":4122,"num_turns":3,"result":"Done.","stop_reason":"end_turn","session_id":"{session}","total_cost_usd":0.0055614}}"#
            )),
        ]
        .iter()
        .map(|l| if l.starts_with('\'') { format!("      printf '%s\\n' {l}") } else { format!("      {l}") })
        .collect::<Vec<_>>()
        .join("\n"),
        Agent::Lingers => format!("      printf '%s\\n' {}\n      lingering=1", say("Looking at the conflicts…")),
    };
    let aborted = quoted(&format!(
        r#"{{"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"aborted_streaming","session_id":"{session}"}}"#
    ));
    let script = format!(
        r#"#!/bin/sh
# Claude Code {CLAUDE_VERSION}, as the world installs it.
case "$1" in
  --version) echo "{CLAUDE_VERSION} (Claude Code)"; exit 0 ;;
  -p) ;;
  *) echo "claude: this world's claude only speaks stream-json" >&2; exit 2 ;;
esac
while IFS= read -r line; do
  case "$line" in
    *'"subtype":"initialize"'*)
      id=${{line#*\"request_id\":\"}}; id=${{id%%\"*}}
      printf '%s%s%s\n' '{{"type":"control_response","response":{{"subtype":"success","request_id":"' "$id" '","response":{{"commands":[],"account":{account}}}}}}}' ;;
    *'"subtype":"interrupt"'*)
      printf '%s\n' {aborted}
      exit 0 ;;
    *'"type":"user"'*)
      printf '%s\n' '{init}'
{work}
      [ -n "$lingering" ] || exit 0 ;;
  esac
done
"#
    );
    w.install_tool("claude", &script);
}

/// Installs `codex`, signed in with ChatGPT as [`AGENT_EMAIL`] or signed out.
pub fn codex(w: &World, signed_in: bool) {
    let account = if signed_in {
        format!(r#""account":{{"type":"chatgpt","email":"{AGENT_EMAIL}","planType":"plus"}},"requiresOpenaiAuth":true"#)
    } else {
        r#""account":null,"requiresOpenaiAuth":true"#.to_owned()
    };
    let thread = "01a0d5eb-f0c0-7481-a417-5e8871805cbc";
    let turn = "01a0d5eb-f176-71d0-8e1c-a459700443de";
    let note =
        |method: &str, params: &str| format!("      printf '%s\\n' '{{\"method\":\"{method}\",\"params\":{params}}}'");
    let ids = format!(r#""threadId":"{thread}","turnId":"{turn}""#);
    let work = [
        note("turn/started", &format!(r#"{{"threadId":"{thread}","turn":{{"id":"{turn}","items":[],"status":"inProgress","error":null}}}}"#)),
        note(
            "item/started",
            &format!(r#"{{"item":{{"type":"commandExecution","id":"exec-1a2d","command":"/bin/bash -lc \"git status\"","status":"inProgress","commandActions":[{{"type":"unknown","command":"git status"}}]}},{ids}}}"#),
        ),
        note("item/agentMessage/delta", &format!(r#"{{{ids},"itemId":"msg_08fd","delta":"Resolved"}}"#)),
        "      printf '%s\\n' 'Resolved by Codex.' >> README.md".to_owned(),
        note(
            "item/completed",
            &format!(r#"{{"item":{{"type":"fileChange","id":"exec-114e","changes":[{{"path":"'"$PWD"'/README.md","kind":{{"type":"update"}},"diff":"+Resolved by Codex."}}],"status":"completed"}},{ids}}}"#),
        ),
        note(
            "item/completed",
            &format!(r#"{{"item":{{"type":"agentMessage","id":"msg_08fd","text":"Resolved: README.md says so.","phase":"final_answer"}},{ids}}}"#),
        ),
        note("turn/completed", &format!(r#"{{"threadId":"{thread}","turn":{{"id":"{turn}","items":[],"status":"completed","error":null,"durationMs":4289}}}}"#)),
    ]
    .join("\n");
    let reply = |result: &str| format!("      printf '%s%s%s\\n' '{{\"id\":' \"$id\" ',\"result\":{result}}}'");
    let script = format!(
        r#"#!/bin/sh
# Codex {CODEX_VERSION}, as the world installs it.
case "$1" in
  --version) echo "codex-cli {CODEX_VERSION}"; exit 0 ;;
  app-server) ;;
  *) echo "codex: this world's codex only serves app-server" >&2; exit 2 ;;
esac
while IFS= read -r line; do
  id=${{line#*\"id\":}}; id=${{id%%,*}}
  case "$line" in
    *'"method":"initialize"'*)
{init}
      ;;
    *'"method":"account/read"'*)
{account_reply}
      ;;
    *'"method":"thread/start"'*)
{thread_reply}
      ;;
    *'"method":"turn/start"'*)
{turn_reply}
{work}
      ;;
  esac
done
"#,
        init = reply(&format!(
            r#"{{"userAgent":"codex_cli_rs/{CODEX_VERSION} (Ubuntu 24.4.0; x86_64)","codexHome":"'"$HOME"'/.codex","platformFamily":"unix","platformOs":"linux"}}"#
        )),
        account_reply = reply(&format!("{{{account}}}")),
        thread_reply = reply(&format!(
            r#"{{"thread":{{"id":"{thread}","cwd":"'"$PWD"'","cliVersion":"{CODEX_VERSION}","turns":[]}},"approvalPolicy":"untrusted"}}"#
        )),
        turn_reply = reply(&format!(r#"{{"turn":{{"id":"{turn}","items":[],"status":"inProgress","error":null}}}}"#)),
    );
    w.install_tool("codex", &script);
}
