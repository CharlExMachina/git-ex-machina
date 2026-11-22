//! A fake GitHub for a world: its GraphQL API, served on 127.0.0.1, holding pull requests,
//! reviews, threads and checks that fixtures put there, and recording the reviews the app
//! sends, so a journey can check what reached "GitHub". It also publishes git-ex-machina's own
//! releases, for the app to update itself from ([`releases`]).
//!
//! Only the network edge is fake. The commits a pull request points at are real commits in the
//! world's repositories, and the app reads its diff from git.

mod graphql;
mod releases;
mod server;

use std::sync::{Arc, Mutex, MutexGuard};

use jiff::Timestamp;

use crate::World;

/// The token the world's app presents; any other is refused with a 401. A world starts signed
/// in with it, kept as a sign-in keeps it; a sign-in through the hub hands it out again.
pub const TOKEN: &str = "gxm-world-token";
/// The one-time code every sign-in shows.
pub const USER_CODE: &str = "WDJB-MJHT";
/// How many times the app asks before the person has answered on "github.com".
const POLLS_BEFORE_ANSWER: u32 = 2;

/// What the person does on github.com with a sign-in code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Approve,
    Decline,
}

/// A person on the fake GitHub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    pub login: &'static str,
    pub name: &'static str,
}

pub const ANA: Person = Person { login: "ana-ruiz", name: "Ana Ruiz" };
pub const KENJI: Person = Person { login: "kenji-mori", name: "Kenji Mori" };
pub const LEA: Person = Person { login: "lea-brandt", name: "Lea Brandt" };

/// A pull request as the fake holds it.
#[derive(Clone, Debug)]
pub struct PullData {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub merged: bool,
    pub author: Person,
    pub head_ref: String,
    pub head_sha: String,
    pub base_ref: String,
    pub merge_commit: Option<String>,
    pub created: Timestamp,
    pub commits: u32,
    pub requested: Vec<Person>,
    pub reviews: Vec<ReviewData>,
    pub comments: Vec<CommentData>,
    pub threads: Vec<ThreadData>,
    pub checks: Vec<CheckData>,
}

#[derive(Clone, Debug)]
pub struct ReviewData {
    pub id: String,
    pub author: Person,
    /// None while the review is pending (being written, not yet submitted).
    pub verdict: Option<Verdict>,
    pub body: String,
    pub at: Timestamp,
    /// What the app sent with it, for journeys to check.
    pub sent: Vec<SentComment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Comment,
    Approve,
    RequestChanges,
}

#[derive(Clone, Debug)]
pub struct CommentData {
    pub author: Person,
    pub body: String,
    pub at: Timestamp,
    /// The review a thread comment belongs to; a pending review's comments aren't public yet.
    pub review: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ThreadData {
    pub id: String,
    pub path: String,
    pub line: u32,
    /// `LEFT` or `RIGHT`, as GitHub names a diff's sides.
    pub side: &'static str,
    pub resolved: bool,
    pub comments: Vec<CommentData>,
}

#[derive(Clone, Debug)]
pub struct CheckData {
    pub name: String,
    pub passed: bool,
    pub title: String,
    pub seconds: i64,
}

/// A comment a review carried: a new thread on a line, or a reply to `thread`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentComment {
    pub path: String,
    pub line: Option<u32>,
    pub body: String,
    pub thread: Option<String>,
}

/// A submitted review, for checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentReview {
    pub author: String,
    pub verdict: Verdict,
    pub body: String,
    pub comments: Vec<SentComment>,
}

struct State {
    /// Where the world's remotes are, and git's environment there: merging happens in them.
    remotes: std::path::PathBuf,
    env: gxm_core::env::Env,
    now: Timestamp,
    viewer: Person,
    repos: Vec<(String, String, Vec<PullData>)>,
    next_id: u32,
    /// How the coming sign-ins go, in order; any past these are approved.
    sign_ins: std::collections::VecDeque<Answer>,
    /// The sign-in under way: its answer, and how many more asks see "pending".
    signing_in: Option<(Answer, u32)>,
    /// git-ex-machina's releases, oldest first.
    releases: Vec<releases::ReleaseData>,
    /// How many of the coming downloads arrive damaged.
    damaged: u32,
    /// The API root, for the download links a release hands out.
    url: String,
}

/// The world's GitHub. Stops when the world is dropped.
pub struct Hub {
    state: Arc<Mutex<State>>,
    server: server::Server,
}

impl Hub {
    pub(crate) fn start(env: gxm_core::env::Env, remotes: std::path::PathBuf) -> Hub {
        let now = env.now();
        let state = Arc::new(Mutex::new(State {
            remotes,
            env,
            now,
            viewer: ANA,
            repos: Vec::new(),
            next_id: 1,
            sign_ins: std::collections::VecDeque::new(),
            signing_in: None,
            releases: Vec::new(),
            damaged: 0,
            url: String::new(),
        }));
        let shared = Arc::clone(&state);
        let server = server::Server::start(move |req| {
            // Signing in, and a public repository's releases, are done without a token.
            if req.method == "POST" && req.path.starts_with("/login/") {
                let mut s = shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                return sign_in(&mut s, &req.path).into();
            }
            if req.method == "GET" {
                let mut s = shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                return releases::answer(&mut s, &req.path);
            }
            if req.authorization.as_deref() != Some(&format!("bearer {TOKEN}")) {
                return (401, r#"{"message":"Bad credentials"}"#.to_owned()).into();
            }
            if req.method != "POST" || req.path != "/graphql" {
                return (404, r#"{"message":"Not Found"}"#.to_owned()).into();
            }
            let mut s = shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            (200, graphql::answer(&mut s, &req.body).to_string()).into()
        })
        .unwrap_or_else(|e| panic!("gxm-world couldn't start its GitHub: {e}"));
        let hub = Hub { state, server };
        let url = hub.url();
        hub.lock().url = url;
        hub
    }

    /// The hub of `w`.
    pub fn of(w: &World) -> &Hub {
        w.hub()
    }

    /// The API root the app talks to.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.server.port)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// How the next sign-ins go, in order.
    pub fn answer_sign_ins(&self, answers: &[Answer]) {
        self.lock().sign_ins.extend(answers);
    }

    /// Publishes git-ex-machina `version` with these notes, carrying `appimage` as this
    /// machine's AppImage.
    pub fn publish(&self, version: &str, notes: &str, appimage: &str) {
        self.lock().releases.push(releases::ReleaseData {
            version: version.to_owned(),
            notes: notes.to_owned(),
            appimage: appimage.to_owned(),
        });
    }

    /// The next `n` downloads of an AppImage arrive damaged: their bytes don't match the digest.
    pub fn damage_downloads(&self, n: u32) {
        self.lock().damaged = n;
    }

    /// Adds a pull request to `owner/name`.
    pub fn add_pull(&self, owner: &str, name: &str, pull: PullData) {
        let mut s = self.lock();
        match s.repos.iter_mut().find(|(o, n, _)| o == owner && n == name) {
            Some((_, _, pulls)) => pulls.push(pull),
            None => s.repos.push((owner.to_owned(), name.to_owned(), vec![pull])),
        }
    }

    /// Whether a pull request has been merged, and the commit merging made.
    pub fn merged(&self, owner: &str, name: &str, number: u64) -> Option<String> {
        let s = self.lock();
        graphql::find(&s, owner, name, number).filter(|p| p.merged).and_then(|p| p.merge_commit.clone())
    }

    /// The reviews submitted on a pull request, oldest first.
    pub fn reviews(&self, owner: &str, name: &str, number: u64) -> Vec<SentReview> {
        let s = self.lock();
        let Some(p) = graphql::find(&s, owner, name, number) else { return Vec::new() };
        p.reviews
            .iter()
            .filter_map(|r| {
                Some(SentReview {
                    author: r.author.login.to_owned(),
                    verdict: r.verdict?,
                    body: r.body.clone(),
                    comments: r.sent.clone(),
                })
            })
            .collect()
    }
}

/// The device flow's two endpoints: a code, then "pending" until the scripted answer.
fn sign_in(s: &mut State, path: &str) -> (u16, String) {
    match path {
        "/login/device/code" => {
            let answer = s.sign_ins.pop_front().unwrap_or(Answer::Approve);
            s.signing_in = Some((answer, POLLS_BEFORE_ANSWER));
            let body = format!(
                r#"{{"device_code":"device-1","user_code":"{USER_CODE}","verification_uri":"https://github.com/login/device","expires_in":900,"interval":1}}"#
            );
            (200, body)
        }
        "/login/oauth/access_token" => match &mut s.signing_in {
            Some((_, polls)) if *polls > 0 => {
                *polls -= 1;
                (200, r#"{"error":"authorization_pending"}"#.to_owned())
            }
            Some((Answer::Approve, _)) => {
                s.signing_in = None;
                (200, format!(r#"{{"access_token":"{TOKEN}","token_type":"bearer","scope":"repo,read:org"}}"#))
            }
            Some((Answer::Decline, _)) => {
                s.signing_in = None;
                (200, r#"{"error":"access_denied"}"#.to_owned())
            }
            None => (200, r#"{"error":"expired_token"}"#.to_owned()),
        },
        _ => (404, r#"{"message":"Not Found"}"#.to_owned()),
    }
}
