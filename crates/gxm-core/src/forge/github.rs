//! GitHub, through its GraphQL API: one request per question, and a review sent as one
//! pending review that collects every comment and reply, then is submitted.

use jiff::Timestamp;
use serde_json::{Value, json};

use super::model::{
    Check, CheckState, Comment, Event, Mergeable, Pull, PullState, PullSummary, ReviewStatus, Reviewer, Side,
    Submission, Thread, User, Verdict,
};
use super::{Forge, ForgeError, http};
use crate::remote::Provider;

pub struct GitHub {
    endpoint: String,
    token: String,
    owner: String,
    name: String,
}

impl GitHub {
    pub fn new(api: &str, token: String, owner: String, name: String) -> GitHub {
        GitHub { endpoint: format!("{}/graphql", api.trim_end_matches('/')), token, owner, name }
    }

    /// Runs one operation; GraphQL reports failures in `errors`, beside or instead of `data`.
    fn run(&self, operation: &str, query: &str, variables: &Value) -> Result<Value, ForgeError> {
        let body = json!({ "operationName": operation, "query": query, "variables": variables });
        let mut answer = http::post_json(&self.endpoint, &self.token, &body)?;
        if let Some(e) = answer.get("errors").and_then(Value::as_array).and_then(|e| e.first()) {
            return Err(ForgeError::Api(e["message"].as_str().unwrap_or("GitHub refused the request").to_owned()));
        }
        Ok(answer["data"].take())
    }

    fn repo_vars(&self) -> Value {
        json!({ "owner": self.owner, "name": self.name })
    }
}

const SUMMARY: &str = "fragment summary on PullRequest {
  number title state isDraft url headRefName headRefOid baseRefName
  mergeCommit { oid }
  author { login ... on User { name } }
}";

const PERSON: &str = "author { login ... on User { name } }";

impl Forge for GitHub {
    fn provider(&self) -> Provider {
        Provider::GitHub
    }

    fn viewer(&self) -> Result<User, ForgeError> {
        let data = self.run("Viewer", "query Viewer { viewer { login name } }", &json!({}))?;
        user(&data["viewer"])
    }

    fn pulls(&self) -> Result<Vec<PullSummary>, ForgeError> {
        let query = format!(
            "query Pulls($owner: String!, $name: String!) {{
  repository(owner: $owner, name: $name) {{
    open: pullRequests(states: [OPEN], first: 50, orderBy: {{field: UPDATED_AT, direction: DESC}}) {{ nodes {{ ...summary }} }}
    merged: pullRequests(states: [MERGED], first: 30, orderBy: {{field: UPDATED_AT, direction: DESC}}) {{ nodes {{ ...summary }} }}
  }}
}}
{SUMMARY}"
        );
        let data = self.run("Pulls", &query, &self.repo_vars())?;
        let repo = &data["repository"];
        nodes(&repo["open"]).iter().chain(nodes(&repo["merged"]).iter()).map(summary).collect()
    }

    fn pull(&self, number: u64) -> Result<Pull, ForgeError> {
        let query = format!(
            "query Pull($owner: String!, $name: String!, $number: Int!) {{
  repository(owner: $owner, name: $name) {{
    pullRequest(number: $number) {{
      ...summary
      id body createdAt mergeable
      commits {{ totalCount }}
      reviewRequests(first: 20) {{ nodes {{ requestedReviewer {{ ... on User {{ login name }} }} }} }}
      latestReviews(first: 20) {{ nodes {{ {PERSON} state submittedAt }} }}
      reviews(first: 100) {{ nodes {{ {PERSON} state body submittedAt }} }}
      comments(first: 100) {{ nodes {{ {PERSON} body createdAt }} }}
      reviewThreads(first: 100) {{ nodes {{ id isResolved path line diffSide
        comments(first: 100) {{ nodes {{ {PERSON} body createdAt }} }} }} }}
      lastCommit: commits(last: 1) {{ nodes {{ commit {{ statusCheckRollup {{ contexts(first: 100) {{ nodes {{
        __typename
        ... on CheckRun {{ name status conclusion title startedAt completedAt }}
        ... on StatusContext {{ context state description }}
      }} }} }} }} }} }}
    }}
  }}
}}
{SUMMARY}"
        );
        let mut vars = self.repo_vars();
        vars["number"] = json!(number);
        let data = self.run("Pull", &query, &vars)?;
        let pr = &data["repository"]["pullRequest"];
        if pr.is_null() {
            return Err(ForgeError::Api(format!("there's no pull request #{number}")));
        }
        pull(pr)
    }

    fn squash_merge(&self, pull: &Pull, headline: &str) -> Result<(), ForgeError> {
        self.merge(pull, headline)
    }

    fn submit(&self, pull: &Pull, review: &Submission) -> Result<(), ForgeError> {
        let threads: Vec<Value> = review
            .comments
            .iter()
            .map(|c| json!({ "path": c.path, "line": c.line, "side": side_name(c.side), "body": c.body }))
            .collect();
        let added = self.run(
            "AddReview",
            "mutation AddReview($pullRequestId: ID!, $commitOID: GitObjectID!, $threads: [DraftPullRequestReviewThread]) {
  addPullRequestReview(input: {pullRequestId: $pullRequestId, commitOID: $commitOID, threads: $threads}) { pullRequestReview { id } }
}",
            &json!({ "pullRequestId": pull.id, "commitOID": pull.summary.head_sha, "threads": threads }),
        )?;
        let review_id = text(&added["addPullRequestReview"]["pullRequestReview"]["id"])?;
        for reply in &review.replies {
            self.run(
                "ReplyToThread",
                "mutation ReplyToThread($reviewId: ID!, $threadId: ID!, $body: String!) {
  addPullRequestReviewThreadReply(input: {pullRequestReviewId: $reviewId, pullRequestReviewThreadId: $threadId, body: $body}) { comment { id } }
}",
                &json!({ "reviewId": review_id, "threadId": reply.thread, "body": reply.body }),
            )?;
        }
        let event = match review.verdict {
            Verdict::Comment => "COMMENT",
            Verdict::Approve => "APPROVE",
            Verdict::RequestChanges => "REQUEST_CHANGES",
        };
        self.run(
            "SubmitReview",
            "mutation SubmitReview($reviewId: ID!, $event: PullRequestReviewEvent!, $body: String) {
  submitPullRequestReview(input: {pullRequestReviewId: $reviewId, event: $event, body: $body}) { pullRequestReview { id state } }
}",
            &json!({ "reviewId": review_id, "event": event, "body": review.body }),
        )?;
        Ok(())
    }
}

impl GitHub {
    pub(super) fn merge(&self, pull: &Pull, headline: &str) -> Result<(), ForgeError> {
        self.run(
            "MergePull",
            "mutation MergePull($pullRequestId: ID!, $expectedHeadOid: GitObjectID!, $commitHeadline: String) {
  mergePullRequest(input: {pullRequestId: $pullRequestId, mergeMethod: SQUASH, expectedHeadOid: $expectedHeadOid, commitHeadline: $commitHeadline}) { pullRequest { state } }
}",
            &json!({ "pullRequestId": pull.id, "expectedHeadOid": pull.summary.head_sha, "commitHeadline": headline }),
        )?;
        Ok(())
    }
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Old => "LEFT",
        Side::New => "RIGHT",
    }
}

fn bad(what: &str) -> ForgeError {
    ForgeError::Parse(format!("no {what}"))
}

fn nodes(v: &Value) -> Vec<Value> {
    v["nodes"].as_array().cloned().unwrap_or_default()
}

fn text(v: &Value) -> Result<String, ForgeError> {
    v.as_str().map(str::to_owned).ok_or_else(|| bad("text"))
}

fn time(v: &Value) -> Result<Timestamp, ForgeError> {
    v.as_str().and_then(|s| s.parse().ok()).ok_or_else(|| bad("time"))
}

/// A user, or GitHub's stand-in for a deleted account.
fn user(v: &Value) -> Result<User, ForgeError> {
    if v.is_null() {
        return Ok(User { login: "ghost".to_owned(), name: "Deleted user".to_owned() });
    }
    let login = text(&v["login"])?;
    let name = v["name"].as_str().filter(|n| !n.is_empty()).unwrap_or(&login).to_owned();
    Ok(User { login, name })
}

fn summary(v: &Value) -> Result<PullSummary, ForgeError> {
    let state = match (v["state"].as_str(), v["isDraft"].as_bool()) {
        (Some("MERGED"), _) => PullState::Merged,
        (Some("CLOSED"), _) => PullState::Closed,
        (_, Some(true)) => PullState::Draft,
        _ => PullState::Open,
    };
    Ok(PullSummary {
        number: v["number"].as_u64().ok_or_else(|| bad("number"))?,
        title: text(&v["title"])?,
        state,
        author: user(&v["author"])?,
        head_ref: text(&v["headRefName"])?,
        head_sha: text(&v["headRefOid"])?,
        base_ref: text(&v["baseRefName"])?,
        merge_commit: v["mergeCommit"]["oid"].as_str().map(str::to_owned),
        url: text(&v["url"])?,
    })
}

fn status(state: &str) -> Option<ReviewStatus> {
    Some(match state {
        "APPROVED" => ReviewStatus::Approved,
        "CHANGES_REQUESTED" => ReviewStatus::ChangesRequested,
        "COMMENTED" => ReviewStatus::Commented,
        // Pending reviews are drafts; dismissed ones no longer count.
        _ => return None,
    })
}

fn pull(v: &Value) -> Result<Pull, ForgeError> {
    let author = user(&v["author"])?;
    let mut reviewers = Vec::new();
    for r in nodes(&v["latestReviews"]) {
        // The author answering a thread files a review too; they aren't their own reviewer.
        if let Some(status) = r["state"].as_str().and_then(status)
            && let who = user(&r["author"])?
            && who.login != author.login
        {
            reviewers.push(Reviewer { user: who, status });
        }
    }
    for r in nodes(&v["reviewRequests"]) {
        let who = &r["requestedReviewer"];
        // Teams have no login here; they're left out until teams are designed.
        if who["login"].is_string() {
            let u = user(who)?;
            if !reviewers.iter().any(|r| r.user.login == u.login) {
                reviewers.push(Reviewer { user: u, status: ReviewStatus::Requested });
            }
        }
    }
    let mut conversation = Vec::new();
    for r in nodes(&v["reviews"]) {
        let Some(status) = r["state"].as_str().and_then(status) else { continue };
        let body = r["body"].as_str().unwrap_or_default().to_owned();
        conversation.push(Event::Review { author: user(&r["author"])?, status, body, at: time(&r["submittedAt"])? });
    }
    for c in nodes(&v["comments"]) {
        conversation.push(Event::Comment(comment(&c)?));
    }
    conversation.sort_by_key(Event::at);
    let threads = nodes(&v["reviewThreads"])
        .iter()
        .map(|t| {
            Ok(Thread {
                id: text(&t["id"])?,
                path: text(&t["path"])?,
                line: t["line"].as_u64().and_then(|l| u32::try_from(l).ok()),
                side: if t["diffSide"].as_str() == Some("LEFT") { Side::Old } else { Side::New },
                resolved: t["isResolved"].as_bool().unwrap_or(false),
                comments: nodes(&t["comments"]).iter().map(comment).collect::<Result<_, _>>()?,
            })
        })
        .collect::<Result<Vec<_>, ForgeError>>()?;
    let contexts = nodes(&v["lastCommit"])
        .first()
        .map(|n| nodes(&n["commit"]["statusCheckRollup"]["contexts"]))
        .unwrap_or_default();
    Ok(Pull {
        summary: summary(v)?,
        id: text(&v["id"])?,
        body: v["body"].as_str().unwrap_or_default().to_owned(),
        created: time(&v["createdAt"])?,
        commits: v["commits"]["totalCount"].as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or(0),
        mergeable: match v["mergeable"].as_str() {
            Some("MERGEABLE") => Mergeable::Clean,
            Some("CONFLICTING") => Mergeable::Conflicts,
            _ => Mergeable::Unknown,
        },
        reviewers,
        checks: contexts.iter().map(check).collect(),
        threads,
        conversation,
    })
}

fn comment(v: &Value) -> Result<Comment, ForgeError> {
    Ok(Comment { author: user(&v["author"])?, body: text(&v["body"])?, at: time(&v["createdAt"])? })
}

/// A check run (GitHub Actions and apps) or a commit status (older integrations).
fn check(v: &Value) -> Check {
    if v["__typename"].as_str() == Some("StatusContext") {
        let state = match v["state"].as_str() {
            Some("SUCCESS") => CheckState::Passed,
            Some("FAILURE" | "ERROR") => CheckState::Failed,
            _ => CheckState::Pending,
        };
        let name = v["context"].as_str().unwrap_or("status").to_owned();
        return Check { name, state, title: v["description"].as_str().unwrap_or_default().to_owned(), seconds: None };
    }
    let state = match (v["status"].as_str(), v["conclusion"].as_str()) {
        (Some("COMPLETED"), Some("SUCCESS" | "NEUTRAL")) => CheckState::Passed,
        (Some("COMPLETED"), Some("SKIPPED")) => CheckState::Skipped,
        (Some("COMPLETED"), _) => CheckState::Failed,
        _ => CheckState::Pending,
    };
    let seconds = match (time(&v["startedAt"]), time(&v["completedAt"])) {
        (Ok(a), Ok(b)) => u32::try_from(b.as_second() - a.as_second()).ok(),
        _ => None,
    };
    Check {
        name: v["name"].as_str().unwrap_or("check").to_owned(),
        state,
        title: v["title"].as_str().unwrap_or_default().to_owned(),
        seconds,
    }
}
