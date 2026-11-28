//! Lockfiles: generated, so no one resolves them line by line. A conflicted lockfile takes the
//! base's side, then its ecosystem's tool writes it again from the merged manifests, when that
//! tool is on the path. Without the tool, it's left as an ordinary conflict.

use std::path::Path;

/// A lockfile kind, by its file name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lockfile {
    Cargo,
    Npm,
    Yarn,
    Pnpm,
}

impl Lockfile {
    /// The kind of lockfile `path` is, if it's one.
    pub fn of(path: &str) -> Option<Lockfile> {
        match Path::new(path).file_name()?.to_str()? {
            "Cargo.lock" => Some(Lockfile::Cargo),
            "package-lock.json" => Some(Lockfile::Npm),
            "yarn.lock" => Some(Lockfile::Yarn),
            "pnpm-lock.yaml" => Some(Lockfile::Pnpm),
            _ => None,
        }
    }

    /// The tool that writes it.
    pub fn tool(self) -> &'static str {
        match self {
            Lockfile::Cargo => "cargo",
            Lockfile::Npm => "npm",
            Lockfile::Yarn => "yarn",
            Lockfile::Pnpm => "pnpm",
        }
    }

    /// What updates it to match the manifests without upgrading anything else, run in the
    /// lockfile's folder: `cargo metadata` resolves only what changed, as a build would.
    pub fn regenerate(self) -> &'static [&'static str] {
        match self {
            Lockfile::Cargo => &["metadata", "--format-version", "1", "--quiet"],
            Lockfile::Npm => &["install", "--package-lock-only", "--ignore-scripts", "--no-audit", "--no-fund"],
            Lockfile::Yarn => &["install", "--ignore-scripts"],
            Lockfile::Pnpm => &["install", "--lockfile-only", "--ignore-scripts"],
        }
    }

    /// `cargo metadata`, as the sidebar says what was run.
    pub fn command(self) -> String {
        format!("{} {}", self.tool(), self.regenerate()[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockfiles_by_name_and_how_each_is_written_again() {
        assert_eq!(Lockfile::of("Cargo.lock"), Some(Lockfile::Cargo));
        assert_eq!(Lockfile::of("web/package-lock.json"), Some(Lockfile::Npm));
        assert_eq!(Lockfile::of("yarn.lock"), Some(Lockfile::Yarn));
        assert_eq!(Lockfile::of("apps/site/pnpm-lock.yaml"), Some(Lockfile::Pnpm));
        assert_eq!(Lockfile::of("Cargo.toml"), None);
        assert_eq!(Lockfile::of("docs/Cargo.lock.md"), None);
        assert_eq!(Lockfile::Cargo.command(), "cargo metadata");
        assert_eq!(Lockfile::Npm.command(), "npm install");
        assert_eq!(Lockfile::Pnpm.tool(), "pnpm");
    }
}
