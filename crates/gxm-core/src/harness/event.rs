//! What a session says, the same for every harness: each driver turns its own stream (Claude
//! Code's stream-json, Codex's JSON-RPC notifications) into these, as T3 Code normalizes its
//! providers' output into one runtime event stream.

/// One thing the agent did or said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The harness is up, in the session it named.
    Started { session: String },
    /// Text the agent wrote to the person.
    Text(String),
    /// The agent used a tool: ran a command, searched, read a file, called an MCP server.
    Tool(Tool),
    /// The agent changed files.
    Edit(Vec<FileEdit>),
    /// The harness wants approval for something the runtime mode doesn't allow by itself. The
    /// session declines it (see [`RuntimeMode`](super::RuntimeMode)).
    Approval { what: String },
    /// A problem the harness reported that didn't end the session (a retried request).
    Warning(String),
    /// The session is over. Always the last event.
    Ended(Ending),
}

/// A tool call, as a person reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tool {
    /// What kind: `Bash`, `Read`, `command`, an MCP tool's name.
    pub name: String,
    /// What it was asked to do, on one line: the command, the file, the pattern.
    pub detail: String,
}

/// A file the agent changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEdit {
    pub path: String,
    pub change: Change,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Added,
    Modified,
    Deleted,
}

impl Change {
    pub fn word(self) -> &'static str {
        match self {
            Change::Added => "added",
            Change::Modified => "edited",
            Change::Deleted => "deleted",
        }
    }
}

/// How a session ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ending {
    /// The agent finished its turn.
    Completed,
    /// The person stopped it.
    Stopped,
    /// It failed, and why.
    Failed(String),
}

impl Event {
    /// The event on one line, as `gxm agents run` prints it.
    pub fn line(&self) -> String {
        match self {
            Event::Started { session } => format!("● started session {session}"),
            Event::Text(t) => t.trim_end().to_owned(),
            Event::Tool(t) if t.detail.is_empty() => format!("→ {}", t.name),
            Event::Tool(t) => format!("→ {}: {}", t.name, t.detail),
            Event::Edit(files) => {
                let each: Vec<String> = files.iter().map(|f| format!("{} {}", f.change.word(), f.path)).collect();
                format!("✎ {}", each.join(", "))
            }
            Event::Approval { what } => format!("✋ declined (needs approval): {what}"),
            Event::Warning(w) => format!("! {w}"),
            Event::Ended(Ending::Completed) => "✓ done".to_owned(),
            Event::Ended(Ending::Stopped) => "■ stopped".to_owned(),
            Event::Ended(Ending::Failed(why)) => format!("✗ failed: {why}"),
        }
    }
}
