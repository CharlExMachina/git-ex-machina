//! An AppImage updates by one rename: the new file downloads beside it and takes its name. The
//! running app goes on from the old file, which the AppImage runtime holds open.

use std::path::{Path, PathBuf};

use super::{Release, UpdateError};
use crate::env::Env;

/// Downloads `release` beside `file`, checks it, and puts it in `file`'s place.
pub(super) fn install(file: &Path, release: &Release, progress: &mut dyn FnMut(u64, u64)) -> Result<(), UpdateError> {
    // Beside it, so the rename that puts it in place never crosses file systems.
    let part = partial(file);
    super::download(&part, release, progress)?;
    let placed = executable(&part)
        .map_err(|e| UpdateError::Write(format!("couldn't write {}: {e}", part.display())))
        .and_then(|()| {
            std::fs::rename(&part, file)
                .map_err(|e| UpdateError::Write(format!("couldn't replace {}: {e}", file.display())))
        });
    if placed.is_err() {
        std::fs::remove_file(&part).ok();
    }
    placed
}

/// Starts `file`, the new version once [`install`] has put it there. (Rune's own relaunch would
/// start the executable inside the old AppImage's mount, which goes away with this process.)
pub(super) fn restart(env: &Env, file: &Path) -> Result<(), UpdateError> {
    let mut cmd = std::process::Command::new(file);
    cmd.envs(env.child_vars())
        .current_dir(&env.home)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // Its own process group, so a terminal this one was started from doesn't take it down.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    cmd.spawn().map(drop).map_err(|e| UpdateError::Start(format!("couldn't start {}: {e}", file.display())))
}

/// `~/Applications/.git-ex-machina.AppImage.update` for `~/Applications/git-ex-machina.AppImage`.
fn partial(file: &Path) -> PathBuf {
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    file.with_file_name(format!(".{name}.update"))
}

#[cfg(unix)]
fn executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn executable(_: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_download_lands_beside_the_file_it_replaces() {
        let at = partial(Path::new("/home/ana/Applications/git-ex-machina.AppImage"));
        assert_eq!(at, Path::new("/home/ana/Applications/.git-ex-machina.AppImage.update"));
    }
}
