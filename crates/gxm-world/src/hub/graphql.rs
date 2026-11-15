//! The fake's GraphQL answers, by operation name: the queries and mutations the app's GitHub
//! client sends, answered in GitHub's shapes from the hub's state.

use serde_json::{Value, json};

use super::{CommentData, Person, PullData, ReviewData, SentComment, State, ThreadData, Verdict};

pub(super) fn find<'a>(s: &'a State, owner: &str, name: &str, number: u64) -> Option<&'a PullData> {
    s.repos.iter().find(|(o, n, _)| o == owner && n == name)?.2.iter().find(|p| p.number == number)
}

fn error(message: &str) -> Value {
    json!({ "errors": [{ "message": message }] })
}

pub(super) fn answer(s: &mut State, body: &str) -> Value {
    let Ok(req) = serde_json::from_str::<Value>(body) else { return error("Problems parsing JSON") };
    let vars = &req["variables"];
    let str_var = |k: &str| vars[k].as_str().unwrap_or_default().to_owned();
    match req["operationName"].as_str().unwrap_or_default() {
        "Viewer" => json!({ "data": { "viewer": person(&s.viewer) } }),
        "Pulls" => {
            let Some((_, _, pulls)) = s.repos.iter().find(|(o, n, _)| *o == str_var("owner") && *n == str_var("name"))
            else {
                return error("Could not resolve to a Repository");
            };
            let (owner, name) = (str_var("owner"), str_var("name"));
            let open: Vec<Value> = pulls.iter().filter(|p| !p.merged).map(|p| summary(&owner, &name, p)).collect();
            let merged: Vec<Value> = pulls.iter().filter(|p| p.merged).map(|p| summary(&owner, &name, p)).collect();
            json!({ "data": { "repository": { "open": { "nodes": open }, "merged": { "nodes": merged } } } })
        }
        "Pull" => {
            let (owner, name) = (str_var("owner"), str_var("name"));
            match find(s, &owner, &name, vars["number"].as_u64().unwrap_or(0)) {
                Some(p) => json!({ "data": { "repository": { "pullRequest": pull(&owner, &name, p) } } }),
                None => json!({ "data": { "repository": { "pullRequest": null } } }),
            }
        }
        "AddReview" => add_review(s, vars),
        "ReplyToThread" => reply(s, vars),
        "SubmitReview" => submit(s, vars),
        "MergePull" => merge(s, vars),
        other => error(&format!("the world's GitHub doesn't answer {other:?}")),
    }
}

fn person(p: &Person) -> Value {
    json!({ "login": p.login, "name": p.name })
}

/// A pull request's id: `PR_<owner>_<name>_<number>`.
fn node_id(owner: &str, name: &str, number: u64) -> String {
    format!("PR_{owner}_{name}_{number}")
}

fn pull_mut<'a>(s: &'a mut State, id: &str) -> Option<&'a mut PullData> {
    s.repos
        .iter_mut()
        .flat_map(|(o, n, pulls)| pulls.iter_mut().map(move |p| (o.clone(), n.clone(), p)))
        .find_map(|(o, n, p)| (node_id(&o, &n, p.number) == id).then_some(p))
}

fn summary(owner: &str, name: &str, p: &PullData) -> Value {
    json!({
        "number": p.number,
        "title": p.title,
        "state": if p.merged { "MERGED" } else { "OPEN" },
        "isDraft": false,
        "url": format!("https://github.com/{owner}/{name}/pull/{}", p.number),
        "headRefName": p.head_ref,
        "headRefOid": p.head_sha,
        "baseRefName": p.base_ref,
        "mergeCommit": p.merge_commit.as_ref().map(|oid| json!({ "oid": oid })),
        "author": person(&p.author),
    })
}

fn verdict_state(v: Verdict) -> &'static str {
    match v {
        Verdict::Comment => "COMMENTED",
        Verdict::Approve => "APPROVED",
        Verdict::RequestChanges => "CHANGES_REQUESTED",
    }
}

/// A comment is public once it isn't part of a pending review.
fn public(p: &PullData, c: &CommentData) -> bool {
    c.review.as_ref().is_none_or(|id| p.reviews.iter().any(|r| r.id == *id && r.verdict.is_some()))
}

fn pull(owner: &str, name: &str, p: &PullData) -> Value {
    let mut v = summary(owner, name, p);
    let submitted: Vec<&ReviewData> = p.reviews.iter().filter(|r| r.verdict.is_some()).collect();
    // Each reviewer's standing is their latest review, except that a comment-only review
    // doesn't replace an approval or a request for changes, as on GitHub.
    let mut latest: Vec<&ReviewData> = Vec::new();
    for r in &submitted {
        let decided = |l: &&ReviewData| l.author == r.author && l.verdict != Some(Verdict::Comment);
        if r.verdict == Some(Verdict::Comment) && latest.iter().any(decided) {
            continue;
        }
        latest.retain(|l| l.author != r.author);
        latest.push(r);
    }
    let review_json = |r: &ReviewData| {
        json!({
            "author": person(&r.author),
            "state": r.verdict.map_or("PENDING", verdict_state),
            "body": r.body,
            "submittedAt": r.at.to_string(),
        })
    };
    let comment_json =
        |c: &CommentData| json!({ "author": person(&c.author), "body": c.body, "createdAt": c.at.to_string() });
    let threads: Vec<Value> = p
        .threads
        .iter()
        .filter(|t| t.comments.first().is_some_and(|c| public(p, c)))
        .map(|t| {
            let comments: Vec<Value> = t.comments.iter().filter(|c| public(p, c)).map(comment_json).collect();
            json!({ "id": t.id, "isResolved": t.resolved, "path": t.path, "line": t.line, "diffSide": t.side,
                    "comments": { "nodes": comments } })
        })
        .collect();
    let checks: Vec<Value> = p
        .checks
        .iter()
        .map(|c| {
            let started = p.created;
            let done = started.checked_add(jiff::SignedDuration::from_secs(c.seconds)).unwrap_or(started);
            json!({ "__typename": "CheckRun", "name": c.name, "status": "COMPLETED",
                    "conclusion": if c.passed { "SUCCESS" } else { "FAILURE" }, "title": c.title,
                    "startedAt": started.to_string(), "completedAt": done.to_string() })
        })
        .collect();
    v["id"] = json!(node_id(owner, name, p.number));
    v["body"] = json!(p.body);
    v["createdAt"] = json!(p.created.to_string());
    v["mergeable"] = json!(if p.merged { "UNKNOWN" } else { "MERGEABLE" });
    v["commits"] = json!({ "totalCount": p.commits });
    v["reviewRequests"] =
        json!({ "nodes": p.requested.iter().map(|r| json!({ "requestedReviewer": person(r) })).collect::<Vec<_>>() });
    v["latestReviews"] = json!({ "nodes": latest.into_iter().map(review_json).collect::<Vec<_>>() });
    v["reviews"] = json!({ "nodes": submitted.into_iter().map(review_json).collect::<Vec<_>>() });
    v["comments"] = json!({ "nodes": p.comments.iter().map(comment_json).collect::<Vec<_>>() });
    v["reviewThreads"] = json!({ "nodes": threads });
    v["lastCommit"] = json!({ "nodes": [{ "commit": { "statusCheckRollup": { "contexts": { "nodes": checks } } } }] });
    v
}

/// `addPullRequestReview`: a pending review by the viewer, with new threads on lines.
fn add_review(s: &mut State, vars: &Value) -> Value {
    let (viewer, now) = (s.viewer.clone(), s.now);
    let id = format!("PRR_{}", s.next_id);
    s.next_id += 1;
    let mut next = s.next_id;
    let Some(p) = pull_mut(s, vars["pullRequestId"].as_str().unwrap_or_default()) else {
        return error("Could not resolve to a PullRequest");
    };
    if vars["commitOID"].as_str() != Some(&p.head_sha) {
        return error("The review must be on the pull request's head commit");
    }
    let mut sent = Vec::new();
    for t in vars["threads"].as_array().cloned().unwrap_or_default() {
        let (path, body) = (t["path"].as_str().unwrap_or_default(), t["body"].as_str().unwrap_or_default());
        let line = t["line"].as_u64().and_then(|l| u32::try_from(l).ok()).unwrap_or(0);
        let side = if t["side"].as_str() == Some("LEFT") { "LEFT" } else { "RIGHT" };
        let comment = CommentData { author: viewer.clone(), body: body.to_owned(), at: now, review: Some(id.clone()) };
        p.threads.push(ThreadData {
            id: format!("PRRT_{next}"),
            path: path.to_owned(),
            line,
            side,
            resolved: false,
            comments: vec![comment],
        });
        next += 1;
        sent.push(SentComment { path: path.to_owned(), line: Some(line), body: body.to_owned(), thread: None });
    }
    p.reviews.push(ReviewData { id: id.clone(), author: viewer, verdict: None, body: String::new(), at: now, sent });
    s.next_id = next;
    json!({ "data": { "addPullRequestReview": { "pullRequestReview": { "id": id } } } })
}

/// `addPullRequestReviewThreadReply`: a reply, held in the pending review.
fn reply(s: &mut State, vars: &Value) -> Value {
    let (viewer, now) = (s.viewer.clone(), s.now);
    let (review, thread) =
        (vars["reviewId"].as_str().unwrap_or_default(), vars["threadId"].as_str().unwrap_or_default());
    let body = vars["body"].as_str().unwrap_or_default().to_owned();
    for (_, _, pulls) in &mut s.repos {
        for p in pulls {
            let Some(r) = p.reviews.iter().position(|r| r.id == review && r.verdict.is_none()) else { continue };
            let Some(t) = p.threads.iter_mut().find(|t| t.id == thread) else {
                return error("Could not resolve to a PullRequestReviewThread");
            };
            t.comments.push(CommentData {
                author: viewer,
                body: body.clone(),
                at: now,
                review: Some(review.to_owned()),
            });
            let path = t.path.clone();
            p.reviews[r].sent.push(SentComment { path, line: None, body, thread: Some(thread.to_owned()) });
            return json!({ "data": { "addPullRequestReviewThreadReply": { "comment": { "id": format!("C_{}", now.as_second()) } } } });
        }
    }
    error("Could not resolve to a pending PullRequestReview")
}

/// `submitPullRequestReview`: the pending review goes public with its verdict and body; the
/// viewer is no longer waited on.
fn submit(s: &mut State, vars: &Value) -> Value {
    let review = vars["reviewId"].as_str().unwrap_or_default();
    let verdict = match vars["event"].as_str() {
        Some("APPROVE") => Verdict::Approve,
        Some("REQUEST_CHANGES") => Verdict::RequestChanges,
        Some("COMMENT") => Verdict::Comment,
        _ => return error("Unknown PullRequestReviewEvent"),
    };
    for (_, _, pulls) in &mut s.repos {
        for p in pulls {
            let Some(r) = p.reviews.iter_mut().find(|r| r.id == review && r.verdict.is_none()) else { continue };
            r.verdict = Some(verdict);
            r.body = vars["body"].as_str().unwrap_or_default().to_owned();
            let who = r.author.login;
            p.requested.retain(|q| q.login != who);
            return json!({ "data": { "submitPullRequestReview": { "pullRequestReview": { "id": review, "state": verdict_state(verdict) } } } });
        }
    }
    error("Could not resolve to a pending PullRequestReview")
}

/// `mergePullRequest` with `SQUASH`: the branch's changes become one commit on the base, in the
/// world's bare repository, by real git, as GitHub does on its own copy.
fn merge(s: &mut State, vars: &Value) -> Value {
    let id = vars["pullRequestId"].as_str().unwrap_or_default().to_owned();
    let (remotes, env) = (s.remotes.clone(), s.env.clone());
    let Some((owner, name)) = s
        .repos
        .iter()
        .find_map(|(o, n, pulls)| pulls.iter().any(|p| node_id(o, n, p.number) == id).then(|| (o.clone(), n.clone())))
    else {
        return error("Could not resolve to a PullRequest");
    };
    let Some(p) = pull_mut(s, &id) else { return error("Could not resolve to a PullRequest") };
    if p.merged {
        return error("Pull request is already merged");
    }
    if vars["expectedHeadOid"].as_str() != Some(&p.head_sha) {
        return error("Head branch was modified. Review and try the merge again.");
    }
    if !p.checks.iter().all(|c| c.passed) {
        return error("Required status checks have not passed");
    }
    let bare = remotes.join("github.com").join(&owner).join(format!("{name}.git"));
    let git = |args: &[&str]| gxm_core::git::run(&env, &bare, args).map(|o| o.trim().to_owned());
    let headline = vars["commitHeadline"].as_str().unwrap_or(&p.title).to_owned();
    let made = (|| {
        let base = git(&["rev-parse", &format!("refs/heads/{}", p.base_ref)])?;
        let tree = git(&["merge-tree", "--write-tree", &base, &p.head_sha])?;
        let tree = tree.lines().next().unwrap_or_default().to_owned();
        let commit = git(&["commit-tree", &tree, "-p", &base, "-m", &headline])?;
        git(&["update-ref", &format!("refs/heads/{}", p.base_ref), &commit, &base])?;
        Ok::<_, gxm_core::git::GitError>(commit)
    })();
    match made {
        Ok(commit) => {
            p.merged = true;
            p.merge_commit = Some(commit);
            json!({ "data": { "mergePullRequest": { "pullRequest": { "state": "MERGED" } } } })
        }
        Err(e) => error(&format!("Merge conflict: {e}")),
    }
}
