//! Updating the app: an AppImage that finds a newer release when it starts, offers it at the
//! sidebar's foot, refuses a download that arrives damaged, installs a good one in place of
//! itself, and restarts into it with the release's notes on screen.

use gxm_app::keys;
use gxm_world::World;
use gxm_world::hub::Hub;

use crate::Journey;

/// The notes the release was published with, as written on GitHub.
const NOTES: &str = "## What's new\n\n\
- **Restart to update**: new versions install themselves, and say what changed.\n\
- The history keeps its place when a fetch lands.\n\n\
## Fixes\n\n\
- Cloning into a folder that exists says so before it starts.";

/// An AppImage in a world: a script that says which version started, in the world's own folder.
fn appimage(w: &World, version: &str) -> String {
    let started = w.root().join("started");
    format!(
        "#!/bin/sh\n# git-ex-machina {version}, as the world installs it.\necho {version} > '{}'\n",
        started.display()
    )
}

/// The demo workspace, run from an AppImage of 0.1.0 in `~/Applications`, with 0.2.0 released
/// on GitHub. Its first download arrives damaged.
fn update_world(w: &World) {
    gxm_world::fixtures::mockup(w);
    let installed = w.appimage();
    let folder = installed.parent().unwrap_or(w.root());
    std::fs::create_dir_all(folder).unwrap_or_else(|e| panic!("make ~/Applications: {e}"));
    std::fs::write(&installed, appimage(w, gxm_world::VERSION)).unwrap_or_else(|e| panic!("install 0.1.0: {e}"));
    let hub = Hub::of(w);
    hub.publish("0.2.0", NOTES, &appimage(w, "0.2.0"));
    hub.damage_downloads(1);
}

pub fn update_app() -> Journey {
    Journey::new(
        "update_app",
        "Someone starts git-ex-machina and sees a newer version at the sidebar's foot; they download it (a damaged first download is refused, and the retry lands), read what's new, put the restart off, then restart into it.",
        update_world,
    )
    // On a fresh start the app asked GitHub, and offers the newer release in the corner.
    .see_in(keys::UPDATE, "Update to 0.2.0")
    .see_in(keys::HISTORY, "3 files changed")
    .mark("available")
    // The download shows its progress as it arrives.
    .tap(keys::UPDATE)
    .wait_for(keys::UPDATE, "Downloading 0.2.0…")
    .mark("downloading")
    // A download that doesn't match GitHub's checksum is refused, and nothing is replaced.
    .wait_for(keys::UPDATE, "Retry update")
    .see_in(keys::UPDATE_ERROR, "The download was damaged on the way; try again")
    .gone(keys::SHEET)
    .world_check("the damaged download replaced nothing, and left nothing behind", |w| {
        let now = std::fs::read_to_string(w.appimage()).map_err(|e| e.to_string())?;
        let left = w.appimage().with_file_name(".git-ex-machina.AppImage.update");
        match (now.contains("0.1.0"), left.exists()) {
            (true, false) => Ok(()),
            (still, left) => Err(format!("0.1.0 still installed: {still}; partial download left: {left}")),
        }
    })
    .mark("damaged")
    // The retry lands: it's installed, and the app offers to restart with the release's notes.
    .tap(keys::UPDATE)
    .wait_for(keys::SHEET, "git-ex-machina 0.2.0 is ready")
    .see_in(keys::SHEET, "takes over from the next start")
    .see_in(keys::UPDATE_NOTES, "What's new")
    .see_in(keys::UPDATE_NOTES, "Restart to update: new versions install themselves, and say what changed.")
    .see_in(keys::UPDATE_NOTES, "Cloning into a folder that exists says so before it starts.")
    .not_see("**")
    .gone(keys::UPDATE_ERROR)
    .world_check("the AppImage in ~/Applications is 0.2.0 now, and runs", |w| {
        let now = std::fs::read_to_string(w.appimage()).map_err(|e| e.to_string())?;
        if !now.contains("git-ex-machina 0.2.0") {
            return Err(format!("the AppImage holds {now:?}"));
        }
        executable(&w.appimage())
    })
    .mark("ready")
    // Later keeps working on the old version, with the restart a click away in the corner.
    .tap(keys::UPDATE_LATER)
    .gone(keys::SHEET)
    .see_in(keys::UPDATE, "Restart to update")
    .mark("later")
    .tap(keys::UPDATE)
    .see_in(keys::SHEET, "git-ex-machina 0.2.0 is ready")
    // Restart starts the new version from the same file, then this one quits.
    .tap(keys::UPDATE_RESTART)
    .quits()
    .world_check("0.2.0 started", |w| {
        let started = w.root().join("started");
        // It starts on its own time; give it a moment.
        for _ in 0..100 {
            if let Ok(said) = std::fs::read_to_string(&started) {
                return if said.trim() == "0.2.0" { Ok(()) } else { Err(format!("{said:?} started")) };
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Err("nothing started".to_owned())
    })
}

#[cfg(unix)]
fn executable(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).map_err(|e| e.to_string())?.permissions().mode();
    if mode & 0o111 == 0o111 { Ok(()) } else { Err(format!("{} isn't executable ({mode:o})", path.display())) }
}

#[cfg(not(unix))]
fn executable(_: &std::path::Path) -> Result<(), String> {
    Ok(())
}
