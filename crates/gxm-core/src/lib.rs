//! git-ex-machina's domain: repositories, history, the commit graph and git operations.
//!
//! No UI lives here. The app (`gxm-app`) and the CLI (`gxm-cli`) are thin layers over it, so
//! everything here can be verified from either.

pub mod auth;
pub mod clone;
pub mod diff;
pub mod env;
pub mod forge;
pub mod git;
pub mod graph;
pub mod history;
pub mod init;
pub mod local;
pub mod ops;
pub mod remote;
pub mod repo;
pub mod secrets;
pub mod tidy;
pub mod time;
pub mod update;
pub mod workspace;
