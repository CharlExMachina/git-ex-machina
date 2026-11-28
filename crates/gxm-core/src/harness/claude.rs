//! Claude Code, through its `claude` CLI in stream-json mode: the transport the Claude Agent SDK
//! uses under T3 Code's `ClaudeAdapterV2.ts`. A prompt goes in as a `user` message on stdin;
//! out come `system`, `assistant`, `user` (tool results) and finally `result`.
//!
//! Its health check follows T3 Code's `ClaudeProvider.ts`: `claude --version`, then the SDK's
//! `initialize` control request, whose answer names the account (or says there's none).

use std::collections::HashMap;
use std::path::Path;

use serde_json::{Value, json};

use super::event::{Change, Ending, Event, FileEdit, Tool};
use super::probe::{self, Fault};
use super::session::{Protocol, Said};
use super::{Auth, Driver, Harness, HarnessStatus, RuntimeMode, Start};
use crate::env::Env;

pub struct ClaudeCode;

/// The flags every stream-json session and probe starts with.
const STREAM: [&str; 6] = ["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"];
const INIT_ID: &str = "gxm-initialize";

pub const SIGNED_OUT: &str = "Claude Code is not authenticated. Run `claude auth login` and try again.";

impl Driver for ClaudeCode {
    fn harness(&self) -> Harness {
        Harness::ClaudeCode
    }

    fn name(&self) -> &'static str {
        "Claude Code"
    }

    fn binary(&self) -> &'static str {
        "claude"
    }

    fn install_hint(&self) -> &'static str {
        "Install it with `npm install -g @anthropic-ai/claude-code`, then sign in with `claude auth login`."
    }

    fn probe(&self, env: &Env, program: &Path) -> HarnessStatus {
        let mut s = HarnessStatus {
            harness: Harness::ClaudeCode,
            path: Some(program.to_path_buf()),
            version: None,
            auth: Auth::Unknown,
            problem: None,
        };
        match probe::version(env, program, probe::VERSION_TIMEOUT) {
            Ok(text) => s.version = probe::parse_version(&text),
            Err(Fault::Timeout) => {
                s.problem = Some("Claude Code is installed but failed to run. Timed out while running command.".into());
                return s;
            }
            Err(_) => {
                s.problem = Some("Claude Code is installed but failed to run.".to_owned());
                return s;
            }
        }
        let mut account = None;
        let asked = probe::converse(env, program, &STREAM, vec![initialize()], probe::AUTH_TIMEOUT, |line| {
            account = initialized(line);
            (Vec::new(), account.is_some())
        });
        match (asked, account) {
            (Ok(()), Some(Some(who))) => s.auth = Auth::SignedIn(who.label()),
            (Ok(()), Some(None)) => {
                s.auth = Auth::SignedOut;
                s.problem = Some(SIGNED_OUT.to_owned());
            }
            // It ran, so the sign-in is only unknown: T3 Code warns and lets it be used.
            (Err(Fault::Timeout), _) => {}
            (_, _) => s.problem = Some("Claude Code failed while checking account status.".to_owned()),
        }
        s
    }

    fn launch(&self, start: &Start) -> (Vec<String>, Box<dyn Protocol>) {
        let mut args: Vec<String> = STREAM.iter().map(|a| (*a).to_owned()).collect();
        // As T3 Code maps its runtime modes (`permissionModeForClaudeRuntimePolicy`).
        match start.mode {
            RuntimeMode::ApprovalRequired => args.extend(["--permission-mode".to_owned(), "default".to_owned()]),
            RuntimeMode::FullAccess => args.push("--dangerously-skip-permissions".to_owned()),
        }
        (args, Box::new(Stream::new(&start.prompt)))
    }
}

/// The SDK's first control request: it answers with the session's capabilities and account.
fn initialize() -> String {
    json!({ "type": "control_request", "request_id": INIT_ID, "request": { "subtype": "initialize" } }).to_string()
}

/// Who's signed in, from the answer to [`initialize`]: Some(None) when no one is.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Account {
    email: Option<String>,
    api_key: bool,
    provider: Option<String>,
}

impl Account {
    fn label(&self) -> Option<String> {
        match (&self.email, self.api_key, self.provider.as_deref()) {
            (Some(e), _, _) => Some(e.clone()),
            (None, true, _) => Some("API key".to_owned()),
            (None, false, Some("bedrock")) => Some("Amazon Bedrock".to_owned()),
            _ => None,
        }
    }
}

/// Reads the `initialize` answer, if `line` is it. As T3 Code's `claudeAuthStatus`: only a
/// token source of `none`, with no API key or account behind it, means signed out.
fn initialized(line: &str) -> Option<Option<Account>> {
    let v: Value = serde_json::from_str(line).ok()?;
    let response = v.get("response").filter(|_| v["type"] == "control_response")?;
    if response["request_id"] != INIT_ID {
        return None;
    }
    let a = &response["response"]["account"];
    let str_of = |k: &str| a.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned);
    let provider = str_of("apiProvider");
    let api_key = str_of("apiKeySource").is_some_and(|k| k != "none");
    let account = Account { email: str_of("email"), api_key, provider: provider.clone() };
    let third_party = provider.as_deref().is_some_and(|p| p != "firstParty");
    let token = str_of("tokenSource").is_some_and(|t| t != "none");
    let signed_in = third_party || token || api_key || account.email.is_some() || str_of("subscriptionType").is_some();
    Some(signed_in.then_some(account))
}

/// A session's stream-json, read.
struct Stream {
    prompt: String,
    /// Edits asked for, by tool-use id, until their results say they happened.
    edits: HashMap<String, (String, FileEdit)>,
}

impl Stream {
    fn new(prompt: &str) -> Stream {
        Stream { prompt: prompt.to_owned(), edits: HashMap::new() }
    }
}

impl Protocol for Stream {
    fn open(&mut self) -> Vec<Said> {
        let message = json!({
            "type": "user",
            "message": { "role": "user", "content": self.prompt },
            "parent_tool_use_id": null,
        });
        vec![Said::Reply(message.to_string())]
    }

    fn line(&mut self, line: &str) -> Vec<Said> {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return Vec::new() };
        let text = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        match (v["type"].as_str(), v["subtype"].as_str()) {
            (Some("system"), Some("init")) => vec![Said::Event(Event::Started { session: text(&v["session_id"]) })],
            (Some("system"), Some("api_retry")) => {
                vec![Said::Event(Event::Warning("Claude's API is busy; retrying".to_owned()))]
            }
            (Some("assistant"), _) => self.assistant(&v["message"]["content"]),
            (Some("user"), _) => self.results(&v["message"]["content"]),
            (Some("result"), _) => result(&v),
            _ => Vec::new(),
        }
    }

    fn interrupt(&mut self) -> Vec<Said> {
        let r =
            json!({ "type": "control_request", "request_id": "gxm-interrupt", "request": { "subtype": "interrupt" } });
        vec![Said::Reply(r.to_string())]
    }
}

impl Stream {
    fn assistant(&mut self, content: &Value) -> Vec<Said> {
        let mut out = Vec::new();
        for block in content.as_array().into_iter().flatten() {
            match block["type"].as_str() {
                Some("text") => {
                    let t = block["text"].as_str().unwrap_or_default();
                    if !t.trim().is_empty() {
                        out.push(Said::Event(Event::Text(t.to_owned())));
                    }
                }
                Some("tool_use") => {
                    let name = block["name"].as_str().unwrap_or("tool").to_owned();
                    let input = &block["input"];
                    // As T3 Code classifies Claude's tools: these change files.
                    if let ("Edit" | "MultiEdit" | "Write" | "NotebookEdit", Some(path)) =
                        (name.as_str(), input["file_path"].as_str().or(input["notebook_path"].as_str()))
                    {
                        let change = if name == "Write" { Change::Added } else { Change::Modified };
                        let id = block["id"].as_str().unwrap_or_default().to_owned();
                        self.edits.insert(id, (name, FileEdit { path: path.to_owned(), change }));
                    } else {
                        out.push(Said::Event(Event::Tool(Tool { detail: detail(input), name })));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Tool results: an edit happened once its result comes back without an error.
    fn results(&mut self, content: &Value) -> Vec<Said> {
        let mut out = Vec::new();
        for block in content.as_array().into_iter().flatten().filter(|b| b["type"] == "tool_result") {
            let Some((tool, edit)) = block["tool_use_id"].as_str().and_then(|id| self.edits.remove(id)) else {
                continue;
            };
            if block["is_error"].as_bool() == Some(true) {
                let why = block_text(&block["content"]);
                let why = probe::last_words(&why).unwrap_or_default();
                out.push(Said::Event(Event::Warning(format!("{tool} {} didn't happen: {why}", edit.path))));
            } else {
                out.push(Said::Event(Event::Edit(vec![edit])));
            }
        }
        out
    }
}

/// A tool's input on one line: the part a person recognises.
fn detail(input: &Value) -> String {
    let first = ["command", "file_path", "path", "pattern", "url", "query", "description", "prompt"]
        .iter()
        .find_map(|k| input[*k].as_str());
    first.and_then(|s| s.lines().next()).unwrap_or_default().to_owned()
}

/// A tool result's content: a string, or text blocks.
fn block_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// The turn's end, as T3 Code's `terminalStatusFromResult` reads it; with what was refused for
/// want of approval first.
fn result(v: &Value) -> Vec<Said> {
    let mut out: Vec<Said> = v["permission_denials"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| {
            let tool = d["tool_name"].as_str().unwrap_or("tool");
            let what = detail(&d["tool_input"]);
            Said::Event(Event::Approval {
                what: if what.is_empty() { tool.to_owned() } else { format!("{tool} {what}") },
            })
        })
        .collect();
    let aborted = v["terminal_reason"].as_str().is_some_and(|r| r.starts_with("aborted"));
    let failed = v["is_error"].as_bool() == Some(true) || v["subtype"].as_str().is_some_and(|s| s != "success");
    let ending = if aborted {
        Ending::Stopped
    } else if failed {
        let errors: Vec<&str> = v["errors"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let why = v["result"]
            .as_str()
            .map(str::to_owned)
            .filter(|r| !r.is_empty())
            .or_else(|| (!errors.is_empty()).then(|| errors.join("; ")));
        Ending::Failed(
            why.unwrap_or_else(|| format!("Claude Code ended with {}", v["subtype"].as_str().unwrap_or("an error"))),
        )
    } else {
        Ending::Completed
    };
    out.push(Said::TurnEnded(ending));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines as Claude Code prints them, from T3 Code's recorded fixtures (`tool_call_read_only`,
    /// `tool_call_denied_write`, `claude_result_is_error`), trimmed.
    const INIT: &str = r#"{"type":"system","subtype":"init","apiKeySource":"none","claude_code_version":"2.1.111","cwd":"/tmp/w","tools":[],"model":"claude-sonnet-4-6","permissionMode":"default","session_id":"473fb58f-85a3-4f21-b9d3-b1fdf5ef41f9"}"#;
    const READS: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me read both files."},{"type":"text","text":"I'll read both files simultaneously!"},{"type":"tool_use","id":"toolu_017Q","name":"Read","input":{"file_path":"/tmp/w/package.json"}}]},"session_id":"473f"}"#;
    const EDIT: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01ED","name":"Edit","input":{"file_path":"src/lib.rs","old_string":"<<<<<<<","new_string":"fn merged() {}"}}]}}"#;
    const EDITED: &str = r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_01ED","type":"tool_result","content":"The file src/lib.rs has been updated."}]},"parent_tool_use_id":null}"#;
    const WRITE: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_016S","name":"Write","input":{"file_path":"/tmp/w/.denied.txt","content":"approval fixture"}}]}}"#;
    const DECLINED: &str = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"User declined tool execution.","is_error":true,"tool_use_id":"toolu_016S"}]}}"#;
    const DONE: &str = r#"{"type":"result","subtype":"success","is_error":false,"api_error_status":null,"duration_ms":4122,"num_turns":3,"result":"read only tool fixture complete","stop_reason":"end_turn","session_id":"473f","total_cost_usd":0.0055614}"#;
    const DENIED: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"I need permission","permission_denials":[{"tool_name":"Bash","tool_use_id":"toolu_9","tool_input":{"command":"cargo test"}}]}"#;
    const UNAUTHORIZED: &str = r#"{"type":"result","subtype":"success","is_error":true,"api_error_status":401,"result":"Failed to authenticate. API Error: 401 Invalid authentication credentials"}"#;
    const ABORTED: &str =
        r#"{"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"aborted_streaming"}"#;

    fn events(lines: &[&str]) -> Vec<Said> {
        let mut s = Stream::new("Resolve the conflicts");
        lines.iter().flat_map(|l| s.line(l)).collect()
    }

    #[test]
    fn the_prompt_goes_in_as_a_user_message() {
        let mut s = Stream::new("Resolve the conflicts");
        let opened = s.open();
        let [Said::Reply(line)] = opened.as_slice() else { panic!("one reply: {opened:?}") };
        let v: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            (v["type"].as_str(), v["message"]["content"].as_str()),
            (Some("user"), Some("Resolve the conflicts"))
        );
    }

    #[test]
    fn a_session_streams_text_tools_and_edits_then_completes() {
        let said = events(&[INIT, READS, "not json", EDIT, EDITED, DONE]);
        assert_eq!(
            said,
            [
                Said::Event(Event::Started { session: "473fb58f-85a3-4f21-b9d3-b1fdf5ef41f9".to_owned() }),
                Said::Event(Event::Text("I'll read both files simultaneously!".to_owned())),
                Said::Event(Event::Tool(Tool { name: "Read".to_owned(), detail: "/tmp/w/package.json".to_owned() })),
                Said::Event(Event::Edit(vec![FileEdit { path: "src/lib.rs".to_owned(), change: Change::Modified }])),
                Said::TurnEnded(Ending::Completed),
            ]
        );
    }

    #[test]
    fn a_declined_edit_is_no_edit_and_refusals_are_named() {
        let said = events(&[WRITE, DECLINED, DENIED]);
        assert_eq!(
            said,
            [
                Said::Event(Event::Warning(
                    "Write /tmp/w/.denied.txt didn't happen: User declined tool execution.".into()
                )),
                Said::Event(Event::Approval { what: "Bash cargo test".to_owned() }),
                Said::TurnEnded(Ending::Completed),
            ]
        );
    }

    #[test]
    fn errors_and_aborts_end_the_turn_as_they_should() {
        let failed = Ending::Failed("Failed to authenticate. API Error: 401 Invalid authentication credentials".into());
        assert_eq!(events(&[UNAUTHORIZED]), [Said::TurnEnded(failed)]);
        assert_eq!(events(&[ABORTED]), [Said::TurnEnded(Ending::Stopped)]);
    }

    #[test]
    fn the_initialize_answer_says_who_is_signed_in() {
        let answer = |account: &str| {
            format!(
                r#"{{"type":"control_response","response":{{"subtype":"success","request_id":"{INIT_ID}","response":{{"commands":[],"account":{account}}}}}}}"#
            )
        };
        let who =
            initialized(&answer(r#"{"email":"ana@example.com","subscriptionType":"max","tokenSource":"claude.ai"}"#));
        assert_eq!(who.flatten().and_then(|a| a.label()).as_deref(), Some("ana@example.com"));
        let key = initialized(&answer(r#"{"tokenSource":"none","apiKeySource":"ANTHROPIC_API_KEY"}"#));
        assert_eq!(key.flatten().and_then(|a| a.label()).as_deref(), Some("API key"));
        assert_eq!(initialized(&answer(r#"{"tokenSource":"none","apiKeySource":"none"}"#)), Some(None), "signed out");
        assert_eq!(initialized(INIT), None, "not the answer");
    }

    #[test]
    fn runtime_modes_map_to_permission_modes() {
        let start = |mode| Start { dir: "/w".into(), prompt: "p".to_owned(), mode };
        let (asks, _) = ClaudeCode.launch(&start(RuntimeMode::ApprovalRequired));
        assert!(asks.windows(2).any(|w| w == ["--permission-mode", "default"]), "{asks:?}");
        let (free, _) = ClaudeCode.launch(&start(RuntimeMode::FullAccess));
        assert!(free.contains(&"--dangerously-skip-permissions".to_owned()), "{free:?}");
        assert!(free.starts_with(&STREAM.map(str::to_owned)));
    }
}
