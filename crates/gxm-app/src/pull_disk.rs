//! What the pull-request side reads from and writes to disk, off the UI thread: the cached
//! answers shown while the provider is asked again, the review drafts, and one commit's changes
//! for the review cockpit. Each is a latest-wins job (`rune::Latest`), like `load.rs`'s reads.

use std::path::PathBuf;

use gxm_core::env::Env;
use gxm_core::forge::drafts::{self, ReviewDraft};
use gxm_core::forge::local::{self, Changes};
use gxm_core::forge::model::Pull;
use gxm_core::forge::{RepoKey, cache};
use rune::{Latest, Waker};

use crate::pulls::Index;

#[derive(Default)]
pub struct PullDisk {
    cached_index: Latest<(RepoKey, Option<Index>)>,
    cached_pull: Latest<(RepoKey, u64, Option<Pull>)>,
    draft: Latest<(PathBuf, ReviewDraft)>,
    save: Latest<()>,
    commit: Latest<(String, Result<Changes, String>)>,
}

/// What the disk answered.
#[derive(Default)]
pub struct DiskLanded {
    pub index: Option<(RepoKey, Index)>,
    pub pull: Option<(RepoKey, u64, Pull)>,
    pub draft: Option<(PathBuf, ReviewDraft)>,
    pub commit: Option<(String, Result<Changes, String>)>,
}

impl PullDisk {
    pub fn set_waker(&self, waker: &Waker) {
        self.cached_index.set_waker(waker.clone());
        self.cached_pull.set_waker(waker.clone());
        self.draft.set_waker(waker.clone());
        self.save.set_waker(waker.clone());
        self.commit.set_waker(waker.clone());
    }

    pub fn busy(&self) -> bool {
        self.cached_index.is_pending()
            || self.cached_pull.is_pending()
            || self.draft.is_pending()
            || self.save.is_pending()
            || self.commit.is_pending()
    }

    /// The index the provider last gave for `key`.
    pub fn read_index(&self, env: &Env, key: &RepoKey) {
        self.cached_index.request((env.clone(), key.clone()), |(env, key)| {
            let index = cache::load(&env, &key, "pulls");
            (key, index)
        });
    }

    /// Pull request `n` as the provider last gave it.
    pub fn read_pull(&self, env: &Env, key: &RepoKey, n: u64) {
        self.cached_pull.request((env.clone(), key.clone()), move |(env, key)| {
            let pull = cache::load(&env, &key, &format!("pull-{n}"));
            (key, n, pull)
        });
    }

    pub fn read_draft(&self, path: PathBuf) {
        self.draft.request(path, |path| {
            let draft = drafts::load(&path);
            (path, draft)
        });
    }

    /// Saves the draft; a newer save supersedes one not yet started, so the newest always lands.
    pub fn save_draft(&self, path: PathBuf, draft: ReviewDraft) {
        self.save.request((path, draft), |(path, draft)| {
            // A draft that can't be saved still lives in memory until the review is sent.
            drafts::save(&path, &draft).ok();
        });
    }

    /// What commit `id` changed, for the cockpit to show it alone.
    pub fn read_commit(&self, env: &Env, repo: PathBuf, id: String) {
        self.commit.request((env.clone(), repo, id), |(env, repo, id)| {
            let changes = local::commit_changes(&env, &repo, &id).map_err(|e| e.to_string());
            (id, changes)
        });
    }

    pub fn land(&self) -> DiskLanded {
        self.save.take();
        DiskLanded {
            index: self.cached_index.take().and_then(|(k, i)| i.map(|i| (k, i))),
            pull: self.cached_pull.take().and_then(|(k, n, p)| p.map(|p| (k, n, p))),
            draft: self.draft.take(),
            commit: self.commit.take(),
        }
    }
}
