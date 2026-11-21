//! `cargo xtask journey NAME [--video]`: runs a journey headless (the same run its test does),
//! then replays its recording through the app binary to render proof: a screenshot at every
//! mark, and with `--video` a keycapped mp4 plus a GIF small enough to embed in a PR.

use std::path::{Path, PathBuf};
use std::process::Command;

use gxm_journeys::{MARK_EVERY, Mark, catalog, run as run_journey};

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
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let recording = run_journey(&j, &world).map_err(|f| f.to_string())?;
    let script = dir.join("script.json");
    let text = serde_json::to_string_pretty(&recording.script()).map_err(|e| e.to_string())?;
    std::fs::write(&script, text).map_err(|e| e.to_string())?;
    let shots: Vec<PathBuf> =
        recording.marks.iter().enumerate().map(|(i, m)| dir.join(format!("{:02}-{}.png", i + 1, m.name))).collect();
    for (m, png) in recording.marks.iter().zip(&shots) {
        let text = serde_json::to_string_pretty(&m.tree).map_err(|e| e.to_string())?;
        std::fs::write(png.with_extension("json"), text).map_err(|e| e.to_string())?;
    }

    let bin = cargo_bin("gxm-app", "git-ex-machina")?;
    let theme = if j.dark { "dark" } else { "light" };
    // A replay's git operations change its world (a push moves the remote), so each replay gets
    // a freshly built one. The app reads off its UI thread in real time, so replays keep pace
    // with the wall clock as the journey did (`--realtime`). A world also serves its GitHub, so
    // each replay keeps its world until the app exits.
    let app = |replay: &str| -> Result<(Command, gxm_world::World), String> {
        let fresh = build_world_in(name, &crate::target().join("worlds").join(format!("{name}-{replay}")))?;
        let mut c = Command::new(&bin);
        c.envs(fresh.env().child_vars()).arg("--script").arg(&script).args(["--theme", theme, "--realtime"]);
        // `--realtime` announces each mark on stdout; the renders are what matter here.
        c.stdout(std::process::Stdio::null());
        Ok((c, fresh))
    };
    // Rendering is the slow part (on CI, a CPU rasterizer draws every frame), so the replays
    // run side by side: one for every mark's screenshot, one for the video.
    let frames = dir.join("frames");
    let marks = recording.marks.first().zip(recording.marks.last()).map(|ends| app("marks").map(|r| (ends, r)));
    let marks = marks.transpose()?.map(|((first, last), (mut c, world))| {
        c.arg("--out").arg(&frames).args(["--scale", "2", "--every", &MARK_EVERY.to_string()]);
        c.args(["--start", &format!("mark:{}", first.name), "--frames", &(last.frame - first.frame + 1).to_string()]);
        (c, world)
    });
    let mp4 = dir.join(format!("{name}.mp4"));
    let clip = video.then(|| app("video")).transpose()?.map(|(mut c, world)| {
        c.arg("--video").arg(&mp4).args(["--keys", "--pointer", "--scale", "1", "--fps", "30"]);
        (c, world)
    });
    std::thread::scope(|scope| {
        let runs: Vec<_> = marks
            .into_iter()
            .chain(clip)
            .map(|(mut c, world)| scope.spawn(move || run(&mut c).map(|()| drop(world))))
            .collect();
        runs.into_iter().try_for_each(|r| r.join().map_err(|_| "a replay panicked".to_owned())?)
    })?;

    keep_marks(&frames, &recording.marks, &shots)?;
    for png in &shots {
        println!("{}", png.display());
    }
    if video {
        let gif = dir.join(format!("{name}.gif"));
        let filter = "fps=12,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4";
        run(Command::new("ffmpeg").args(["-loglevel", "error", "-y", "-i"]).arg(&mp4).args(["-vf", filter]).arg(&gif))?;
        println!("{}\n{}", mp4.display(), gif.display());
    }
    println!("{}", script.display());
    Ok(())
}

/// Moves each mark's frame out of the replay's dump to its screenshot's name, then drops the
/// frames between marks. Rune numbers dumped frames from `--start`, which is the first mark.
fn keep_marks(frames: &Path, marks: &[Mark], shots: &[PathBuf]) -> Result<(), String> {
    let Some(first) = marks.first() else { return Ok(()) };
    for (m, png) in marks.iter().zip(shots) {
        let dumped = frames.join(format!("frame_{:04}.png", m.frame - first.frame));
        std::fs::rename(&dumped, png)
            .map_err(|e| format!("mark {:?}: the replay left no {}: {e}", m.name, dumped.display()))?;
    }
    std::fs::remove_dir_all(frames).map_err(|e| e.to_string())
}
