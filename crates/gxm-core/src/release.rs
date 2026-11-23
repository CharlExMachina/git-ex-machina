//! Whether a repository releases from tags: its CI runs when a tag is pushed. GitHub Actions
//! (`on: push: tags:` in `.github/workflows/`), GitLab CI (`$CI_COMMIT_TAG` or `only: tags` in
//! `.gitlab-ci.yml`) and Bitbucket Pipelines (`pipelines: tags:` in `bitbucket-pipelines.yml`).
//!
//! The files are read just far enough to find the trigger, by indentation, with no YAML parser:
//! a trigger written in a form this misses only means the app offers a plain tag.

use std::path::Path;

/// What pushing a tag starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trigger {
    /// The tag names it runs for, as globs (`v*`). Empty: any tag.
    pub patterns: Vec<String>,
    /// What runs, as a sentence names it: "the release workflow".
    pub what: String,
    /// The file that says so, from the repository's root.
    pub file: String,
    /// Where the release it makes takes its notes from.
    pub notes: NotesFrom,
}

/// Where a release's notes come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotesFrom {
    /// The tag's message (`gh release create --notes-from-tag`): the tag sheet writes them.
    Tag,
    /// A file in the repository (`--notes-file`, `body_path:`), as the workflow names it.
    File(String),
    /// GitHub writes them from the merged pull requests (`--generate-notes`).
    Generated,
    /// The workflow doesn't say in a way this reads.
    Unknown,
}

impl NotesFrom {
    /// Where a GitHub workflow's release takes its notes from: `gh release create`'s flags, or
    /// `softprops/action-gh-release`'s inputs.
    pub fn of(doc: &str) -> NotesFrom {
        let words: Vec<&str> = doc.split_whitespace().collect();
        let after = |flag: &str| {
            words.iter().position(|w| *w == flag || w.starts_with(&format!("{flag}="))).map(|i| {
                let w = words[i];
                let value = w.split_once('=').map_or_else(|| words.get(i + 1).copied().unwrap_or_default(), |(_, v)| v);
                value.trim_matches(['"', '\'']).to_owned()
            })
        };
        if words.contains(&"--notes-from-tag") {
            NotesFrom::Tag
        } else if let Some(file) = after("--notes-file").or_else(|| after("-F")).or_else(|| after("body_path:")) {
            NotesFrom::File(file)
        } else if words.contains(&"--generate-notes") || doc.contains("generate_release_notes: true") {
            NotesFrom::Generated
        } else {
            NotesFrom::Unknown
        }
    }
}

impl Trigger {
    /// Whether pushing `tag` starts it.
    pub fn runs_for(&self, tag: &str) -> bool {
        self.patterns.is_empty() || self.patterns.iter().any(|p| glob(p, tag))
    }

    /// The file's name, without its folder: `release.yml`.
    pub fn file_name(&self) -> &str {
        self.file.rsplit('/').next().unwrap_or(&self.file)
    }
}

/// The first CI in the work tree that runs on a pushed tag. Reads files: off the UI thread.
pub fn trigger(repo: &Path) -> Option<Trigger> {
    let read = |file: &str| std::fs::read_to_string(repo.join(file)).ok();
    let mut workflows: Vec<String> = std::fs::read_dir(repo.join(".github/workflows"))
        .map(|dir| {
            dir.filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter(|n| n.ends_with(".yml") || n.ends_with(".yaml"))
                .collect()
        })
        .unwrap_or_default();
    workflows.sort();
    workflows
        .iter()
        .find_map(|name| {
            let file = format!(".github/workflows/{name}");
            github(&read(&file)?, &file)
        })
        .or_else(|| gitlab(&read(".gitlab-ci.yml")?))
        .or_else(|| bitbucket(&read("bitbucket-pipelines.yml")?))
}

/// A GitHub Actions workflow that runs on `push: tags:`, named by its `name:`.
pub fn github(doc: &str, file: &str) -> Option<Trigger> {
    let lines = lines(doc);
    let (value, block) = find(&lines, &["on", "push", "tags"]).or_else(|| find(&lines, &["\"on\"", "push", "tags"]))?;
    let patterns = list(&lines, value, block);
    let stem = file.rsplit('/').next().unwrap_or(file).trim_end_matches(".yml").trim_end_matches(".yaml");
    let name = find(&lines, &["name"]).map(|(v, _)| unquote(v)).filter(|n| !n.is_empty());
    let name = name.unwrap_or_else(|| stem.to_owned());
    Some(Trigger { patterns, what: format!("the {name} workflow"), file: file.to_owned(), notes: NotesFrom::of(doc) })
}

/// GitLab CI with a job for tags: a rule on `$CI_COMMIT_TAG`, or `only: tags`.
pub fn gitlab(doc: &str) -> Option<Trigger> {
    let lines = lines(doc);
    let on_tags = lines.iter().enumerate().any(|(i, l)| {
        if l.text.contains("CI_COMMIT_TAG") {
            return true;
        }
        match key(l.text) {
            Some(("only", value)) => {
                let block = i + 1..block_end(&lines, i);
                list(&lines, value, block).iter().any(|v| v == "tags")
            }
            _ => false,
        }
    });
    on_tags.then(|| Trigger {
        patterns: Vec::new(),
        what: "the tag pipeline".to_owned(),
        file: ".gitlab-ci.yml".to_owned(),
        notes: NotesFrom::Unknown,
    })
}

/// Bitbucket Pipelines with `pipelines: tags:`, whose keys are the tag globs.
pub fn bitbucket(doc: &str) -> Option<Trigger> {
    let lines = lines(doc);
    let (_, block) = find(&lines, &["pipelines", "tags"])?;
    let patterns = keys_of(&lines, block).into_iter().filter(|p| p != "*" && p != "**").collect();
    Some(Trigger {
        patterns,
        what: "the tag pipeline".to_owned(),
        file: "bitbucket-pipelines.yml".to_owned(),
        notes: NotesFrom::Unknown,
    })
}

/// GitHub's tag filter globs: `*` within a path segment, `**` across them, `?` one character.
pub fn glob(pattern: &str, name: &str) -> bool {
    fn go(p: &[char], n: &[char]) -> bool {
        match p {
            [] => n.is_empty(),
            ['*', '*', rest @ ..] => (0..=n.len()).any(|i| go(rest, &n[i..])),
            ['*', rest @ ..] => (0..=n.len()).take_while(|&i| i == 0 || n[i - 1] != '/').any(|i| go(rest, &n[i..])),
            ['?', rest @ ..] => !n.is_empty() && go(rest, &n[1..]),
            [c, rest @ ..] => n.first() == Some(c) && go(rest, &n[1..]),
        }
    }
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    go(&p, &n)
}

/// A meaningful line: how far it's indented, and its text without a trailing comment.
struct Line<'a> {
    indent: usize,
    text: &'a str,
}

fn lines(doc: &str) -> Vec<Line<'_>> {
    doc.lines()
        .filter_map(|raw| {
            let text = raw.trim_start();
            let text = text.split_once(" #").map_or(text, |(t, _)| t).trim_end();
            (!text.is_empty() && !text.starts_with('#'))
                .then(|| Line { indent: raw.len() - raw.trim_start().len(), text })
        })
        .collect()
}

/// `key: value` split into the key, unquoted, and the value; `None` for a line that isn't one.
fn key(text: &str) -> Option<(&str, &str)> {
    let text = text.strip_prefix("- ").unwrap_or(text);
    let (k, v) = text.split_once(':').filter(|(_, v)| v.is_empty() || v.starts_with(' '))?;
    Some((k.trim().trim_matches(['"', '\'']), v.trim()))
}

/// Where the block under the key on line `i` ends: the first line indented no deeper, except a
/// list's `- ` items, which may sit at the key's own indent.
fn block_end(lines: &[Line<'_>], i: usize) -> usize {
    let at = lines[i].indent;
    let below = lines[i + 1..].iter().position(|l| l.indent < at || (l.indent == at && !l.text.starts_with("- ")));
    below.map_or(lines.len(), |n| i + 1 + n)
}

/// The value on the line of the key at `path` (nested keys, outermost first), and the block of
/// lines under it.
fn find<'a>(lines: &[Line<'a>], path: &[&str]) -> Option<(&'a str, std::ops::Range<usize>)> {
    let path = path.iter().map(|k| k.trim_matches('"'));
    let mut block = 0..lines.len();
    let mut found = None;
    for want in path {
        let indent = lines.get(block.start)?.indent;
        let i =
            block.clone().find(|&i| lines[i].indent == indent && key(lines[i].text).is_some_and(|(k, _)| k == want))?;
        let value = key(lines[i].text).map_or("", |(_, v)| v);
        block = i + 1..block_end(lines, i);
        found = Some((value, block.clone()));
    }
    found
}

/// A list's items: a flow list or a scalar on the key's line, else the block's `- ` items.
fn list(lines: &[Line<'_>], value: &str, block: std::ops::Range<usize>) -> Vec<String> {
    if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        return inner.split(',').map(unquote).filter(|v| !v.is_empty()).collect();
    }
    if !value.is_empty() {
        return vec![unquote(value)];
    }
    let indent = block.clone().next().map(|i| lines[i].indent);
    block
        .filter(|&i| Some(lines[i].indent) == indent)
        .filter_map(|i| lines[i].text.strip_prefix("- ").map(unquote))
        .collect()
}

/// The keys of the map in `block`.
fn keys_of(lines: &[Line<'_>], block: std::ops::Range<usize>) -> Vec<String> {
    let indent = block.clone().next().map(|i| lines[i].indent);
    block
        .filter(|&i| Some(lines[i].indent) == indent)
        .filter_map(|i| key(lines[i].text))
        .map(|(k, _)| k.to_owned())
        .collect()
}

fn unquote(v: &str) -> String {
    v.trim().trim_matches(['"', '\'']).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_github_workflow_on_pushed_tags_releases_with_its_patterns_and_name() {
        let flow = "name: release\n# A tag releases.\non:\n  push:\n    tags: [\"v*\", 'release-*']\njobs:\n  a:\n    runs-on: x\n";
        let t = github(flow, ".github/workflows/release.yml").unwrap();
        assert_eq!(t.patterns, ["v*", "release-*"]);
        assert_eq!((t.what.as_str(), t.file_name()), ("the release workflow", "release.yml"));

        let block = "\"on\":\n  workflow_dispatch:\n  push:\n    branches:\n      - main\n    tags:\n      - 'v[0-9]*'  # versions\njobs: {}\n";
        let t = github(block, ".github/workflows/publish.yaml").unwrap();
        assert_eq!(t.patterns, ["v[0-9]*"]);
        assert_eq!(t.what, "the publish workflow", "no name: the file's");

        let same_indent = "on:\n  push:\n    tags:\n    - v*\n";
        assert_eq!(github(same_indent, "x.yml").unwrap().patterns, ["v*"]);
    }

    #[test]
    fn a_release_takes_its_notes_from_the_tag_a_file_or_github() {
        let notes = |step: &str| {
            NotesFrom::of(&format!("on:\n  push:\n    tags: [v*]\njobs:\n  a:\n    steps:\n      - run: {step}\n"))
        };
        assert_eq!(notes("gh release create \"$TAG\" --notes-from-tag --verify-tag"), NotesFrom::Tag);
        let file = notes("gh release create \"$TAG\" --notes-file \"docs/releases/$GITHUB_REF_NAME.md\"");
        assert_eq!(file, NotesFrom::File("docs/releases/$GITHUB_REF_NAME.md".to_owned()));
        assert_eq!(notes("gh release create $TAG --generate-notes"), NotesFrom::Generated);
        assert_eq!(notes("make release"), NotesFrom::Unknown);
        let action = "steps:\n  - uses: softprops/action-gh-release@v2\n    with:\n      body_path: CHANGELOG.md\n";
        assert_eq!(NotesFrom::of(action), NotesFrom::File("CHANGELOG.md".to_owned()));
    }

    #[test]
    fn workflows_on_branches_pull_requests_or_published_releases_are_not_tag_releases() {
        for doc in [
            "name: ci\non:\n  push:\n    branches: [main]\n  pull_request:\njobs:\n  tags:\n    x: 1\n",
            "on: [push, pull_request]\n",
            "on:\n  release:\n    types: [published]\n",
            "jobs:\n  build:\n    steps:\n      - name: tags\n        on:\n          push:\n            tags: [v*]\n",
        ] {
            assert_eq!(github(doc, "ci.yml"), None, "{doc}");
        }
    }

    #[test]
    fn gitlab_releases_from_tags_on_a_tag_rule_or_only_tags() {
        let rule = "release:\n  stage: deploy\n  rules:\n    - if: $CI_COMMIT_TAG\n";
        assert_eq!(gitlab(rule).unwrap().patterns, Vec::<String>::new());
        assert!(gitlab("publish:\n  script: make\n  only:\n    - tags\n").is_some());
        assert!(gitlab("publish:\n  only: [tags, main]\n").is_some());
        assert_eq!(gitlab("test:\n  only:\n    - main\n  # $CI_COMMIT_TAG someday\n"), None);
    }

    #[test]
    fn bitbucket_releases_from_the_tags_its_pipelines_name() {
        let doc = "pipelines:\n  default:\n    - step:\n        script: [make]\n  tags:\n    'v*':\n      - step:\n          script: [make release]\n";
        assert_eq!(bitbucket(doc).unwrap().patterns, ["v*"]);
        assert_eq!(bitbucket("pipelines:\n  branches:\n    main: []\n"), None);
    }

    #[test]
    fn tag_globs_match_as_github_filters_do() {
        assert!(glob("v*", "v0.1.2"));
        assert!(!glob("v*", "0.1.2"));
        assert!(!glob("v*", "v1/rc"), "* stays within a segment");
        assert!(glob("v**", "v1/rc"));
        assert!(glob("v?.*", "v1.2"));
        assert!(glob("release", "release") && !glob("release", "release-1"));
    }
}
