//! Ready-made worlds.
//!
//! [`mockup`] is the demo workspace: seven repositories across GitHub, GitLab and Bitbucket in
//! different states, built with real git. Screenshots, journeys and the README use it.

use std::path::PathBuf;

use jiff::Timestamp;

use crate::{Author, RepoBuilder, World};

mod atlas;
mod pulls;

pub use atlas::atlas_web;
pub use pulls::{DESCRIPTION, atlas_pulls, dotfiles_pull};

// The app colours people by a hash of their email; these addresses give the colours the
// design gives them (Ana blue, Kenji teal, Lea rose).
pub const ANA: Author = Author { name: "Ana Ruiz", email: "ana@example.com" };
pub const KENJI: Author = Author { name: "Kenji Mori", email: "kenji@example.com" };
pub const LEA: Author = Author { name: "Lea Brandt", email: "lea.brandt@example.com" };

/// The repositories [`mockup`] made.
pub struct Mockup {
    pub atlas_web: PathBuf,
    pub rune_playground: PathBuf,
    pub payments_api: PathBuf,
    pub brand_tokens: PathBuf,
    pub dotfiles: PathBuf,
    pub field_notes: PathBuf,
    pub infra_terraform: PathBuf,
}

/// The demo workspace: atlas-web with its branches, tag and uncommitted work, and
/// six more repositories across GitHub, GitLab and Bitbucket in different states.
pub fn mockup(w: &World) -> Mockup {
    let atlas_web = atlas_web(w);
    atlas_pulls(w, &atlas_web);
    let m = Mockup {
        rune_playground: simple(
            w,
            Simple {
                host: "github.com",
                owner: "personal",
                name: "rune-playground",
                branch: "spring-tuning",
                ..Simple::DEFAULT
            },
        ),
        payments_api: simple(
            w,
            Simple {
                host: "bitbucket.org",
                owner: "studio",
                name: "payments-api",
                branch: "release/2.3",
                behind: 4,
                ..Simple::DEFAULT
            },
        ),
        brand_tokens: simple(
            w,
            Simple { host: "github.com", owner: "studio", name: "brand-tokens", dirty: 12, ..Simple::DEFAULT },
        ),
        dotfiles: simple(w, Simple { host: "github.com", owner: "personal", name: "dotfiles", ..Simple::DEFAULT }),
        field_notes: simple(
            w,
            Simple { host: "github.com", owner: "personal", name: "field-notes", branch: "drafts", ..Simple::DEFAULT },
        ),
        infra_terraform: simple(
            w,
            Simple { host: "gitlab.com", owner: "studio", name: "infra-terraform", behind: 1, ..Simple::DEFAULT },
        ),
        atlas_web,
    };
    dotfiles_pull(w, &m.dotfiles);
    w.add_to_workspace(&m.atlas_web, true);
    w.add_to_workspace(&m.rune_playground, true);
    for repo in [&m.payments_api, &m.brand_tokens, &m.dotfiles, &m.field_notes, &m.infra_terraform] {
        w.add_to_workspace(repo, false);
    }
    m
}

pub(crate) fn t(s: &str) -> Timestamp {
    s.parse().unwrap_or_else(|e| panic!("fixture time {s:?}: {e}"))
}

/// A repository with a short linear history on one branch, in a given state.
#[derive(Clone, Copy)]
struct Simple {
    host: &'static str,
    owner: &'static str,
    name: &'static str,
    branch: &'static str,
    /// Commits the remote has that the working copy doesn't.
    behind: usize,
    /// Uncommitted files.
    dirty: usize,
}

impl Simple {
    const DEFAULT: Simple =
        Simple { host: "github.com", owner: "studio", name: "", branch: "main", behind: 0, dirty: 0 };
}

fn simple(w: &World, s: Simple) -> PathBuf {
    let r: RepoBuilder<'_> = w.repo(s.host, s.owner, s.name);
    let people = [ANA, KENJI, LEA];
    let commits = 6 + s.behind;
    r.commit(people[0], t("2026-10-01T09:00:00Z"), "Initial commit", &["README.md"]);
    if s.branch != "main" {
        r.branch(s.branch);
    }
    for i in 1..commits {
        let at = t("2026-10-01T09:00:00Z")
            .checked_add(jiff::SignedDuration::from_hours(30 * i as i64))
            .unwrap_or_else(|e| panic!("{e}"));
        r.commit(people[i % 3], at, &format!("Update {} ({i})", s.name), &[&format!("notes/{i}.md")]);
    }
    r.push(s.branch);
    if s.behind > 0 {
        r.reset_hard(&format!("HEAD~{}", s.behind));
    }
    for i in 0..s.dirty {
        r.write(&format!("wip/{i}.txt"), "work in progress\n");
    }
    r.path().to_path_buf()
}
