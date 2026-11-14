//! `cargo xtask journey NAME [--video]`: runs a journey headless (the same run its test does),
//! then replays its recording through the app binary to render proof: a screenshot at every
//! mark, and with `--video` a keycapped mp4 plus a GIF small enough to embed in a PR.

use std::process::Command;

use gxm_journeys::{catalog, run as run_journey};

use crate::{build_world, cargo_bin, proof_dir, run};

pub fn list() -> Result<(), String> {
    for j in catalog::all() {
        println!("{:<20} {}", j.name, j.story);
    }
    Ok(())
}

pub fn journey(args: &[String]) -> Result<(), String> {
    let name =
        args.iter().find(|a| !a.starts_with("--")).ok_or("journey takes a NAME (cargo xtask journeys lists them)")?;
    let video = args.iter().any(|a| a == "--video");
    let j =
        catalog::find(name).ok_or_else(|| format!("no journey named {name:?} (cargo xtask journeys lists them)"))?;
    let world = build_world(name)?;
    let dir = proof_dir().join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let recording = run_journey(&j, &world).map_err(|f| f.to_string())?;
    let script = dir.join("script.json");
    let text = serde_json::to_string_pretty(&recording.script()).map_err(|e| e.to_string())?;
    std::fs::write(&script, text).map_err(|e| e.to_string())?;

    let bin = cargo_bin("gxm-app", "git-ex-machina")?;
    let theme = if j.dark { "dark" } else { "light" };
    // Each replay runs the script from the start and its git operations change the world (a
    // push moves the remote), so every replay gets a freshly built world.
    let app = || -> Result<Command, String> {
        let fresh = build_world(name)?;
        let mut c = Command::new(&bin);
        c.envs(fresh.env().child_vars()).arg("--script").arg(&script).args(["--theme", theme]);
        Ok(c)
    };
    for (i, (mark, _)) in recording.marks.iter().enumerate() {
        let png = dir.join(format!("{:02}-{mark}.png", i + 1));
        run(app()?.arg("--snap").arg(&png).args(["--scale", "2", "--at", &format!("mark:{mark}")]))?;
        run(app()?.arg("--tree").arg(png.with_extension("json")).args(["--at", &format!("mark:{mark}")]))?;
        println!("{}", png.display());
    }
    if video {
        let mp4 = dir.join(format!("{name}.mp4"));
        run(app()?.arg("--video").arg(&mp4).args(["--keys", "--pointer", "--scale", "1", "--fps", "30"]))?;
        let gif = dir.join(format!("{name}.gif"));
        let filter = "fps=12,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4";
        run(Command::new("ffmpeg").args(["-loglevel", "error", "-y", "-i"]).arg(&mp4).args(["-vf", filter]).arg(&gif))?;
        println!("{}\n{}", mp4.display(), gif.display());
    }
    println!("{}", script.display());
    Ok(())
}
