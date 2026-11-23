//! Draws the journey's own runtime at a mark, on Rune's renderer, the way a headless `--snap`
//! draws its frame: the screenshot is of the very state the journey's expectations checked, and
//! it costs one frame, where a replay of the script would draw every frame up to the mark.

use rune::Runtime;
use rune::render::{Gpu, OFFSCREEN_FORMAT, Offscreen, Renderer};

/// Screenshots are drawn at this device scale, as Retina screens show the app.
pub const SCALE: f32 = 2.0;
/// A titled window's corner radius, and its shadow's reach and darkest alpha, in points.
const WINDOW_RADIUS: f32 = 10.0;
const SHADOW: f32 = 28.0;
const SHADOW_ALPHA: f32 = 0.5;

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

    /// `window` laid over these pixels with its top left corner at `at` (in points), as a
    /// desktop shows a window over another: corners rounded as macOS rounds a window's
    /// (10 points), and a soft shadow under it.
    pub fn composite(&mut self, window: &Pixels, at: (f32, f32)) {
        let radius = WINDOW_RADIUS * SCALE;
        let spread = SHADOW * SCALE;
        let (x0, y0) = (at.0 * SCALE, at.1 * SCALE);
        let (w, h) = (window.width as f32, window.height as f32);
        // How far a pixel's centre is outside the window's rounded rectangle (negative inside).
        let outside = |px: f32, py: f32| {
            let (cx, cy) =
                ((px - x0 - w / 2.0).abs() - (w / 2.0 - radius), (py - y0 - h / 2.0).abs() - (h / 2.0 - radius));
            let (ox, oy) = (cx.max(0.0), cy.max(0.0));
            (ox * ox + oy * oy).sqrt() + cx.max(cy).min(0.0) - radius
        };
        let left = (x0 - spread).max(0.0) as u32;
        let top = (y0 - spread / 2.0).max(0.0) as u32;
        let right = ((x0 + w + spread) as u32).min(self.width);
        let bottom = ((y0 + h + spread * 1.5) as u32).min(self.height);
        for py in top..bottom {
            for px in left..right {
                let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                let i = ((py * self.width + px) * 4) as usize;
                // The shadow falls a little below the window, fading over `spread`.
                let d = outside(fx, fy - spread / 3.0);
                if d > 0.0 {
                    let a = SHADOW_ALPHA * (1.0 - (d / spread).min(1.0)).powi(2);
                    for c in 0..3 {
                        self.rgba[i + c] = (f32::from(self.rgba[i + c]) * (1.0 - a)).round() as u8;
                    }
                }
                let cover = (0.5 - outside(fx, fy)).clamp(0.0, 1.0);
                if cover > 0.0 {
                    let (wx, wy) = ((fx - x0) as u32, (fy - y0) as u32);
                    let j = ((wy.min(window.height - 1) * window.width + wx.min(window.width - 1)) * 4) as usize;
                    for c in 0..3 {
                        let (under, over) = (f32::from(self.rgba[i + c]), f32::from(window.rgba[j + c]));
                        self.rgba[i + c] = (under + (over - under) * cover).round() as u8;
                    }
                }
            }
        }
    }

    pub fn write_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        rune::render::write_png(path, self.width, self.height, &self.rgba)
    }
}

impl Painter {
    /// One painter per window: its text atlas follows the runtime's from the first frame.
    /// `size` is the window's, in points.
    pub(crate) fn new<S>(rt: &mut Runtime<S>, size: (f32, f32)) -> Painter {
        let gpu = Gpu::headless();
        let srgb = rt.srgb_blend();
        let format = if srgb { OFFSCREEN_FORMAT.remove_srgb_suffix() } else { OFFSCREEN_FORMAT };
        let mut renderer = Renderer::new(&gpu.device, format);
        renderer.set_srgb_blend(srgb);
        renderer.attach_text(&mut rt.text.atlas);
        let (w, h) = ((size.0 * SCALE).round() as u32, (size.1 * SCALE).round() as u32);
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
