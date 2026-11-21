//! A journey's proof on disk: a screenshot and an element tree at every mark, and the script
//! that replays the run (`cargo xtask journey NAME --video` records it).

use std::path::{Path, PathBuf};

use crate::Recording;

/// `target/proof`, where every journey's proof lands.
pub fn proof_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/proof")
}

/// Replaces `dir` with `recording`'s proof, and returns the screenshots' paths.
pub fn write_proof(recording: &Recording, dir: &Path) -> Result<Vec<PathBuf>, String> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let script = serde_json::to_string_pretty(&recording.script()).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("script.json"), script).map_err(|e| e.to_string())?;
    let mut shots = Vec::new();
    for (i, m) in recording.marks.iter().enumerate() {
        let png = dir.join(format!("{:02}-{}.png", i + 1, m.name));
        m.pixels.write_png(&png).map_err(|e| format!("{}: {e}", png.display()))?;
        let tree = serde_json::to_string_pretty(&m.tree).map_err(|e| e.to_string())?;
        std::fs::write(png.with_extension("json"), tree).map_err(|e| e.to_string())?;
        shots.push(png);
    }
    Ok(shots)
}
