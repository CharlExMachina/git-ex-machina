//! Seeing the app: headless snapshots with their element trees.

use std::path::Path;
use std::process::Command;

use crate::{build_world, cargo_bin, proof_dir, run};

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
