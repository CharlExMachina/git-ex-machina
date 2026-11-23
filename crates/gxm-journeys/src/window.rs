//! The windows a journey drives (the main one and the tag window), behind one trait, so the
//! runner works the one in front without caring which.

use std::path::PathBuf;

use rune::{ElementInfo, FileDialogKind, KeyEvent, PhysicalInput, Runtime, TitlebarAction};

/// What the runner does with a window, whichever window it is.
pub(crate) trait Window {
    fn elements(&self) -> Vec<ElementInfo>;
    fn find(&self, key: &str) -> Option<(f32, f32)>;
    fn pointer_move(&mut self, x: f32, y: f32);
    fn pointer_down(&mut self, x: f32, y: f32);
    fn pointer_up(&mut self, x: f32, y: f32);
    fn wheel(&mut self, x: f32, y: f32, dy: f32);
    fn key(&mut self, ev: KeyEvent);
    fn physical_input(&mut self, input: PhysicalInput);
    fn drop_files(&mut self, files: Vec<PathBuf>, at: Option<(f32, f32)>);
    /// The folder chooser it has up: its title and the folder it starts in.
    fn folder_chooser(&self) -> Result<(String, Option<PathBuf>), String>;
    /// Answers the file dialog it has up, as picking `path` or cancelling.
    fn answer_dialog(&mut self, path: Option<PathBuf>) -> Result<(), String>;
    /// What the last press asked of the window itself: to move it, or to zoom it.
    fn take_titlebar_action(&mut self) -> Option<TitlebarAction>;
}

/// The middle of the widest stretch along the element keyed `key`'s middle line that none of
/// its keyed descendants (its controls) cover: where a person grabs a bar to move the window.
pub(crate) fn empty_spot(w: &dyn Window, key: &str) -> Option<(f32, f32)> {
    let all = w.elements();
    let at = all.iter().position(|e| e.key.as_deref() == Some(key))?;
    let descends = |mut i: usize| loop {
        match all[i].parent {
            Some(p) if p == at => return true,
            Some(p) => i = p,
            None => return false,
        }
    };
    let controls: Vec<[f32; 4]> =
        (0..all.len()).filter(|&i| all[i].key.is_some() && descends(i)).map(|i| all[i].visible).collect();
    let [x0, y0, w0, h0] = all[at].visible;
    let y = y0 + h0 * 0.5;
    let free = |x: f32| !controls.iter().any(|r| x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3]);
    // Whole points, inset one from each end, so a bar's border isn't the spot.
    let (mut best, mut run): (Option<(f32, f32)>, Option<f32>) = (None, None);
    let mut x = x0 + 1.0;
    while x <= x0 + w0 - 1.0 {
        match (free(x), run) {
            (true, None) => run = Some(x),
            (false, Some(start)) => {
                best = best.filter(|(a, b)| b - a >= x - start).or(Some((start, x)));
                run = None;
            }
            _ => {}
        }
        x += 1.0;
    }
    if let Some(start) = run {
        best = best.filter(|(a, b)| b - a >= x - start).or(Some((start, x)));
    }
    best.map(|(a, b)| ((a + b) * 0.5, y))
}

impl<S: 'static> Window for Runtime<S> {
    fn elements(&self) -> Vec<ElementInfo> {
        Runtime::elements(self)
    }
    fn find(&self, key: &str) -> Option<(f32, f32)> {
        Runtime::find(self, key)
    }
    fn pointer_move(&mut self, x: f32, y: f32) {
        Runtime::pointer_move(self, x, y);
    }
    fn pointer_down(&mut self, x: f32, y: f32) {
        Runtime::pointer_down(self, x, y);
    }
    fn pointer_up(&mut self, x: f32, y: f32) {
        Runtime::pointer_up(self, x, y);
    }
    fn wheel(&mut self, x: f32, y: f32, dy: f32) {
        Runtime::wheel(self, x, y, dy);
    }
    fn key(&mut self, ev: KeyEvent) {
        Runtime::key(self, ev);
    }
    fn physical_input(&mut self, input: PhysicalInput) {
        Runtime::physical_input(self, input);
    }
    fn drop_files(&mut self, files: Vec<PathBuf>, at: Option<(f32, f32)>) {
        Runtime::drop_files(self, files, at);
    }
    fn folder_chooser(&self) -> Result<(String, Option<PathBuf>), String> {
        match self.file_dialog() {
            Some((FileDialogKind::Folder, options)) => Ok((options.title.clone(), options.directory.clone())),
            Some((kind, options)) => Err(format!("a {kind:?} dialog is up instead, titled {:?}", options.title)),
            None => Err("no file dialog is up".to_owned()),
        }
    }
    fn answer_dialog(&mut self, path: Option<PathBuf>) -> Result<(), String> {
        self.resolve_file_dialog(path).map_err(|e| e.to_string())
    }
    fn take_titlebar_action(&mut self) -> Option<TitlebarAction> {
        Runtime::take_titlebar_action(self)
    }
}
