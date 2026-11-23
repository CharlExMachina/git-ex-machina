//! The windows a journey drives (the main one and the tag window), behind one trait, so the
//! runner works the one in front without caring which.

use std::path::PathBuf;

use rune::{ElementInfo, FileDialogKind, KeyEvent, PhysicalInput, Runtime};

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
}
