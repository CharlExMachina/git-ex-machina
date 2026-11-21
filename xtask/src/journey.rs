//! `cargo xtask journey NAME [--video]`: runs a journey headless (the same run its test does)
//! and writes its proof: a screenshot and element tree at every mark, drawn from the journey's
//! own runtime. With `--video` it also replays the run through the app binary into a keycapped
//! mp4, plus a GIF small enough to embed in a PR.

use std::process::Command;

use gxm_journeys::{catalog, run as run_journey, write_proof};

use crate::{build_world, build_world_in, cargo_bin, proof_dir, run};

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
    let recording = run_journey(&j, &world).map_err(|f| f.to_string())?;
    for png in write_proof(&recording, &dir)? {
        println!("{}", png.display());
    }
    if video {
        let bin = cargo_bin("gxm-app", "git-ex-machina")?;
        let theme = if j.dark { "dark" } else { "light" };
        // The replay's git operations change its world (a push moves the remote), so it gets a
        // freshly built one, which also serves its GitHub until the app exits. The app reads off
        // its UI thread in real time, so the replay keeps pace with the wall clock as the journey
        // did (`--realtime`), which announces each mark on stdout.
        let replay = build_world_in(name, &crate::target().join("worlds").join(format!("{name}-video")))?;
        let mp4 = dir.join(format!("{name}.mp4"));
        let mut c = Command::new(&bin);
        c.envs(replay.env().child_vars()).arg("--script").arg(dir.join("script.json"));
        c.args(["--theme", theme, "--realtime"]).stdout(std::process::Stdio::null());
        run(c.arg("--video").arg(&mp4).args(["--keys", "--pointer", "--scale", "1", "--fps", "30"]))?;
        drop(replay);
        let gif = dir.join(format!("{name}.gif"));
        let filter = "fps=12,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4";
        run(Command::new("ffmpeg").args(["-loglevel", "error", "-y", "-i"]).arg(&mp4).args(["-vf", filter]).arg(&gif))?;
        println!("{}\n{}", mp4.display(), gif.display());
    }
    println!("{}", dir.join("script.json").display());
    Ok(())
}
