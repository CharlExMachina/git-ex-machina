//! git-ex-machina's domain: repositories, history, the commit graph and git operations.
//!
//! No UI lives here. The app (`gxm-app`) and the CLI (`gxm-cli`) are thin layers over it, so
//! everything here can be verified from either.

pub mod diff;
pub mod env;
pub mod forge;
pub mod git;
pub mod graph;
pub mod history;
pub mod ops;
pub mod remote;
pub mod repo;
pub mod time;
pub mod workspace;
