//! How closely the app matches the design: per region of the window, the share of pixels that
//! differ visibly between `mockup.png` (the design) and `app.png` (the app on the same data).
//!
//! Antialiasing alone makes glyph edges differ a little, so a perfect score isn't zero; each
//! region has a budget a little above where the app is today. The budgets only go down: lower
//! one when the app gets closer, and never raise one without the person's sign-off, since a
//! raise means the app drifted from the design. Dark and light have budgets of their own.

use std::path::Path;
use std::process::Command;

use crate::run;

const W: usize = 1552;
const H: usize = 852;
/// A channel differing by more than this is a visible difference, not antialiasing.
const VISIBLE: u8 = 48;

struct Region {
    name: &'static str,
    rect: Rect,
    /// The most of the region, in percent, that may differ.
    budget: f64,
    /// The most of the region's colour, in percent, that may be off: of its chromatic pixels
    /// (lanes, dots, tiles, accents; not grey text), the share in a colour the other image
    /// doesn't have as much of. A wrong hue anywhere shows here even when it's a few pixels.
    colour: f64,
    /// `budget` and `colour` for the light theme, whose antialiasing lands differently.
    light: (f64, f64),
}

#[derive(Clone, Copy)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

const fn r(x: usize, y: usize, w: usize, h: usize) -> Rect {
    Rect { x, y, w, h }
}

const REGIONS: &[Region] = &[
    Region { name: "sidebar", rect: r(0, 0, 249, 852), budget: 1.0, colour: 10.0, light: (1.0, 10.0) },
    Region { name: "toolbar", rect: r(249, 0, 942, 49), budget: 1.0, colour: 8.0, light: (1.0, 8.0) },
    Region { name: "summary and columns", rect: r(249, 49, 942, 74), budget: 2.0, colour: 5.0, light: (2.25, 5.0) },
    Region { name: "history", rect: r(249, 123, 942, 729), budget: 3.25, colour: 14.0, light: (3.5, 14.0) },
    Region { name: "inspector", rect: r(1191, 0, 361, 852), budget: 1.5, colour: 17.0, light: (1.5, 27.0) },
    // Small components get regions of their own: a thin graph line or a 16-point avatar is too
    // little of a big region for a wrong colour to show in its score.
    Region { name: "graph", rect: r(257, 129, 52, 723), budget: 1.0, colour: 13.0, light: (1.0, 13.0) },
    Region { name: "repository tiles", rect: r(16, 123, 26, 290), budget: 1.5, colour: 10.0, light: (1.5, 10.0) },
    Region { name: "avatars", rect: r(915, 129, 20, 723), budget: 4.5, colour: 16.0, light: (4.5, 17.0) },
    Region { name: "ref badges", rect: r(305, 189, 300, 90), budget: 9.0, colour: 35.0, light: (9.0, 36.0) },
    Region { name: "changed files", rect: r(1203, 240, 340, 130), budget: 2.0, colour: 19.0, light: (2.0, 31.0) },
];

/// Differences that are deliberate, each with its reason.
const MASKS: &[(Rect, &str)] = &[
    (r(0, 0, 72, 48), "the traffic lights: macOS draws them on the real window"),
    (r(1105, 129, 72, 723), "commit ids: git derives them, the design made them up"),
    (r(1191, 140, 361, 34), "the inspector's ids, for the same reason"),
    (r(249, 159, 942, 30), "the merge row: the design's origin/main badge moved (main ↑2 can't be)"),
    (r(249, 249, 942, 30), "the Spring row, where origin/main now is"),
    (r(1211, 380, 320, 30), "the hunk header: git's line counts, the design's are made up"),
    (r(455, 10, 36, 28), "the push badge reads 4 where the design says 2"),
    (r(303, 60, 28, 26), "the summary's ↑4"),
    (r(186, 130, 28, 28), "the sidebar's ↑4"),
];

pub struct Report {
    pub lines: Vec<String>,
    pub over: Vec<String>,
}

pub fn score(design: &Path, app: &Path, scratch: &Path, theme: &str) -> Result<Report, String> {
    let a = rgb(design, &scratch.join("design.rgb"))?;
    let b = rgb(app, &scratch.join("app.rgb"))?;
    let masked = |x: usize, y: usize| MASKS.iter().any(|(m, _)| x >= m.x && x < m.x + m.w && y >= m.y && y < m.y + m.h);
    let mut report = Report { lines: Vec::new(), over: Vec::new() };
    for region in REGIONS {
        let (mut seen, mut differ) = (0usize, 0usize);
        for y in region.rect.y..region.rect.y + region.rect.h {
            for x in region.rect.x..region.rect.x + region.rect.w {
                if masked(x, y) {
                    continue;
                }
                let i = (y * W + x) * 3;
                seen += 1;
                if (0..3).any(|c| a[i + c].abs_diff(b[i + c]) > VISIBLE) {
                    differ += 1;
                }
            }
        }
        let pct = if seen == 0 { 0.0 } else { 100.0 * differ as f64 / seen as f64 };
        let hue = colour_off(&a, &b, region.rect, &masked);
        let (budget, colour) = if theme == "light" { region.light } else { (region.budget, region.colour) };
        let ok = pct <= budget && hue <= colour;
        report.lines.push(format!(
            "  {:<22} {:>5.2}% differs (budget {:.1}%), {:>5.1}% of colour off (budget {:.0}%){}",
            region.name,
            pct,
            budget,
            hue,
            colour,
            if ok { "" } else { "  ← over" }
        ));
        if !ok {
            report.over.push(region.name.to_owned());
        }
    }
    report.lines.push("  masked: ".to_owned() + &MASKS.iter().map(|(_, why)| *why).collect::<Vec<_>>().join("; "));
    Ok(report)
}

/// Of the chromatic pixels in `rect` (channels spread over 40), the percentage that land in a
/// colour bucket (3 bits a channel) the other image has fewer of.
fn colour_off(a: &[u8], b: &[u8], rect: Rect, masked: &dyn Fn(usize, usize) -> bool) -> f64 {
    let mut hist = std::collections::HashMap::<(u8, u8, u8), (i64, i64)>::new();
    for y in rect.y..rect.y + rect.h {
        for x in rect.x..rect.x + rect.w {
            if masked(x, y) {
                continue;
            }
            let i = (y * W + x) * 3;
            for (img, side) in [(a, 0), (b, 1)] {
                let p = [img[i], img[i + 1], img[i + 2]];
                let spread = p.iter().max().unwrap_or(&0) - p.iter().min().unwrap_or(&0);
                if spread > 40 {
                    let e = hist.entry((p[0] >> 5, p[1] >> 5, p[2] >> 5)).or_default();
                    if side == 0 { e.0 += 1 } else { e.1 += 1 }
                }
            }
        }
    }
    let total: i64 = hist.values().map(|(x, y)| x + y).sum();
    let off: i64 = hist.values().map(|(x, y)| (x - y).abs()).sum();
    if total == 0 { 0.0 } else { 100.0 * off as f64 / total as f64 }
}

/// A PNG's pixels, `W × H` RGB, through ffmpeg (scaled to the design's size).
fn rgb(png: &Path, raw: &Path) -> Result<Vec<u8>, String> {
    run(Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(png)
        .args(["-vf", &format!("scale={W}:{H}"), "-f", "rawvideo", "-pix_fmt", "rgb24"])
        .arg(raw))?;
    let bytes = std::fs::read(raw).map_err(|e| e.to_string())?;
    if bytes.len() != W * H * 3 {
        return Err(format!("{} isn't {W}×{H}", png.display()));
    }
    Ok(bytes)
}
