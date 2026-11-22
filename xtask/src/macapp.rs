//! `cargo xtask macapp`: the macOS release, as a disk image per architecture,
//! `git-ex-machina-<arch>.dmg`, holding `git-ex-machina.app` beside a link to Applications.
//!
//! Gatekeeper opens an app downloaded from the internet only when it's signed with a Developer
//! ID, with the hardened runtime and a secure timestamp, and Apple has notarized it. So both the
//! app and the disk image are signed, notarized and stapled (the ticket rides along, and the check
//! passes offline), and before it's done this tries both as Gatekeeper would (see `try_it`), so
//! the release workflow publishes nothing macOS would refuse to open.
//!
//! The signing identity is the keychain's "Developer ID Application" certificate
//! (`GXM_SIGN_IDENTITY` names another). Notarization uses the App Store Connect API key in
//! `GXM_NOTARY_KEY` (a `.p8` path), `GXM_NOTARY_KEY_ID` and `GXM_NOTARY_ISSUER` when they're set,
//! as the release workflow sets them; otherwise the `notarytool store-credentials` profile named
//! by `GXM_NOTARY_PROFILE` (default `gxm-notary`).
//!
//! The app holds its binary, `Info.plist` and icon, and nothing else: it draws with Metal and runs
//! the system's git, as a person's Mac has them.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::{build_world, release, root, run, target};

/// The binary, which also names the app and the disk image.
const BIN: &str = "git-ex-machina";
/// The bundle id, the same id the Linux desktop entry and window use.
const ID: &str = gxm_app::APP_ID;
/// The oldest macOS Rust's `aarch64-apple-darwin` builds for.
const MIN_MACOS: &str = "11.0";

/// `macapp [--tag vX.Y.Z] [--unsigned]`.
pub fn macapp(args: &[String]) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("the macOS app is built on macOS (codesign, notarytool and hdiutil are Apple's)".to_owned());
    }
    let version = release::version(args)?;
    let signed = !args.iter().any(|a| a == "--unsigned");
    let identity = setting("GXM_SIGN_IDENTITY").unwrap_or_else(|| "Developer ID Application".to_owned());
    let arch = std::env::consts::ARCH;
    let out = target().join("macapp");
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    println!("macapp: building the release binary");
    run(Command::new("cargo")
        .args(["build", "--release", "--quiet", "-p", "gxm-app", "--bin", BIN])
        .current_dir(root()))?;
    let app = out.join(format!("{BIN}.app"));
    lay_out(&app, &target().join("release").join(BIN), version, &out)?;

    let dmg = out.join(format!("{BIN}-{arch}.dmg"));
    if signed {
        println!("macapp: signing {} as {identity:?}", app.display());
        sign(&app, &identity)?;
        // The app is notarized on its own first, so the copy a person drags out of the disk image
        // carries its ticket and opens offline too.
        let zip = out.join(format!("{BIN}.zip"));
        run(Command::new("ditto").args(["-c", "-k", "--keepParent"]).arg(&app).arg(&zip))?;
        notarize(&zip)?;
        std::fs::remove_file(&zip).map_err(|e| e.to_string())?;
        run(Command::new("xcrun").args(["stapler", "staple"]).arg(&app))?;
        disk_image(&app, &dmg)?;
        sign(&dmg, &identity)?;
        notarize(&dmg)?;
        run(Command::new("xcrun").args(["stapler", "staple"]).arg(&dmg))?;
    } else {
        println!("macapp: signing {} ad hoc (--unsigned: Gatekeeper won't open a download of it)", app.display());
        run(Command::new("codesign").args(["--force", "--sign", "-"]).arg(&app))?;
        disk_image(&app, &dmg)?;
    }
    try_it(&app, &dmg, signed)?;
    println!("macapp: {} ({version})", dmg.display());
    Ok(())
}

/// `git-ex-machina.app`: the binary, `Info.plist`, and the icon.
///
/// The icon's source is `assets/macos/AppIcon.icon`, in Icon Composer's format. macOS 26 and
/// later draw only an icon compiled from that (into `Assets.car`) as a full tile; an `.icns`
/// alone they shrink into a grey frame. `actool` writes both, and older macOS reads the `.icns`.
fn lay_out(app: &Path, binary: &Path, version: &str, scratch: &Path) -> Result<(), String> {
    let contents = app.join("Contents");
    let resources = contents.join("Resources");
    for dir in [contents.join("MacOS"), resources.clone()] {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::copy(binary, contents.join("MacOS").join(BIN)).map_err(|e| format!("copy {}: {e}", binary.display()))?;
    std::fs::write(contents.join("Info.plist"), info_plist(version)).map_err(|e| e.to_string())?;
    run(Command::new("plutil").arg("-lint").arg(contents.join("Info.plist")).stdout(Stdio::null()))?;
    run(Command::new("xcrun")
        .arg("actool")
        .arg(root().join("assets/macos/AppIcon.icon"))
        .arg("--compile")
        .arg(&resources)
        .args(["--platform", "macosx", "--minimum-deployment-target", MIN_MACOS, "--app-icon", "AppIcon"])
        .arg("--output-partial-info-plist")
        .arg(scratch.join("icon.plist"))
        .stdout(Stdio::null()))?;
    for made in ["Assets.car", "AppIcon.icns"] {
        if !resources.join(made).exists() {
            return Err(format!("actool made no {made} from assets/macos/AppIcon.icon (it needs Xcode 26 or later)"));
        }
    }
    // The keys actool asks for are already in `info_plist`.
    std::fs::remove_file(scratch.join("icon.plist")).map_err(|e| e.to_string())
}

fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>{BIN}</string>
	<key>CFBundleExecutable</key>
	<string>{BIN}</string>
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
	<key>CFBundleIconName</key>
	<string>AppIcon</string>
	<key>CFBundleIdentifier</key>
	<string>{ID}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>{BIN}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
	<key>LSMinimumSystemVersion</key>
	<string>{MIN_MACOS}</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSSupportsAutomaticGraphicsSwitching</key>
	<true/>
</dict>
</plist>
"#
    )
}

/// Signs with the hardened runtime and a secure timestamp, both of which notarization requires.
fn sign(path: &Path, identity: &str) -> Result<(), String> {
    run(Command::new("codesign").args(["--force", "--options", "runtime", "--timestamp", "--sign", identity]).arg(path))
}

/// Sends `file` to Apple's notary service and waits for its verdict. A rejection prints Apple's
/// log, which names each problem.
fn notarize(file: &Path) -> Result<(), String> {
    println!("macapp: notarizing {} (Apple usually answers within minutes)", file.display());
    let mut cmd = Command::new("xcrun");
    cmd.args(["notarytool", "submit"]).arg(file).args(["--wait", "--output-format", "json"]);
    notary_credentials(&mut cmd);
    let out = cmd.output().map_err(|e| format!("notarytool: {e}"))?;
    let said = String::from_utf8_lossy(&out.stdout);
    if json_field(&said, "status") == Some("Accepted") {
        return Ok(());
    }
    let log = json_field(&said, "id").map_or_else(String::new, |id| {
        let mut cmd = Command::new("xcrun");
        cmd.args(["notarytool", "log", id]);
        notary_credentials(&mut cmd);
        cmd.output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
    });
    Err(format!("Apple didn't notarize {}.\n{said}{}\n{log}", file.display(), String::from_utf8_lossy(&out.stderr)))
}

fn notary_credentials(cmd: &mut Command) {
    match (setting("GXM_NOTARY_KEY"), setting("GXM_NOTARY_KEY_ID"), setting("GXM_NOTARY_ISSUER")) {
        (Some(key), Some(id), Some(issuer)) => {
            cmd.args(["--key", &key, "--key-id", &id, "--issuer", &issuer]);
        }
        _ => {
            let profile = setting("GXM_NOTARY_PROFILE").unwrap_or_else(|| "gxm-notary".to_owned());
            cmd.args(["--keychain-profile", &profile]);
        }
    }
}

/// A signing or notarization setting from the environment, as the release workflow passes them.
fn setting(name: &str) -> Option<String> {
    #[expect(clippy::disallowed_methods, reason = "the release's signing settings are xtask's, not the app's")]
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// A string field of notarytool's one-line JSON.
fn json_field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let rest = json.split(&format!("\"{key}\":")).nth(1)?.trim_start().strip_prefix('"')?;
    rest.split('"').next()
}

/// The disk image a person opens: the app beside a link to Applications, to drag it onto.
fn disk_image(app: &Path, dmg: &Path) -> Result<(), String> {
    let stage = dmg.with_extension("stage");
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    run(Command::new("ditto").arg(app).arg(stage.join(format!("{BIN}.app"))))?;
    std::os::unix::fs::symlink("/Applications", stage.join("Applications")).map_err(|e| e.to_string())?;
    run(Command::new("hdiutil")
        .args(["create", "-quiet", "-volname", BIN, "-fs", "HFS+", "-format", "UDZO", "-ov", "-srcfolder"])
        .arg(&stage)
        .arg(dmg))?;
    std::fs::remove_dir_all(&stage).map_err(|e| e.to_string())
}

/// Checks the app and disk image as a person's Mac would, so a broken one never gets released:
/// signed with the hardened runtime, accepted by Gatekeeper as notarized, the tickets stapled,
/// and the app from inside the disk image starts.
fn try_it(app: &Path, dmg: &Path, signed: bool) -> Result<(), String> {
    println!("macapp: trying {}", dmg.display());
    let said = output(Command::new("codesign").args(["--verify", "--strict", "--verbose=2"]).arg(app))?;
    if !said.contains("satisfies its Designated Requirement") {
        return Err(format!("{} isn't validly signed:\n{said}", app.display()));
    }
    if signed {
        let details = output(Command::new("codesign").args(["--display", "--verbose=4"]).arg(app))?;
        for (want, why) in [
            ("(runtime)", "the hardened runtime"),
            ("Authority=Developer ID Application", "a Developer ID"),
            ("Timestamp=", "a secure timestamp"),
        ] {
            if !details.contains(want) {
                return Err(format!("{} isn't signed with {why}:\n{details}", app.display()));
            }
        }
        for (path, kind) in [(app, "execute"), (dmg, "open")] {
            let mut cmd = Command::new("spctl");
            cmd.args(["--assess", "--verbose=2", "--type", kind]);
            if kind == "open" {
                cmd.args(["--context", "context:primary-signature"]);
            }
            let said = output(cmd.arg(path))?;
            if !said.contains("accepted") || !said.contains("Notarized Developer ID") {
                return Err(format!("Gatekeeper wouldn't open {}:\n{said}", path.display()));
            }
            run(Command::new("xcrun").args(["stapler", "validate", "-q"]).arg(path))?;
        }
    }
    let mount = dmg.with_extension("mount");
    std::fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    run(Command::new("hdiutil")
        .args(["attach", "-quiet", "-nobrowse", "-readonly", "-mountpoint"])
        .arg(&mount)
        .arg(dmg))?;
    let started = start(&mount.join(format!("{BIN}.app/Contents/MacOS/{BIN}")));
    run(Command::new("hdiutil").args(["detach", "-quiet"]).arg(&mount))?;
    std::fs::remove_dir(&mount).map_err(|e| e.to_string())?;
    started?;
    println!(
        "macapp: {}it starts and keeps running",
        if signed { "signed, notarized, stapled, Gatekeeper accepts it, and " } else { "" }
    );
    Ok(())
}

/// Opens the app's window in the demo world (a release build has no headless rendering) and
/// checks it's still running a few seconds later: past loading, its first frames, and the reads
/// that follow them, where a broken bundle or a missing library would have stopped it.
fn start(binary: &Path) -> Result<(), String> {
    let world = build_world("mockup")?;
    let mut child = Command::new(binary)
        .envs(world.env().child_vars())
        .current_dir(world.code())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    std::thread::sleep(std::time::Duration::from_secs(8));
    if child.try_wait().map_err(|e| e.to_string())?.is_none() {
        child.kill().ok();
        child.wait().ok();
        return Ok(());
    }
    let said = child.wait_with_output().map_err(|e| e.to_string())?;
    Err(format!(
        "{} stopped by itself ({}). It said:\n{}{}",
        binary.display(),
        said.status,
        String::from_utf8_lossy(&said.stdout),
        String::from_utf8_lossy(&said.stderr)
    ))
}

/// What a command printed, both streams (codesign and spctl answer on stderr).
fn output(cmd: &mut Command) -> Result<String, String> {
    let shown = format!("{cmd:?}");
    let out = cmd.output().map_err(|e| format!("{shown}: {e}"))?;
    Ok(format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notarytool_says_whether_it_accepted_and_which_submission_it_was() {
        let said =
            r#"{"status":"Accepted","message":"Processing complete","id":"2efe2717-52ef-43a5-96dc-0797e4ca1041"}"#;
        assert_eq!(json_field(said, "status"), Some("Accepted"));
        assert_eq!(json_field(said, "id"), Some("2efe2717-52ef-43a5-96dc-0797e4ca1041"));
        assert_eq!(json_field(r#"{"status": "Invalid"}"#, "status"), Some("Invalid"));
        assert_eq!(json_field("Error: HTTP status code: 403", "status"), None);
    }

    #[test]
    fn the_bundle_names_its_binary_icon_and_version() {
        let plist = info_plist("1.2.3");
        for want in [
            format!("<key>CFBundleExecutable</key>\n\t<string>{BIN}</string>"),
            format!("<key>CFBundleIdentifier</key>\n\t<string>{ID}</string>"),
            "<key>CFBundleShortVersionString</key>\n\t<string>1.2.3</string>".to_owned(),
            "<key>CFBundleIconFile</key>\n\t<string>AppIcon</string>".to_owned(),
            "<key>CFBundleIconName</key>\n\t<string>AppIcon</string>".to_owned(),
        ] {
            assert!(plist.contains(&want), "Info.plist lacks {want}:\n{plist}");
        }
    }
}
