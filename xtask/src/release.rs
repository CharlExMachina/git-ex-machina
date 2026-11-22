//! What every platform's release build checks first: a tag names this version, and its notes
//! are written. The release workflow passes the tag it was started by.

use std::path::PathBuf;

use crate::{flag, root};

/// The workspace's version, after checking `--tag`, when given, names it and has its notes.
pub fn version(args: &[String]) -> Result<&'static str, String> {
    let version = env!("CARGO_PKG_VERSION");
    if let Some(tag) = flag(args, "--tag") {
        if tag != format!("v{version}") {
            return Err(format!("the tag {tag} isn't this version, v{version}: bump the workspace version first"));
        }
        notes(tag)?;
    }
    Ok(version)
}

/// The release notes for `tag`, which the app shows when it offers the update.
fn notes(tag: &str) -> Result<PathBuf, String> {
    let path = root().join("docs/releases").join(format!("{tag}.md"));
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "{}: {e}. Write the release's notes there: the app shows them when it offers the update",
            path.display()
        )
    })?;
    if text.trim().is_empty() {
        return Err(format!("{} is empty", path.display()));
    }
    Ok(path)
}
