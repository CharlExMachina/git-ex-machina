//! `cargo xtask appimage`: the Linux release, as one AppImage per architecture,
//! `git-ex-machina-<arch>.AppImage`. The release workflow runs it on a tag, and the app's
//! updater looks for exactly that name on GitHub's latest release (`gxm_core::update`).
//!
//! Before it's done, it tries the AppImage it made (see `try_it`), so the release workflow
//! publishes nothing that doesn't start.
//!
//! The AppImage holds the release binary, its desktop entry and icons, and nothing else: the app
//! draws with the system's Vulkan and Wayland/X11 libraries and runs the system's git, as a
//! person's machine has them. It's built with a pinned appimagetool and runtime, each checked
//! against its SHA-256, so a release can't pick up a changed tool.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{root, run, target};

/// The appimagetool and the runtime it prepends (the static type-2 runtime, which mounts with
/// FUSE 3 and needs no libfuse2 on the person's machine), for each architecture we release.
struct Tools {
    arch: &'static str,
    appimagetool: (&'static str, &'static str),
    runtime: (&'static str, &'static str),
}

const TOOLS: &[Tools] = &[Tools {
    arch: "x86_64",
    appimagetool: (
        "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage",
        "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0",
    ),
    runtime: (
        "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64",
        "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d",
    ),
}];

/// The icon sizes in `assets/icon/`, each installed into the hicolor theme.
const ICON_SIZES: &[u32] = &[16, 32, 48, 64, 128, 256, 512, 1024];

const APP: &str = "git-ex-machina";

/// `appimage [--tag vX.Y.Z]`. With a tag (as the release workflow passes it), the tag must name
/// this version and its notes must be written.
pub fn appimage(args: &[String]) -> Result<(), String> {
    let version = env!("CARGO_PKG_VERSION");
    if let Some(tag) = flag(args, "--tag") {
        if tag != format!("v{version}") {
            return Err(format!("the tag {tag} isn't this version, v{version}: bump the workspace version first"));
        }
        notes(&tag)?;
    }
    let arch = std::env::consts::ARCH;
    let tools = TOOLS.iter().find(|t| t.arch == arch).ok_or_else(|| format!("no AppImage tools pinned for {arch}"))?;
    let out = target().join("appimage");
    let tool_dir = out.join("tools");
    std::fs::create_dir_all(&tool_dir).map_err(|e| e.to_string())?;
    let appimagetool = fetch(&tool_dir, tools.appimagetool)?;
    let runtime = fetch(&tool_dir, tools.runtime)?;

    println!("appimage: building the release binary");
    run(Command::new("cargo")
        .args(["build", "--release", "--quiet", "-p", "gxm-app", "--bin", APP])
        .current_dir(root()))?;

    let dir = out.join("AppDir");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    lay_out(&dir, &target().join("release").join(APP))?;

    let file = out.join(format!("{APP}-{arch}.AppImage"));
    println!("appimage: packing {}", file.display());
    run(Command::new(&appimagetool)
        .arg("--no-appstream")
        .arg("--runtime-file")
        .arg(&runtime)
        .arg(&dir)
        .arg(&file)
        .env("ARCH", arch)
        // CI's runners have no FUSE: appimagetool runs from its own extracted copy.
        .env("APPIMAGE_EXTRACT_AND_RUN", "1")
        .env("VERSION", version))?;
    try_it(&file, &out.join("check"))?;
    println!("appimage: {} ({version})", file.display());
    Ok(())
}

/// The AppDir: `AppRun`, the binary, the desktop entry and icon at the top (where AppImage
/// tools look) and under `usr/share` (where desktop integration copies them from).
fn lay_out(dir: &Path, binary: &Path) -> Result<(), String> {
    let assets = root().join("assets");
    let copy = |from: &Path, to: &Path| -> Result<(), String> {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::copy(from, to).map(drop).map_err(|e| format!("copy {} to {}: {e}", from.display(), to.display()))
    };
    copy(binary, &dir.join("usr/bin").join(APP))?;
    let desktop = assets.join(format!("linux/{APP}.desktop"));
    copy(&desktop, &dir.join(format!("{APP}.desktop")))?;
    copy(&desktop, &dir.join(format!("usr/share/applications/{APP}.desktop")))?;
    for size in ICON_SIZES {
        let icon = assets.join(format!("icon/{APP}-{size}.png"));
        copy(&icon, &dir.join(format!("usr/share/icons/hicolor/{size}x{size}/apps/{APP}.png")))?;
    }
    let icon = assets.join(format!("icon/{APP}-256.png"));
    copy(&icon, &dir.join(format!("{APP}.png")))?;
    copy(&icon, &dir.join(".DirIcon"))?;
    let app_run = dir.join("AppRun");
    std::fs::write(
        &app_run,
        format!("#!/bin/sh\nHERE=\"$(dirname \"$(readlink -f \"$0\")\")\"\nexec \"$HERE/usr/bin/{APP}\" \"$@\"\n"),
    )
    .map_err(|e| e.to_string())?;
    executable(&app_run)
}

/// Opens the AppImage as a person's machine would, so a broken one never gets released: what it
/// holds is all there, and it starts. With no display to open a window on, a good AppImage
/// mounts, its binary loads, and it stops only where it would open the window.
fn try_it(file: &Path, dir: &Path) -> Result<(), String> {
    println!("appimage: trying {}", file.display());
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let extracted = Command::new(file)
        .arg("--appimage-extract")
        .current_dir(dir)
        .output()
        .map_err(|e| format!("{} --appimage-extract: {e}", file.display()))?;
    if !extracted.status.success() {
        return Err(format!("{} doesn't extract: {}", file.display(), String::from_utf8_lossy(&extracted.stderr)));
    }
    let root = dir.join("squashfs-root");
    let wanted = [
        "AppRun".to_owned(),
        ".DirIcon".to_owned(),
        format!("{APP}.desktop"),
        format!("{APP}.png"),
        format!("usr/bin/{APP}"),
        format!("usr/share/icons/hicolor/256x256/apps/{APP}.png"),
    ];
    let missing: Vec<&String> = wanted.iter().filter(|w| !root.join(w).exists()).collect();
    if !missing.is_empty() {
        return Err(format!("the AppImage is missing {missing:?}"));
    }
    let mut child = Command::new(file)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("DISPLAY")
        // CI's runners have no FUSE; the runtime runs it from an extracted copy instead.
        .env("APPIMAGE_EXTRACT_AND_RUN", "1")
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", file.display()))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        if std::time::Instant::now() > deadline {
            child.kill().ok();
            return Err(format!("{} didn't stop within a minute with no display", file.display()));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let said = child.wait_with_output().map_err(|e| e.to_string())?;
    let said = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
    // winit's words for "no display": everything before opening a window worked.
    if !said.contains("neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set") {
        return Err(format!("{} didn't start as far as opening its window. It said:\n{said}", file.display()));
    }
    println!("appimage: it extracts, holds what it should, and starts");
    Ok(())
}

/// The release notes for `tag`, which the app shows when it offers the update.
pub fn notes(tag: &str) -> Result<PathBuf, String> {
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

/// Downloads a pinned file into `dir` (once) and checks its SHA-256.
fn fetch(dir: &Path, (url, sha256): (&str, &str)) -> Result<PathBuf, String> {
    let name = url.rsplit('/').next().unwrap_or("tool");
    let path = dir.join(name);
    if !path.exists() || digest(&path)? != sha256 {
        println!("appimage: downloading {url}");
        let part = path.with_extension("part");
        run(Command::new("curl").args(["-fsSL", "-o"]).arg(&part).arg(url))?;
        let got = digest(&part)?;
        if got != sha256 {
            std::fs::remove_file(&part).ok();
            return Err(format!("{url} has SHA-256 {got}, not the pinned {sha256}"));
        }
        std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
    }
    executable(&path)?;
    Ok(path)
}

fn digest(path: &Path) -> Result<String, String> {
    let out = Command::new("sha256sum").arg(path).output().map_err(|e| format!("sha256sum: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| format!("sha256sum {} said nothing", path.display()))
}

fn executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}
