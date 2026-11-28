//! A fake GitHub for a world: its GraphQL API, served on 127.0.0.1, holding pull requests,
//! reviews, threads and checks that fixtures put there, and recording the reviews the app
//! sends, so a journey can check what reached "GitHub". It also publishes git-ex-machina's own
//! releases, for the app to update itself from ([`releases`]).
//!
//! Only the network edge is fake. The commits a pull request points at are real commits in the
//! world's repositories, and the app reads its diff from git.

mod actions;
pub mod admin;
mod graphql;
mod notes;
mod releases;
mod rest;
mod rest_more;
mod server;

use std::sync::{Arc, Mutex, MutexGuard};

use jiff::Timestamp;

use crate::World;

/// The token the world's app presents; any other is refused with a 401. A world starts signed
/// in with it, kept as a sign-in keeps it; a sign-in through the hub hands it out again.
pub const TOKEN: &str = "gxm-world-token";
/// The token a sign-in that asked for `delete_repo` too hands out: the one that may delete a
/// repository.
pub const TOKEN_DELETE: &str = "gxm-world-token-delete";
/// The one-time code every sign-in shows.
pub const USER_CODE: &str = "WDJB-MJHT";
/// How many times the app asks (a second apart) before the person has answered on
/// "github.com": long enough for a replay, in real time, to copy the code while it shows.
const POLLS_BEFORE_ANSWER: u32 = 6;

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
    /// Opened as a draft, not yet ready for review.
    pub draft: bool,
    pub author: Person,
    pub head_ref: String,
    pub head_sha: String,
    pub base_ref: String,
    pub merge_commit: Option<String>,
    pub created: Timestamp,
    pub commits: u32,
    /// Who's asked to review it, and when they were last asked.
    pub requested: Vec<Request>,
    pub reviews: Vec<ReviewData>,
    pub comments: Vec<CommentData>,
    pub threads: Vec<ThreadData>,
    pub checks: Vec<CheckData>,
}

/// A review asked of someone.
#[derive(Clone, Debug)]
pub struct Request {
    pub person: Person,
    pub at: Timestamp,
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
    /// Asked to run again and not finished yet: GitHub shows it queued.
    pub queued: bool,
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
    /// The sign-in under way: its answer, how many more asks see "pending", and whether it
    /// asked for `delete_repo`.
    signing_in: Option<(Answer, u32, bool)>,
    /// Each repository's settings, made when first asked for (see [`admin`]).
    settings: Vec<((String, String), admin::Settings)>,
    /// Repositories renamed: their old name and the new one.
    renamed: Vec<((String, String), String)>,
    /// git-ex-machina's releases, oldest first.
    releases: Vec<releases::ReleaseData>,
    /// How many of the coming downloads arrive damaged.
    damaged: u32,
    /// The API root, for the download links a release hands out.
    url: String,
    /// The checks asked to run again, in order: `owner/name`, the pull request, the check.
    reruns: Vec<(String, u64, String)>,
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
            settings: Vec::new(),
            renamed: Vec::new(),
            releases: Vec::new(),
            damaged: 0,
            url: String::new(),
            reruns: Vec::new(),
        }));
        let shared = Arc::clone(&state);
        let server = server::Server::start(move |req| {
            // Signing in, and a public repository's releases, are done without a token.
            if req.method == "POST" && req.path.starts_with("/login/") {
                let mut s = shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                return sign_in(&mut s, &req.path, &req.body).into();
            }
            // Releases are public, as GitHub's are: no token.
            if req.method == "GET" && (req.path.starts_with("/releases/") || req.path.ends_with("/releases/latest")) {
                let mut s = shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                return releases::answer(&mut s, &req.path);
            }
            let token = req.authorization.as_deref().and_then(|a| a.strip_prefix("bearer "));
            if token != Some(TOKEN) && token != Some(TOKEN_DELETE) {
                return (401, r#"{"message":"Bad credentials"}"#.to_owned()).into();
            }
            let mut s = shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if req.method == "POST" && req.path == "/graphql" {
                return (200, graphql::answer(&mut s, &req.body).to_string()).into();
            }
            if let Some(answer) = actions::answer(&mut s, &req.method, &req.path) {
                return answer.into();
            }
            rest::answer(&mut s, &req.method, &req.path, &req.body, token == Some(TOKEN_DELETE)).into()
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

    /// Publishes git-ex-machina `version` with these notes, carrying `bytes` as this machine's
    /// build, named `asset` (see `gxm_core::update::asset_name`).
    pub fn publish(&self, version: &str, notes: &str, asset: &str, bytes: Vec<u8>) {
        self.lock().releases.push(releases::ReleaseData {
            version: version.to_owned(),
            notes: notes.to_owned(),
            asset: asset.to_owned(),
            bytes,
        });
    }

    /// The next `n` downloads of a build arrive damaged: their bytes don't match the digest.
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

    /// Who's signed in to the world's GitHub from now on (Ana unless a fixture says otherwise).
    pub fn set_viewer(&self, viewer: Person) {
        self.lock().viewer = viewer;
    }

    /// `owner/name`'s settings as they stand, once anything has read or set them.
    pub fn settings(&self, owner: &str, name: &str) -> Option<admin::Settings> {
        self.lock().settings_mut(owner, name).map(|s| s.clone())
    }

    /// Changes `owner/name`'s settings, as a fixture sets a repository up.
    pub fn edit_settings(&self, owner: &str, name: &str, edit: impl FnOnce(&mut admin::Settings)) {
        let mut s = self.lock();
        match s.settings_mut(owner, name) {
            Some(settings) => edit(settings),
            None => panic!("the world has no repository {owner}/{name} to set up"),
        }
    }

    /// Whether a pull request has been merged, and the commit merging made.
    pub fn merged(&self, owner: &str, name: &str, number: u64) -> Option<String> {
        let s = self.lock();
        graphql::find(&s, owner, name, number).filter(|p| p.merged).and_then(|p| p.merge_commit.clone())
    }

    /// Asks `person` to review a pull request at `at`, again if they were asked before.
    pub fn request_review(&self, owner: &str, name: &str, number: u64, person: Person, at: Timestamp) {
        let mut s = self.lock();
        let p = graphql::find_mut(&mut s, owner, name, number)
            .unwrap_or_else(|| panic!("the world's GitHub has no {owner}/{name}#{number}"));
        p.requested.retain(|r| r.person != person);
        p.requested.push(Request { person, at });
    }

    /// Points a pull request at a new head commit, as a push to its branch does.
    pub fn moved(&self, owner: &str, name: &str, number: u64, head_sha: &str) {
        let mut s = self.lock();
        let p = graphql::find_mut(&mut s, owner, name, number)
            .unwrap_or_else(|| panic!("the world's GitHub has no {owner}/{name}#{number}"));
        head_sha.clone_into(&mut p.head_sha);
    }

    /// The checks asked to run again on a pull request, in order, by name.
    pub fn reruns(&self, owner: &str, name: &str, number: u64) -> Vec<String> {
        let repo = format!("{owner}/{name}");
        self.lock().reruns.iter().filter(|(r, n, _)| *r == repo && *n == number).map(|(_, _, c)| c.clone()).collect()
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
fn sign_in(s: &mut State, path: &str, form: &str) -> (u16, String) {
    match path {
        "/login/device/code" => {
            let answer = s.sign_ins.pop_front().unwrap_or(Answer::Approve);
            let delete = form.split('&').any(|f| f.starts_with("scope=") && f.contains("delete_repo"));
            s.signing_in = Some((answer, POLLS_BEFORE_ANSWER, delete));
            let body = format!(
                r#"{{"device_code":"device-1","user_code":"{USER_CODE}","verification_uri":"https://github.com/login/device","expires_in":900,"interval":1}}"#
            );
            (200, body)
        }
        "/login/oauth/access_token" => match &mut s.signing_in {
            Some((_, polls, _)) if *polls > 0 => {
                *polls -= 1;
                (200, r#"{"error":"authorization_pending"}"#.to_owned())
            }
            Some((Answer::Approve, _, delete)) => {
                let (token, scope) =
                    if *delete { (TOKEN_DELETE, "repo,read:org,delete_repo") } else { (TOKEN, "repo,read:org") };
                s.signing_in = None;
                (200, format!(r#"{{"access_token":"{token}","token_type":"bearer","scope":"{scope}"}}"#))
            }
            Some((Answer::Decline, _, _)) => {
                s.signing_in = None;
                (200, r#"{"error":"access_denied"}"#.to_owned())
            }
            None => (200, r#"{"error":"expired_token"}"#.to_owned()),
        },
        _ => (404, r#"{"message":"Not Found"}"#.to_owned()),
    }
}

/// The ids of the people the world's GitHub knows; anyone else gets one from their login.
const PEOPLE: &[(&str, u64)] = &[("ana-ruiz", 1), ("kenji-mori", 2), ("lea-brandt", 3), ("jo-park", 4), ("sam-lee", 5)];

/// A login's id on the world's GitHub.
pub(crate) fn user_id(login: &str) -> u64 {
    PEOPLE
        .iter()
        .find(|(l, _)| *l == login)
        .map_or_else(|| 100 + login.bytes().map(u64::from).sum::<u64>(), |(_, id)| *id)
}

/// The login an id belongs to, for the people the world knows.
pub(crate) fn login_of(id: u64) -> Option<String> {
    PEOPLE.iter().find(|(_, i)| *i == id).map(|(l, _)| (*l).to_owned())
}
