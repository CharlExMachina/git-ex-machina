//! The harness drivers against the world's `claude` and `codex`, which speak the real wire
//! formats: finding them, reading who's signed in, running a prompt to the end, and stopping one.

use std::time::Duration;

use gxm_core::harness::{self, Auth, Ending, Event, Harness, RuntimeMode, Session, Start};
use gxm_world::World;
use gxm_world::fixtures::agents::{self, AGENT_EMAIL, Agent};

/// Every event until the session ends.
fn drain(s: &Session) -> Vec<Event> {
    let mut out = Vec::new();
    while let Some(e) = s.next_event(Duration::from_secs(10)) {
        let end = matches!(e, Event::Ended(_));
        out.push(e);
        if end {
            break;
        }
    }
    out
}

fn start(w: &World, h: Harness) -> Session {
    let dir = w.code().join("site");
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("make site: {e}"));
    std::fs::write(dir.join("README.md"), "# site\n").unwrap_or_else(|e| panic!("write README: {e}"));
    let start = Start { dir, prompt: "Resolve the conflicts".to_owned(), mode: RuntimeMode::ApprovalRequired };
    harness::start(w.env(), h, &start, Box::new(|| {})).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn harnesses_are_found_asked_who_is_signed_in_and_chosen() {
    let w = World::new();
    let none = harness::available(w.env());
    assert!(none.iter().all(|s| !s.installed()), "{none:?}");
    assert_eq!(
        none[0].problem.as_deref(),
        Some(
            "Claude Code (`claude`) was not found on PATH. Install it with `npm install -g @anthropic-ai/claude-code`, then sign in with `claude auth login`."
        )
    );
    assert_eq!(harness::preferred(w.env()), None);

    agents::claude(&w, false, Agent::Works);
    agents::codex(&w, false);
    let out = harness::available(w.env());
    assert_eq!((out[0].version.as_deref(), &out[0].auth), (Some(agents::CLAUDE_VERSION), &Auth::SignedOut));
    assert_eq!(
        out[0].problem.as_deref(),
        Some("Claude Code is not authenticated. Run `claude auth login` and try again.")
    );
    assert_eq!((out[1].version.as_deref(), &out[1].auth), (Some(agents::CODEX_VERSION), &Auth::SignedOut));
    assert_eq!(out[1].problem.as_deref(), Some("Codex CLI is not authenticated. Run `codex login` and try again."));
    assert_eq!(harness::preferred(w.env()), None, "nothing signed in is usable");

    agents::claude(&w, true, Agent::Works);
    agents::codex(&w, true);
    let both = harness::available(w.env());
    assert!(both.iter().all(harness::HarnessStatus::usable), "{both:?}");
    assert_eq!(both[0].auth, Auth::SignedIn(Some(AGENT_EMAIL.to_owned())));
    assert_eq!(harness::preferred(w.env()), Some(Harness::ClaudeCode), "the first usable, until one is chosen");
    harness::prefer(w.env(), Harness::Codex).unwrap();
    assert_eq!(harness::preferred(w.env()), Some(Harness::Codex));
}

#[test]
fn a_prompt_runs_to_the_end_in_its_folder_with_either_harness() {
    let w = World::new();
    agents::claude(&w, true, Agent::Works);
    agents::codex(&w, true);

    let claude = drain(&start(&w, Harness::ClaudeCode));
    let lines: Vec<String> = claude.iter().map(Event::line).collect();
    assert_eq!(
        lines,
        [
            "● started session 7d295979-f2e5-43cb-a9ee-b05f140f3ed8",
            "I'll look at the README first.",
            "→ Read: README.md",
            "✎ edited README.md",
            "Done: README.md says how the conflict was resolved.",
            "✓ done",
        ]
    );
    let readme = std::fs::read_to_string(w.code().join("site/README.md")).unwrap();
    assert_eq!(readme, "# site\nResolved by Claude.\n", "it worked in the folder it was given");

    let codex = drain(&start(&w, Harness::Codex));
    let lines: Vec<String> = codex.iter().map(Event::line).collect();
    assert_eq!(
        lines,
        [
            "● started session 01a0d5eb-f0c0-7481-a417-5e8871805cbc",
            "→ command: git status",
            "✎ edited README.md",
            "Resolved: README.md says so.",
            "✓ done",
        ]
    );
}

#[test]
fn a_session_the_person_stops_ends_stopped() {
    let w = World::new();
    agents::claude(&w, true, Agent::Lingers);
    let s = start(&w, Harness::ClaudeCode);
    assert!(matches!(s.next_event(Duration::from_secs(10)), Some(Event::Started { .. })));
    assert_eq!(s.next_event(Duration::from_secs(10)), Some(Event::Text("Looking at the conflicts…".to_owned())));
    assert_eq!(s.try_event(), None, "still working");
    s.stop();
    assert_eq!(drain(&s), [Event::Ended(Ending::Stopped)]);
}
