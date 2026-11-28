//! Codex, through `codex app-server`: JSON-RPC over stdio, one JSON object per line (no
//! `"jsonrpc"` field), as T3 Code's `CodexAdapterV2.ts` drives it. The client says
//! `initialize`, then `initialized`; `thread/start` opens a thread in the folder and
//! `turn/start` gives it the prompt; notifications (`item/started`, `item/completed`,
//! `turn/completed`) say what the agent does, and the server asks the client to approve
//! commands and edits (`item/commandExecution/requestApproval`).
//!
//! Its health check follows T3 Code's `CodexProvider.ts`: the same handshake, then
//! `account/read`; the version is in `initialize`'s user agent.

use std::path::Path;

use serde_json::{Value, json};

use super::event::{Change, Ending, Event, FileEdit, Tool};
use super::probe::{self, Fault};
use super::session::{Protocol, Said};
use super::{Auth, Driver, Harness, HarnessStatus, RuntimeMode, Start};
use crate::env::Env;

pub struct Codex;

pub const SIGNED_OUT: &str = "Codex CLI is not authenticated. Run `codex login` and try again.";

/// The requests a session makes, by id.
const INITIALIZE: u64 = 1;
const THREAD_START: u64 = 2;
const TURN_START: u64 = 3;
const INTERRUPT: u64 = 4;
const ACCOUNT_READ: u64 = 2;

impl Driver for Codex {
    fn harness(&self) -> Harness {
        Harness::Codex
    }

    fn name(&self) -> &'static str {
        "Codex"
    }

    fn binary(&self) -> &'static str {
        "codex"
    }

    fn install_hint(&self) -> &'static str {
        "Install it with `npm install -g @openai/codex`, then sign in with `codex login`."
    }

    fn probe(&self, env: &Env, program: &Path) -> HarnessStatus {
        let mut s = HarnessStatus {
            harness: Harness::Codex,
            path: Some(program.to_path_buf()),
            version: None,
            auth: Auth::Unknown,
            problem: None,
        };
        let (mut version, mut account) = (None, None);
        let asked = probe::converse(env, program, &["app-server"], vec![initialize()], probe::AUTH_TIMEOUT, |line| {
            let Ok(v) = serde_json::from_str::<Value>(line) else { return (Vec::new(), false) };
            match v["id"].as_u64() {
                Some(INITIALIZE) if v.get("result").is_some() || is_already_initialized(&v) => {
                    version = v["result"]["userAgent"].as_str().and_then(probe::parse_version);
                    let read = json!({ "id": ACCOUNT_READ, "method": "account/read", "params": {} });
                    (vec![initialized(), read.to_string()], false)
                }
                Some(ACCOUNT_READ) => {
                    account = Some(v["result"].clone());
                    (Vec::new(), true)
                }
                _ => (Vec::new(), false),
            }
        });
        // Without the app-server, the version still says what's installed.
        s.version = version.or_else(|| {
            probe::version(env, program, probe::VERSION_TIMEOUT).ok().as_deref().and_then(probe::parse_version)
        });
        match (asked, account) {
            (Ok(()), Some(a)) => match a.get("account").filter(|a| !a.is_null()) {
                Some(acct) => s.auth = Auth::SignedIn(label(acct)),
                None if a["requiresOpenaiAuth"].as_bool() == Some(true) => {
                    s.auth = Auth::SignedOut;
                    s.problem = Some(SIGNED_OUT.to_owned());
                }
                // Another model provider, which needs no OpenAI sign-in.
                None => s.auth = Auth::SignedIn(None),
            },
            (Err(Fault::Timeout), _) => {
                s.problem = Some("Timed out while checking Codex app-server provider status.".to_owned());
            }
            (Err(Fault::Spawn(_)), _) => {
                s.problem = Some(format!(
                    "Could not start Codex CLI (`{}`). Make sure it exists and can be run.",
                    program.display()
                ));
            }
            (Err(Fault::Exited(_, said)), _) => {
                let why = probe::last_words(&said).unwrap_or_else(|| "it exited".to_owned());
                s.problem = Some(format!("Codex app-server provider probe failed: {why}."));
            }
            (Ok(()), None) => s.problem = Some("Codex app-server provider probe failed: no answer.".to_owned()),
        }
        s
    }

    fn launch(&self, start: &Start) -> (Vec<String>, Box<dyn Protocol>) {
        (vec!["app-server".to_owned()], Box::new(AppServer::new(start)))
    }
}

fn initialize() -> String {
    let client = json!({ "name": "git-ex-machina", "title": "git-ex-machina", "version": env!("CARGO_PKG_VERSION") });
    json!({ "id": INITIALIZE, "method": "initialize", "params": { "clientInfo": client } }).to_string()
}

fn initialized() -> String {
    json!({ "method": "initialized" }).to_string()
}

/// A second `initialize` on a shared server is refused this way, and is fine.
fn is_already_initialized(v: &Value) -> bool {
    v["error"]["code"].as_i64() == Some(-32600)
}

/// Who's signed in, as T3 Code's `codexAccountAuthLabel` says it.
fn label(account: &Value) -> Option<String> {
    match account["type"].as_str() {
        Some("apiKey") => Some("OpenAI API key".to_owned()),
        Some("amazonBedrock") => Some("Amazon Bedrock".to_owned()),
        _ => account["email"].as_str().map(str::to_owned).or_else(|| Some("ChatGPT".to_owned())),
    }
}

/// A session on the app-server: the handshake, one thread, one turn.
struct AppServer {
    cwd: String,
    prompt: String,
    mode: RuntimeMode,
    thread: Option<String>,
    turn: Option<String>,
    /// The last error the server reported, for a turn that then fails.
    error: Option<String>,
}

impl AppServer {
    fn new(start: &Start) -> AppServer {
        AppServer {
            cwd: start.dir.display().to_string(),
            prompt: start.prompt.clone(),
            mode: start.mode,
            thread: None,
            turn: None,
            error: None,
        }
    }

    /// `turn/start`'s policy, as T3 Code's `codexRuntimeModeTurnDefaults` sets it.
    fn policy(&self) -> (Value, Value) {
        match self.mode {
            RuntimeMode::ApprovalRequired => (json!("untrusted"), json!({ "type": "readOnly" })),
            RuntimeMode::FullAccess => (json!("never"), json!({ "type": "dangerFullAccess" })),
        }
    }

    fn response(&mut self, id: u64, v: &Value) -> Vec<Said> {
        if let Some(e) = v.get("error").filter(|_| !(id == INITIALIZE && is_already_initialized(v))) {
            let why = e["message"].as_str().unwrap_or("Codex refused the request").to_owned();
            return if id == INTERRUPT { Vec::new() } else { vec![Said::TurnEnded(Ending::Failed(why))] };
        }
        match id {
            INITIALIZE => {
                let start = json!({ "id": THREAD_START, "method": "thread/start", "params": { "cwd": self.cwd } });
                vec![Said::Reply(initialized()), Said::Reply(start.to_string())]
            }
            THREAD_START => {
                let Some(thread) = v["result"]["thread"]["id"].as_str().map(str::to_owned) else {
                    return vec![Said::TurnEnded(Ending::Failed("Codex started no thread".to_owned()))];
                };
                let (approval, sandbox) = self.policy();
                let turn = json!({ "id": TURN_START, "method": "turn/start", "params": {
                    "threadId": thread,
                    "input": [{ "type": "text", "text": self.prompt }],
                    "cwd": self.cwd,
                    "approvalPolicy": approval,
                    "sandboxPolicy": sandbox,
                }});
                self.thread = Some(thread.clone());
                vec![Said::Event(Event::Started { session: thread }), Said::Reply(turn.to_string())]
            }
            TURN_START => {
                self.turn = v["result"]["turn"]["id"].as_str().map(str::to_owned);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// A request from the server: approvals are granted in full access and declined otherwise
    /// (no one is there to ask); anything else is refused as not understood.
    fn request(&self, id: &Value, method: &str, params: &Value) -> Vec<Said> {
        let what = match method {
            "item/commandExecution/requestApproval" => params["command"].as_str().unwrap_or("a command").to_owned(),
            "item/fileChange/requestApproval" => "changing files".to_owned(),
            "item/permissions/requestApproval" => "more permissions".to_owned(),
            _ => {
                let refuse =
                    json!({ "id": id, "error": { "code": -32601, "message": "git-ex-machina can't answer this" } });
                return vec![Said::Reply(refuse.to_string())];
            }
        };
        let full = self.mode == RuntimeMode::FullAccess;
        let answer = json!({ "id": id, "result": { "decision": if full { "accept" } else { "decline" } } });
        let mut out = vec![Said::Reply(answer.to_string())];
        if !full {
            out.insert(0, Said::Event(Event::Approval { what }));
        }
        out
    }

    fn notification(&mut self, method: &str, p: &Value) -> Vec<Said> {
        let item = &p["item"];
        match (method, item["type"].as_str()) {
            ("item/started", Some("commandExecution")) => {
                let command = item["commandActions"][0]["command"].as_str().or(item["command"].as_str());
                tool("command", command.unwrap_or_default())
            }
            ("item/started", Some("mcpToolCall")) => {
                let name =
                    format!("{}.{}", item["server"].as_str().unwrap_or("mcp"), item["tool"].as_str().unwrap_or("tool"));
                tool(&name, "")
            }
            ("item/completed", Some("webSearch")) => tool("web search", item["query"].as_str().unwrap_or_default()),
            ("item/completed", Some("agentMessage")) => {
                let text = item["text"].as_str().unwrap_or_default();
                if text.trim().is_empty() { Vec::new() } else { vec![Said::Event(Event::Text(text.to_owned()))] }
            }
            ("item/completed", Some("fileChange")) if item["status"] != "declined" && item["status"] != "failed" => {
                let edits: Vec<FileEdit> = item["changes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| {
                        let change = match c["kind"]["type"].as_str()? {
                            "add" => Change::Added,
                            "delete" => Change::Deleted,
                            _ => Change::Modified,
                        };
                        Some(FileEdit { path: relative(c["path"].as_str()?, &self.cwd), change })
                    })
                    .collect();
                if edits.is_empty() { Vec::new() } else { vec![Said::Event(Event::Edit(edits))] }
            }
            ("error", _) => {
                let why = p["error"]["message"].as_str().unwrap_or("Codex reported an error").to_owned();
                if p["willRetry"].as_bool() == Some(true) {
                    vec![Said::Event(Event::Warning(why))]
                } else {
                    self.error = Some(why);
                    Vec::new()
                }
            }
            ("turn/completed", _) => {
                let turn = &p["turn"];
                let ending = match turn["status"].as_str() {
                    Some("completed") => Ending::Completed,
                    Some("interrupted") => Ending::Stopped,
                    _ => Ending::Failed(
                        turn["error"]["message"]
                            .as_str()
                            .map(str::to_owned)
                            .or_else(|| self.error.clone())
                            .unwrap_or_else(|| "Codex's turn failed".to_owned()),
                    ),
                };
                vec![Said::TurnEnded(ending)]
            }
            _ => Vec::new(),
        }
    }
}

fn tool(name: &str, detail: &str) -> Vec<Said> {
    let detail = detail.lines().next().unwrap_or_default().to_owned();
    vec![Said::Event(Event::Tool(Tool { name: name.to_owned(), detail }))]
}

/// A changed file inside the session's folder, as the folder names it.
fn relative(path: &str, cwd: &str) -> String {
    Path::new(path).strip_prefix(cwd).map_or_else(|_| path.to_owned(), |p| p.display().to_string())
}

impl Protocol for AppServer {
    fn open(&mut self) -> Vec<Said> {
        vec![Said::Reply(initialize())]
    }

    fn line(&mut self, line: &str) -> Vec<Said> {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return Vec::new() };
        match (v.get("id"), v["method"].as_str()) {
            (Some(id), Some(method)) => self.request(id, method, &v["params"]),
            (Some(id), None) => id.as_u64().map(|id| self.response(id, &v)).unwrap_or_default(),
            (None, Some(method)) => self.notification(method, &v["params"]),
            (None, None) => Vec::new(),
        }
    }

    fn interrupt(&mut self) -> Vec<Said> {
        let (Some(thread), Some(turn)) = (&self.thread, &self.turn) else { return Vec::new() };
        let r =
            json!({ "id": INTERRUPT, "method": "turn/interrupt", "params": { "threadId": thread, "turnId": turn } });
        vec![Said::Reply(r.to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frames as `codex app-server` sends them, from T3 Code's recorded fixtures (`simple`,
    /// `command_output_streaming`, `tool_call_restricted_granular`, `turn_interrupt`), trimmed.
    const INITIALIZED: &str = r#"{"id":1,"result":{"userAgent":"t3code_desktop/0.156.1 (Debian 13.0.0; x86_64) unknown (t3code_desktop; 0.1.0)","codexHome":"/home/replay-user/.codex","platformFamily":"unix","platformOs":"linux"}}"#;
    const THREAD: &str = r#"{"id":2,"result":{"thread":{"id":"01a0d5eb-f0c0-7481-a417-5e8871805cbc","cwd":"/w","cliVersion":"0.156.1","turns":[]},"model":"gpt-6-luna","approvalPolicy":"on-request"}}"#;
    const TURN: &str = r#"{"id":3,"result":{"turn":{"id":"01a0d5eb-f176-71d0-8e1c-a459700443de","items":[],"status":"inProgress","error":null}}}"#;
    const WARNING: &str =
        r#"{"method":"configWarning","params":{"summary":"Codex could not find bubblewrap on PATH.","details":null}}"#;
    const COMMAND: &str = r#"{"method":"item/started","params":{"item":{"type":"commandExecution","id":"exec-1a2d","command":"/bin/zsh -lc \"git status\"","cwd":"/w","status":"inProgress","commandActions":[{"type":"unknown","command":"git status"}]},"threadId":"01a0","turnId":"01a0"}}"#;
    const DELTA: &str = r#"{"method":"item/agentMessage/delta","params":{"threadId":"01a0","turnId":"01a0","itemId":"msg_08fd","delta":" ok"}}"#;
    const CHANGED: &str = r#"{"method":"item/completed","params":{"item":{"type":"fileChange","id":"exec-114e","changes":[{"path":"/w/src/lib.rs","kind":{"type":"update"},"diff":"-<<<<<<<\n+fn merged() {}\n"},{"path":"/w/NOTES.md","kind":{"type":"add"},"diff":"notes\n"}],"status":"completed"},"threadId":"01a0","turnId":"01a0"}}"#;
    const MESSAGE: &str = r#"{"method":"item/completed","params":{"item":{"type":"agentMessage","id":"msg_08fd","text":"fixture simple ok","phase":"final_answer"},"threadId":"01a0","turnId":"01a0"}}"#;
    const COMPLETED: &str = r#"{"method":"turn/completed","params":{"threadId":"01a0","turn":{"id":"01a0d5eb-f176","items":[],"status":"completed","error":null,"durationMs":4289}}}"#;
    const APPROVAL: &str = r#"{"method":"item/commandExecution/requestApproval","id":0,"params":{"kind":"command","threadId":"01a0","turnId":"01a0","itemId":"exec-27a5","command":"/bin/bash -lc \"printf 'x' > x.txt\"","cwd":"/w","availableDecisions":["accept","decline"]}}"#;
    const INTERRUPTED: &str = r#"{"method":"turn/completed","params":{"threadId":"01a0","turn":{"id":"01a0d5ed-38b3","items":[],"status":"interrupted","error":null}}}"#;

    fn server(mode: RuntimeMode) -> AppServer {
        AppServer::new(&Start { dir: "/w".into(), prompt: "Resolve the conflicts".to_owned(), mode })
    }

    fn replies(said: &[Said]) -> Vec<Value> {
        said.iter().filter_map(|s| if let Said::Reply(l) = s { serde_json::from_str(l).ok() } else { None }).collect()
    }

    #[test]
    fn the_handshake_opens_a_thread_and_starts_a_turn_in_the_folder() {
        let mut s = server(RuntimeMode::ApprovalRequired);
        assert_eq!(replies(&s.open())[0]["method"], "initialize");
        let after_init = replies(&s.line(INITIALIZED));
        assert_eq!(after_init[0]["method"], "initialized");
        assert_eq!(
            (after_init[1]["method"].as_str(), after_init[1]["params"]["cwd"].as_str()),
            (Some("thread/start"), Some("/w"))
        );
        let said = s.line(THREAD);
        assert_eq!(said[0], Said::Event(Event::Started { session: "01a0d5eb-f0c0-7481-a417-5e8871805cbc".to_owned() }));
        let turn = &replies(&said)[0];
        assert_eq!(turn["method"], "turn/start");
        assert_eq!(turn["params"]["input"][0]["text"], "Resolve the conflicts");
        assert_eq!(
            (turn["params"]["approvalPolicy"].as_str(), turn["params"]["sandboxPolicy"]["type"].as_str()),
            (Some("untrusted"), Some("readOnly"))
        );
        s.line(TURN);
        let interrupt = &replies(&s.interrupt())[0];
        assert_eq!(
            (interrupt["method"].as_str(), interrupt["params"]["turnId"].as_str()),
            (Some("turn/interrupt"), Some("01a0d5eb-f176-71d0-8e1c-a459700443de"))
        );
    }

    #[test]
    fn notifications_become_tools_edits_text_and_the_end() {
        let mut s = server(RuntimeMode::FullAccess);
        let said: Vec<Said> =
            [WARNING, COMMAND, DELTA, CHANGED, MESSAGE, COMPLETED].iter().flat_map(|l| s.line(l)).collect();
        assert_eq!(
            said,
            [
                Said::Event(Event::Tool(Tool { name: "command".to_owned(), detail: "git status".to_owned() })),
                Said::Event(Event::Edit(vec![
                    FileEdit { path: "src/lib.rs".to_owned(), change: Change::Modified },
                    FileEdit { path: "NOTES.md".to_owned(), change: Change::Added },
                ])),
                Said::Event(Event::Text("fixture simple ok".to_owned())),
                Said::TurnEnded(Ending::Completed),
            ]
        );
        assert_eq!(s.line(INTERRUPTED), [Said::TurnEnded(Ending::Stopped)]);
    }

    #[test]
    fn approvals_are_declined_unless_the_mode_is_full_access() {
        let mut asks = server(RuntimeMode::ApprovalRequired);
        let said = asks.line(APPROVAL);
        assert_eq!(said[0], Said::Event(Event::Approval { what: "/bin/bash -lc \"printf 'x' > x.txt\"".to_owned() }));
        assert_eq!(replies(&said)[0], json!({ "id": 0, "result": { "decision": "decline" } }));
        let mut free = server(RuntimeMode::FullAccess);
        assert_eq!(replies(&free.line(APPROVAL))[0], json!({ "id": 0, "result": { "decision": "accept" } }));
        let full = free.policy();
        assert_eq!((full.0, full.1["type"].clone()), (json!("never"), json!("dangerFullAccess")));
    }

    #[test]
    fn a_failed_turn_says_why() {
        let mut s = server(RuntimeMode::FullAccess);
        s.line(
            r#"{"method":"error","params":{"turnId":"t","willRetry":true,"error":{"message":"Reconnecting… 1/5"}}}"#,
        );
        s.line(r#"{"method":"error","params":{"turnId":"t","willRetry":false,"error":{"message":"You've hit your usage limit."}}}"#);
        let ended =
            s.line(r#"{"method":"turn/completed","params":{"turn":{"id":"t","status":"failed","error":null}}}"#);
        assert_eq!(ended, [Said::TurnEnded(Ending::Failed("You've hit your usage limit.".to_owned()))]);
        let refused = s.line(r#"{"id":2,"error":{"code":-32603,"message":"cwd does not exist"}}"#);
        assert_eq!(refused, [Said::TurnEnded(Ending::Failed("cwd does not exist".to_owned()))]);
    }
}
