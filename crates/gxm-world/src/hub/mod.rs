//! A fake GitHub for a world: its GraphQL API, served on 127.0.0.1, holding pull requests,
//! reviews, threads and checks that fixtures put there, and recording the reviews the app
//! sends, so a journey can check what reached "GitHub".
//!
//! Only the network edge is fake. The commits a pull request points at are real commits in the
//! world's repositories, and the app reads its diff from git.

mod graphql;
mod server;

use std::sync::{Arc, Mutex, MutexGuard};

use jiff::Timestamp;

use crate::World;

/// The token the world's app presents; any other is refused with a 401.
pub const TOKEN: &str = "gxm-world-token";

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
}

/// The world's GitHub. Stops when the world is dropped.
pub struct Hub {
    state: Arc<Mutex<State>>,
    server: server::Server,
}

impl Hub {
    pub(crate) fn start(env: gxm_core::env::Env, remotes: std::path::PathBuf) -> Hub {
        let now = env.now();
        let state = Arc::new(Mutex::new(State { remotes, env, now, viewer: ANA, repos: Vec::new(), next_id: 1 }));
        let shared = Arc::clone(&state);
        let server = server::Server::start(move |req| {
            if req.authorization.as_deref() != Some(&format!("bearer {TOKEN}")) {
                return (401, r#"{"message":"Bad credentials"}"#.to_owned());
            }
            if req.method != "POST" || req.path != "/graphql" {
                return (404, r#"{"message":"Not Found"}"#.to_owned());
            }
            let mut s = shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            (200, graphql::answer(&mut s, &req.body).to_string())
        })
        .unwrap_or_else(|e| panic!("gxm-world couldn't start its GitHub: {e}"));
        Hub { state, server }
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
