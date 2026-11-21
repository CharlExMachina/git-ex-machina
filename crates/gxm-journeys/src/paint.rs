//! Draws the journey's own runtime at a mark, on Rune's renderer, the way a headless `--snap`
//! draws its frame: the screenshot is of the very state the journey's expectations checked, and
//! it costs one frame, where a replay of the script would draw every frame up to the mark.

use rune::Runtime;
use rune::render::{Gpu, OFFSCREEN_FORMAT, Offscreen, Renderer};

/// Screenshots are drawn at this device scale, as Retina screens show the app.
pub const SCALE: f32 = 2.0;

pub(crate) struct Painter {
    gpu: Gpu,
    renderer: Renderer,
    target: Offscreen,
}

/// A mark's screenshot: tightly packed sRGB RGBA.
#[derive(Clone, Debug)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Pixels {
    /// One colour everywhere: the renderer drew nothing.
    pub fn is_blank(&self) -> bool {
        let (px, _) = self.rgba.as_chunks::<4>();
        px.iter().all(|p| Some(p) == px.first())
    }

    pub fn write_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        rune::render::write_png(path, self.width, self.height, &self.rgba)
    }
}

impl Painter {
    /// One painter per run: its text atlas follows the runtime's from the first frame.
    pub(crate) fn new<S>(rt: &mut Runtime<S>) -> Painter {
        let gpu = Gpu::headless();
        let srgb = rt.srgb_blend();
        let format = if srgb { OFFSCREEN_FORMAT.remove_srgb_suffix() } else { OFFSCREEN_FORMAT };
        let mut renderer = Renderer::new(&gpu.device, format);
        renderer.set_srgb_blend(srgb);
        renderer.attach_text(&mut rt.text.atlas);
        let (w, h) = ((gxm_app::SIZE.0 * SCALE).round() as u32, (gxm_app::SIZE.1 * SCALE).round() as u32);
        let target = Offscreen::with_format(&gpu.device, w, h, format);
        Painter { gpu, renderer, target }
    }

    pub(crate) fn paint<S>(&mut self, rt: &mut Runtime<S>) -> Pixels {
        let (device, queue) = (&self.gpu.device, &self.gpu.queue);
        let size = (self.target.width, self.target.height);
        self.renderer.upload_images(device, queue, &mut rt.images);
        self.renderer.prepare_surfaces(device, queue, &rt.surfaces, size);
        let mut enc = device.create_command_encoder(&Default::default());
        self.renderer.prepare(device, queue, &mut rt.text.atlas, &rt.instances, size);
        self.renderer.draw(&mut enc, &self.target.view, rt.clear_color());
        self.target.copy(&mut enc);
        self.renderer.submit(queue, enc);
        Pixels { width: size.0, height: size.1, rgba: self.target.read(device) }
    }
}
