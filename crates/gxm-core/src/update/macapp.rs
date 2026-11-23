//! The Mac app updates from the release's disk image, the same one a person downloads. It
//! downloads into a folder beside the app, opens there, and the app inside is copied out whole
//! (its signature with it). The copy must be signed as the running app is: by the same developer,
//! and notarized by Apple, when the running app is; an intact signature at least, when the
//! running app is a build of one's own. Then two renames swap it in, and the old app goes with
//! the folder. The running app goes on from what it already loaded.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Release, UpdateError};

/// Downloads `release`'s disk image beside `app`, checks it, and puts the app inside it in
/// `app`'s place.
pub(super) fn install(app: &Path, release: &Release, progress: &mut dyn FnMut(u64, u64)) -> Result<(), UpdateError> {
    // Beside it, so the renames that swap it in never cross file systems.
    let stage = staging(app);
    let cant = |e: std::io::Error| UpdateError::Write(format!("couldn't make {}: {e}", stage.display()));
    // What an interrupted update left.
    std::fs::remove_dir_all(&stage).ok();
    std::fs::create_dir_all(&stage).map_err(cant)?;
    let installed = install_from(app, &stage, release, progress);
    std::fs::remove_dir_all(&stage).ok();
    installed
}

fn install_from(
    app: &Path,
    stage: &Path,
    release: &Release,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), UpdateError> {
    let image = stage.join("update.dmg");
    super::download(&image, release, progress)?;
    let disk = stage.join("disk");
    run(Command::new("hdiutil")
        .args(["attach", "-quiet", "-nobrowse", "-noautoopen", "-readonly", "-mountpoint"])
        .arg(&disk)
        .arg(&image))
    .map_err(|e| UpdateError::Write(format!("couldn't open the disk image: {e}")))?;
    let new = stage.join(app.file_name().unwrap_or("git-ex-machina.app".as_ref()));
    let copied = copy_app(&disk, &new);
    // A disk image left open only costs a mount; a stubborn one is forced.
    if run(Command::new("hdiutil").args(["detach", "-quiet"]).arg(&disk)).is_err() {
        run(Command::new("hdiutil").args(["detach", "-quiet", "-force"]).arg(&disk)).ok();
    }
    copied?;
    trust(app, &new)?;
    swap(app, &new, &stage.join("old.app"))
}

/// `/Applications/.git-ex-machina.app.update` for `/Applications/git-ex-machina.app`.
fn staging(app: &Path) -> PathBuf {
    let name = app.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    app.with_file_name(format!(".{name}.update"))
}

/// Copies the app on the disk image (beside its link to Applications) to `to`, whole.
fn copy_app(disk: &Path, to: &Path) -> Result<(), UpdateError> {
    let entries =
        std::fs::read_dir(disk).map_err(|e| UpdateError::Write(format!("couldn't read the disk image: {e}")))?;
    let app = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "app") && !p.is_symlink() && p.is_dir())
        .ok_or_else(|| UpdateError::Write("the disk image holds no app".to_owned()))?;
    run(Command::new("ditto").arg(&app).arg(to))
        .map_err(|e| UpdateError::Write(format!("couldn't copy the app from the disk image: {e}")))
}

/// Whether `new` may replace `running` (see the module's comment).
fn trust(running: &Path, new: &Path) -> Result<(), UpdateError> {
    let untrusted = |why: &str| UpdateError::Untrusted(format!("the new version {why}, so it wasn't installed"));
    run(Command::new("codesign").args(["--verify", "--deep", "--strict"]).arg(new))
        .map_err(|_| untrusted("isn't signed properly"))?;
    // `codesign -d` shows a signature on stderr, and its requirements on stdout.
    let shown = Command::new("codesign")
        .args(["-d", "--verbose=1", "-r-"])
        .arg(running)
        .output()
        .map(|o| format!("{}{}", String::from_utf8_lossy(&o.stderr), String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default();
    match signer(&shown) {
        Some(Signer::Anyone) => Ok(()),
        Some(Signer::Developer(requirement)) => {
            run(Command::new("codesign")
                .args(["--verify", "--deep", "--strict"])
                .arg(format!("-R={requirement}"))
                .arg(new))
            .map_err(|_| untrusted("isn't signed by git-ex-machina's developer"))?;
            run(Command::new("spctl").args(["--assess", "--type", "execute"]).arg(new))
                .map_err(|_| untrusted("isn't notarized by Apple"))
        }
        None => Err(UpdateError::Untrusted("couldn't read this app's signature to check the new version's".to_owned())),
    }
}

/// Who must have signed a new version, given the running app's signature as `codesign -d
/// --verbose=1 -r-` shows it.
#[derive(Debug, PartialEq, Eq)]
enum Signer<'a> {
    /// The running app is signed ad hoc, or not at all: a build of one's own.
    Anyone,
    /// The running app's developer, as its designated requirement says.
    Developer(&'a str),
}

/// None when a developer signed the running app but its requirement can't be read.
fn signer(shown: &str) -> Option<Signer<'_>> {
    let team = shown.lines().find_map(|l| l.strip_prefix("TeamIdentifier="));
    if team.is_none_or(|t| t == "not set") {
        return Some(Signer::Anyone);
    }
    shown.lines().find_map(|l| l.strip_prefix("designated => ")).map(Signer::Developer)
}

/// Moves `app` aside to `old` and `new` into its place, or puts `app` back.
fn swap(app: &Path, new: &Path, old: &Path) -> Result<(), UpdateError> {
    std::fs::rename(app, old).map_err(|e| UpdateError::Write(format!("couldn't replace {}: {e}", app.display())))?;
    std::fs::rename(new, app).map_err(|e| {
        std::fs::rename(old, app).ok();
        UpdateError::Write(format!("couldn't replace {}: {e}", app.display()))
    })
}

/// Opens `app` anew (the new version once [`install`] has put it there) beside this one, which
/// quits.
pub(super) fn restart(app: &Path) -> Result<(), UpdateError> {
    run(Command::new("open").arg("-n").arg(app))
        .map_err(|e| UpdateError::Start(format!("couldn't open {}: {e}", app.display())))
}

/// Runs `cmd`, failing with what it said.
fn run(cmd: &mut Command) -> Result<(), String> {
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    Err(if said.is_empty() { out.status.to_string() } else { said })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_stages_beside_the_app_it_replaces() {
        let at = staging(Path::new("/Applications/git-ex-machina.app"));
        assert_eq!(at, Path::new("/Applications/.git-ex-machina.app.update"));
    }

    #[test]
    fn a_new_version_must_come_from_whoever_signed_this_one() {
        let released = "Executable=/Applications/git-ex-machina.app/Contents/MacOS/git-ex-machina\n\
            Identifier=com.charlexmachina.GitExMachina\n\
            Format=app bundle with Mach-O thin (arm64)\n\
            TeamIdentifier=M8DQ92H3R6\n\
            designated => identifier \"com.charlexmachina.GitExMachina\" and anchor apple generic and certificate leaf[subject.OU] = M8DQ92H3R6\n";
        let requirement = "identifier \"com.charlexmachina.GitExMachina\" and anchor apple generic and certificate leaf[subject.OU] = M8DQ92H3R6";
        assert_eq!(signer(released), Some(Signer::Developer(requirement)));
        let ad_hoc = "Identifier=gxm\nSignature=adhoc\nTeamIdentifier=not set\n# designated => cdhash H\"94ec\"\n";
        assert_eq!(signer(ad_hoc), Some(Signer::Anyone), "a build of one's own");
        let unsigned = "/Applications/git-ex-machina.app: code object is not signed at all\n";
        assert_eq!(signer(unsigned), Some(Signer::Anyone));
        assert_eq!(
            signer("TeamIdentifier=M8DQ92H3R6\n"),
            None,
            "signed by a developer, but no requirement to hold it to"
        );
    }
}
