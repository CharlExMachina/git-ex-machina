//! Updating the app: git-ex-machina, installed as this system installs it (an AppImage on
//! Linux, the Mac app on macOS), finds a newer release when it starts, offers it at the sidebar's
//! foot, refuses a download that arrives damaged, installs a good one in place of itself, and
//! restarts into it with the release's notes on screen.

use std::path::PathBuf;

use gxm_app::keys;
use gxm_core::env::Bundle;
use gxm_world::World;
use gxm_world::hub::Hub;

use crate::Journey;

/// The notes the release was published with, as written on GitHub.
const NOTES: &str = "## What's new\n\n\
- **Restart to update**: new versions install themselves, and say what changed.\n\
- The history keeps its place when a fetch lands.\n\n\
## Fixes\n\n\
- Cloning into a folder that exists says so before it starts.";

/// git-ex-machina's program in a world: a script that says which version started, in the
/// world's own folder.
fn program(w: &World, version: &str) -> String {
    let started = w.root().join("started");
    format!(
        "#!/bin/sh\n# git-ex-machina {version}, as the world installs it.\necho {version} > '{}'\n",
        started.display()
    )
}

/// The demo workspace, run from 0.1.0 installed in `~/Applications`, with 0.2.0 released on
/// GitHub. Its first download arrives damaged.
fn update_world(w: &World) {
    gxm_world::fixtures::mockup(w);
    let folder = w.installed().parent().map(PathBuf::from).unwrap_or_else(|| w.root().to_path_buf());
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| panic!("make ~/Applications: {e}"));
    build::install(w, &program(w, gxm_world::VERSION));
    let bundle = Bundle::at(w.installed());
    Hub::of(w).publish("0.2.0", NOTES, &gxm_core::update::asset_name(&bundle), build::release(w, &program(w, "0.2.0")));
    Hub::of(w).damage_downloads(1);
}

pub fn update_app() -> Journey {
    let journey = Journey::new(
        "update_app",
        "Someone starts git-ex-machina, installed as their system installs it, and sees a newer version at the sidebar's foot; they download it (a damaged first download is refused, and the retry lands), read what's new, put the restart off, then restart into it.",
        update_world,
    )
    // On a fresh start the app asked GitHub, and offers the newer release in the corner, as an
    // icon that says what it does.
    .see_in(keys::UPDATE, "Update to 0.2.0")
    .not_see("Update to 0.2.0")
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
        let now = std::fs::read_to_string(build::program_in(w)).map_err(|e| e.to_string())?;
        if !now.contains("git-ex-machina 0.1.0") {
            return Err(format!("the installed program holds {now:?}"));
        }
        only_the_app_in_applications(w)
    })
    .mark("damaged")
    // The retry lands: it's installed, and the app offers to restart with the release's notes.
    .tap(keys::UPDATE);
    // The Mac app comes in a disk image, which takes a moment to open and copy from once it's
    // downloaded: the corner says so, rather than sitting at 100%.
    #[cfg(target_os = "macos")]
    let journey = journey.wait_for(keys::UPDATE, "Installing 0.2.0…").mark("installing");
    journey
        .wait_for(keys::SHEET, "git-ex-machina 0.2.0 is ready")
        .see_in(keys::SHEET, "takes over from the next start")
        .see_in(keys::UPDATE_NOTES, "What's new")
        .see_in(keys::UPDATE_NOTES, "Restart to update: new versions install themselves, and say what changed.")
        .see_in(keys::UPDATE_NOTES, "Cloning into a folder that exists says so before it starts.")
        .not_see("**")
        .gone(keys::UPDATE_ERROR)
        .world_check("the app in ~/Applications is 0.2.0 now, runs, and nothing else is left there", |w| {
            let now = std::fs::read_to_string(build::program_in(w)).map_err(|e| e.to_string())?;
            if !now.contains("git-ex-machina 0.2.0") {
                return Err(format!("the installed program holds {now:?}"));
            }
            executable(&build::program_in(w))?;
            build::intact(w)?;
            only_the_app_in_applications(w)
        })
        .mark("ready")
        // Later keeps working on the old version, with the restart a click away in the corner.
        .tap(keys::UPDATE_LATER)
        .gone(keys::SHEET)
        .see_in(keys::UPDATE, "Restart to update to 0.2.0")
        .mark("later")
        .tap(keys::UPDATE)
        .see_in(keys::SHEET, "git-ex-machina 0.2.0 is ready")
        // Restart starts the new version from the same place, then this one quits.
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

/// `~/Applications` holds the app and nothing else: no download or copy left beside it.
fn only_the_app_in_applications(w: &World) -> Result<(), String> {
    let folder = w.installed().parent().map(PathBuf::from).unwrap_or_default();
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort();
    if names == [gxm_world::INSTALLED] { Ok(()) } else { Err(format!("~/Applications holds {names:?}")) }
}

/// An AppImage: the program itself is the file installed, and the file released.
#[cfg(not(target_os = "macos"))]
mod build {
    use gxm_world::World;

    pub fn install(w: &World, program: &str) {
        std::fs::write(w.installed(), program).unwrap_or_else(|e| panic!("install 0.1.0: {e}"));
    }

    pub fn release(_: &World, program: &str) -> Vec<u8> {
        program.as_bytes().to_vec()
    }

    pub fn program_in(w: &World) -> std::path::PathBuf {
        w.installed()
    }

    pub fn intact(_: &World) -> Result<(), String> {
        Ok(())
    }
}

/// The Mac app: the program inside `git-ex-machina.app`, signed (ad hoc, as a world has no
/// Developer ID), and released in a disk image beside a link to Applications, as `cargo xtask
/// macapp` builds it.
#[cfg(target_os = "macos")]
mod build {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use gxm_world::World;

    const NAME: &str = "git-ex-machina";

    fn bundle(app: &Path, program: &str) {
        let made = (|| {
            std::fs::create_dir_all(app.join("Contents/MacOS"))?;
            let plist = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
                 <key>CFBundleExecutable</key><string>{NAME}</string>\
                 <key>CFBundleIdentifier</key><string>com.charlexmachina.GitExMachina.world</string>\
                 <key>CFBundlePackageType</key><string>APPL</string></dict></plist>\n"
            );
            std::fs::write(app.join("Contents/Info.plist"), plist)?;
            let exe = app.join("Contents/MacOS").join(NAME);
            std::fs::write(&exe, program)?;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755))
        })();
        made.unwrap_or_else(|e| panic!("lay out {}: {e}", app.display()));
        run(Command::new("codesign").args(["--force", "--sign", "-"]).arg(app));
    }

    fn run(cmd: &mut Command) {
        let out = cmd.output().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
        assert!(out.status.success(), "{cmd:?}: {}", String::from_utf8_lossy(&out.stderr));
    }

    pub fn install(w: &World, program: &str) {
        bundle(&w.installed(), program);
    }

    pub fn release(w: &World, program: &str) -> Vec<u8> {
        let stage = w.root().join("release");
        bundle(&stage.join(format!("{NAME}.app")), program);
        std::os::unix::fs::symlink("/Applications", stage.join("Applications"))
            .unwrap_or_else(|e| panic!("link Applications: {e}"));
        let dmg = w.root().join(format!("{NAME}.dmg"));
        run(Command::new("hdiutil")
            .args(["create", "-quiet", "-volname", NAME, "-fs", "HFS+", "-format", "UDZO", "-ov", "-srcfolder"])
            .arg(&stage)
            .arg(&dmg));
        std::fs::read(&dmg).unwrap_or_else(|e| panic!("read {}: {e}", dmg.display()))
    }

    pub fn program_in(w: &World) -> PathBuf {
        w.installed().join("Contents/MacOS").join(NAME)
    }

    /// The installed app's signature still holds: it was copied whole, not rebuilt.
    pub fn intact(w: &World) -> Result<(), String> {
        let out = Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(w.installed())
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_owned()) }
    }
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
