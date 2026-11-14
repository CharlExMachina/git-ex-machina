//! Seeing the app: headless snapshots and a side-by-side with the design.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{build_world, cargo_bin, proof_dir, root, run};

/// The design mockup's window, as rendered by headless Chrome at 1600×1000: the window sits at
/// (24, 24) and is 1552×852.
const MOCKUP: &str = "design/mockup-appshell.html";
const MOCKUP_CROP: &str = "1552:852:24:24";

pub fn snap(args: &[String]) -> Result<(), String> {
    let mut name = "mockup".to_owned();
    let mut theme = "dark".to_owned();
    let mut size = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--theme" => theme = it.next().ok_or("--theme takes dark or light")?.clone(),
            "--size" => size = Some(it.next().ok_or("--size takes WxH")?.clone()),
            n => name = n.to_owned(),
        }
    }
    let out = proof_dir().join(format!("snap-{name}-{theme}.png"));
    let tree = out.with_extension("json");
    app_snap(&name, &theme, size.as_deref(), &out, &tree)?;
    println!("{}\n{}", out.display(), tree.display());
    Ok(())
}

/// Renders the app inside world `name` to `out`, with its element tree in `tree`.
pub fn app_snap(name: &str, theme: &str, size: Option<&str>, out: &Path, tree: &Path) -> Result<(), String> {
    let w = build_world(name)?;
    let bin = cargo_bin("gxm-app", "git-ex-machina")?;
    std::fs::create_dir_all(proof_dir()).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&bin);
    cmd.envs(w.env().child_vars())
        .arg("--snap")
        .arg(out)
        .arg("--tree")
        .arg(tree)
        .args(["--theme", theme, "--scale", "1", "--at", "0.5"]);
    if let Some(s) = size {
        cmd.args(["--size", s]);
    }
    run(&mut cmd)
}

/// `compare [--theme light]`: the mockup next to the app, same size, same data, scored.
pub fn compare_cmd(args: &[String]) -> Result<(), String> {
    let light = args.windows(2).any(|w| w[0] == "--theme" && w[1] == "light");
    compare_theme(if light { "light" } else { "dark" })
}

/// Both themes, for `verify`.
pub fn compare() -> Result<(), String> {
    compare_theme("dark")?;
    compare_theme("light")
}

/// The mockup in `theme` next to the app in `theme`, same size, same data, scored.
fn compare_theme(theme: &str) -> Result<(), String> {
    let dir = proof_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let suffix = if theme == "dark" { String::new() } else { format!("-{theme}") };
    let full = dir.join(format!("mockup-full{suffix}.png"));
    let mockup = dir.join(format!("mockup{suffix}.png"));
    let app = dir.join(format!("app{suffix}.png"));
    // The mockup picks its theme from a prop that defaults to dark; a copy flips the default.
    let page = if theme == "dark" {
        root().join(MOCKUP)
    } else {
        let html = std::fs::read_to_string(root().join(MOCKUP)).map_err(|e| e.to_string())?;
        // Both the prop's declared default and the code's fallback.
        let flipped = html
            .replace("props.theme ?? 'dark'", &format!("props.theme ?? '{theme}'"))
            .replace("default&quot;:&quot;dark&quot;", &format!("default&quot;:&quot;{theme}&quot;"));
        if flipped == html {
            return Err(format!("{MOCKUP} no longer has a theme default to flip"));
        }
        let copy = dir.join(format!("mockup-{theme}.html"));
        std::fs::write(&copy, flipped).map_err(|e| e.to_string())?;
        copy
    };
    chrome_screenshot(&page, &full)?;
    run(Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(&full)
        .args(["-vf", &format!("crop={MOCKUP_CROP}")])
        .arg(&mockup))?;
    app_snap("mockup", theme, None, &app, &dir.join(format!("app{suffix}.json")))?;
    let out = dir.join(format!("compare{suffix}.png"));
    let diff = dir.join(format!("compare{suffix}-diff.png"));
    let ff = || {
        let mut c = Command::new("ffmpeg");
        c.args(["-loglevel", "error", "-y", "-i"]).arg(&mockup).arg("-i").arg(&app);
        c
    };
    // Design on the left, app on the right.
    run(ff().args(["-filter_complex", "[1:v]scale=1552:852[b];[0:v][b]hstack=inputs=2"]).arg(&out))?;
    // Black where they agree; anything bright is a difference in place, size or colour.
    run(ff()
        .args([
            "-filter_complex",
            "[0:v]format=rgb24[a];[1:v]scale=1552:852,format=rgb24[b];[a][b]blend=all_mode=difference",
        ])
        .arg(&diff))?;
    println!("{}  (design left, app right)\n{}  (difference: black = same)", out.display(), diff.display());
    let report = crate::fidelity::score(&mockup, &app, &dir, theme)?;
    println!("fidelity against {MOCKUP}, {theme}:\n{}", report.lines.join("\n"));
    if report.over.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the app drifted from the design in: {}. Look at compare.png and compare-diff.png.",
            report.over.join(", ")
        ))
    }
}

fn chrome_screenshot(page: &Path, out: &Path) -> Result<(), String> {
    let chrome = chrome().ok_or("Google Chrome (or Chromium) is needed to render the mockup")?;
    let profile = crate::target().join("chrome-profile");
    run(Command::new(chrome)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--hide-scrollbars",
            "--window-size=1600,1000",
            "--virtual-time-budget=8000",
        ])
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--screenshot={}", out.display()))
        .arg(format!("file://{}", page.display())))
}

fn chrome() -> Option<PathBuf> {
    [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}
