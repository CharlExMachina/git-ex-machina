//! What every platform's release build checks first: a tag names this version. The release
//! workflow passes the tag it was started by (its notes, the tag's message, it checks itself).

use crate::flag;

/// The workspace's version, after checking `--tag`, when given, names it.
pub fn version(args: &[String]) -> Result<&'static str, String> {
    let version = env!("CARGO_PKG_VERSION");
    if let Some(tag) = flag(args, "--tag")
        && tag != format!("v{version}")
    {
        return Err(format!("the tag {tag} isn't this version, v{version}: bump the workspace version first"));
    }
    Ok(version)
}
