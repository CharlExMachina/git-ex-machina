//! `cargo xtask journey NAME [--video]`: runs a journey headless (the same run its test does)
//! and writes its proof: a screenshot and element tree at every mark, drawn from the journey's
//! own runtime. With `--video` it also replays the run through the app binary into a keycapped
//! mp4, plus a GIF small enough to embed in a PR.

use std::process::Command;

use gxm_journeys::{catalog, run as run_journey, write_proof};

use crate::{build_world, build_world_in, cargo_bin, proof_dir, run};

/// Runs the replay, doing what the person did outside the app (`Step::Outside`) to its world when
/// the app announces that step's mark (`rune-mark outside-N`, on stdout in a `--realtime` run).
/// A replay of several windows (`--desktop`) announces no marks, so there the app writes its
/// clock to `clock` (`GXM_CLOCK_FILE`), and each step happens when that reaches its mark.
fn replay_with_outside(
    c: &mut Command,
    j: &gxm_journeys::Journey,
    world: &gxm_world::World,
    recording: &gxm_journeys::Recording,
    clock: Option<&std::path::Path>,
) -> Result<(), String> {
    use std::io::BufRead as _;
    let shown = format!("{c:?}");
    let mut child = c.spawn().map_err(|e| format!("{shown}: {e}"))?;
    let acts = j.outside_acts();
    if let Some(clock) = clock {
        // The replay of several windows runs slower than the clock on a software renderer, so
        // each step waits for the app's own clock to reach its mark.
        let started = std::time::Instant::now();
        for (n, act) in acts.iter().enumerate() {
            let name = gxm_journeys::outside_mark(n);
            let at = recording.events.iter().find(|e| e["type"] == "mark" && e["name"] == name.as_str());
            let at = at.and_then(|e| e["t"].as_f64()).unwrap_or_default();
            loop {
                let now = std::fs::read_to_string(clock).ok().and_then(|t| t.trim().parse::<f64>().ok());
                if now.is_some_and(|now| now >= at) {
                    break;
                }
                if child.try_wait().map_err(|e| format!("{shown}: {e}"))?.is_some() {
                    return Err(format!("{shown} ended before {name}, at {at}s"));
                }
                if started.elapsed() > std::time::Duration::from_secs(600) {
                    return Err(format!("{shown} never reached {name}, at {at}s"));
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            act(world);
        }
    }
    if let Some(out) = child.stdout.take() {
        for line in std::io::BufReader::new(out).lines() {
            let line = line.map_err(|e| format!("{shown}: {e}"))?;
            let mark = line.strip_prefix("rune-mark ").and_then(|m| m.split(' ').next()).unwrap_or_default();
            if let Some(act) = (0..acts.len()).find(|n| gxm_journeys::outside_mark(*n) == mark).map(|n| acts[n]) {
                act(world);
            }
        }
    }
    let status = child.wait().map_err(|e| format!("{shown}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{shown} failed: {status}")) }
}

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
        // Files the journey drops are named from the world's `~/code`.
        c.current_dir(replay.code());
        c.envs(replay.env().child_vars()).arg("--script").arg(dir.join("script.json"));
        c.args(["--theme", theme, "--realtime"]).stdout(std::process::Stdio::piped());
        if recording.windows {
            // Another window showed: record the desktop, every window as it stood, the main
            // window filling the frame and the others over it, as a screen recording shows them.
            let backdrop = crate::target().join("desktop-backdrop.png");
            let (w, h) = (gxm_app::SIZE.0 as u32, gxm_app::SIZE.1 as u32);
            run(Command::new("ffmpeg")
                .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i", &format!("color=c=0x101114:s={w}x{h}")])
                .args(["-frames:v", "1"])
                .arg(&backdrop))?;
            c.arg("--desktop").arg(&backdrop).args(["--size", &format!("{w}x{h}")]);
        }
        c.arg("--video").arg(&mp4).args(["--keys", "--pointer", "--scale", "1", "--fps", "30"]);
        let clock = recording.windows.then(|| dir.join("clock"));
        if let Some(clock) = &clock {
            std::fs::remove_file(clock).ok();
            c.env(gxm_core::env::CLOCK_FILE_VAR, clock);
        }
        replay_with_outside(&mut c, &j, &replay, &recording, clock.as_deref())?;
        if let Some(clock) = &clock {
            std::fs::remove_file(clock).ok();
        }
        drop(replay);
        let gif = dir.join(format!("{name}.gif"));
        let filter = "fps=12,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4";
        run(Command::new("ffmpeg").args(["-loglevel", "error", "-y", "-i"]).arg(&mp4).args(["-vf", filter]).arg(&gif))?;
        println!("{}\n{}", mp4.display(), gif.display());
    }
    println!("{}", dir.join("script.json").display());
    Ok(())
}
