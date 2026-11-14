# Pull requests: the chosen flow

The goal is that a person never needs a provider's PR page (GitHub, GitLab, Bitbucket) again. These mockups are the spec for the first slice of that. They're built from this folder's `components.css` (kept from the retired design system) with the app's fonts. Regenerate them with `python3 pages.py && python3 flow.py`, and open the HTML from this folder.

**Chosen: C + B.** The person approved this flow on 2026-11-14. A and the standalone B and C are kept for reference.

| Step | Frame | What it shows |
|---|---|---|
| 1 | `flow1` | A commit selected. The inspector shows labelled rows (Author, Date with absolute and relative time, Commit, Parent or Parents, Pull request with state and number) and a **See pull request** button. |
| 2 | `flow2` | **The PR on the graph.** The PR's lane is lit and the rest of the history dims. A `#131` row sits above the PR's tip with its failing-check and approval counts. The inspector becomes the PR: header, description, merge box, reviewers, and tabs for Overview, Checks and Conversation. The toolbar becomes Review (primary), Check out and Squash and merge (disabled while a check fails), plus "Close pull request" to return to the plain history. |
| 3 | `flow3` | **The review cockpit**, opened from Review. The sidebar becomes the PR's files (with viewed progress) and commits, plus a back link "Pull request #131". The middle is the diff with inline threads and replies, Unified or Split. The right column holds the verdict: summary, Comment / Approve / Request changes, and Submit review (⌘↩). |
| 4 | `flow4` | Back on the graph after approving. The PR row shows 2 approvals, your reviewer row shows Approved, and the merge box says it's ready once checks pass. |

`inspector.*` shows the inspector for a commit in an open PR and for a merge commit from a merged PR.

`extras.*` (`python3 extras.py`) proposes the states the flow needs that the frames don't draw, built with the first slice and **awaiting sign-off**: draft comments and a pending reply in the cockpit (a `Pending` pill, the composer under a pressed line), Split, Viewed ticked, the inspector's Checks and Conversation tabs (after `a`'s checks list and timeline), a repository whose provider isn't connected, the merge box once a pull request is ready and while confirming the squash commit's message, and the cockpit with one commit picked.

## Decisions still open

- "Close pull request" only returns to the history; it doesn't close the PR on the provider. It needs a clearer name.
- Draft review comments are kept locally and sent to the provider together, in one review, on Submit review. (Built: GitHub gets one pending review holding every comment and reply, then submits it.)
- The flow's inspector (flow1, `inspector`) has no diff preview under Changed files; the app keeps its first-hunk preview from the earlier design until the person says otherwise.
- flow2's summary line ends "its commits are lit, the rest of the history dims", which reads like a note to the reader; the app shows it as drawn.
- Not designed yet: re-running a check and reading its log, editing the title and description, requesting reviewers, resolving threads.
