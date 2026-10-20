//! Manifest renderer: MX text in, layout boxes, diagnostics, perception and pixels out, with no browser.
pub mod draw;
pub mod flow;
pub mod host;
pub mod layout;
pub mod space;
pub mod text;
pub mod theme;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

use mx::perceive::PerceiveOptions;
use mx::{Box as MxBox, Diag, Surface};
use std::collections::BTreeMap;

pub use layout::{Hit, SURFACE_W};
pub use theme::{Scheme, Theme};

/// A chart's data: its own, or the `series` it is bound to.
pub fn chart_data(s: &Surface, n: usize) -> Vec<(String, f64, bool)> {
    let nd = &s.nodes[n];
    match nd.args.bind.as_deref().and_then(|b| s.get(b)) {
        Some(sn) if s.nodes[sn].ty == "series" => s.nodes[sn].args.data.clone(),
        _ => nd.args.data.clone(),
    }
}

/// `=sales`, `=sales.last`, `=sales.min`, `=sales.max`, `=sales.sum`, `=sales.Jun` as text.
pub fn bind_value(s: &Surface, bind: &str) -> Option<String> {
    let (name, field) = bind.split_once('.').unwrap_or((bind, "last"));
    let sn = s.get(name)?;
    let data = &s.nodes[sn].args.data;
    let vals: Vec<f64> = data.iter().map(|d| d.1).collect();
    let v = match field {
        "last" => *vals.last()?,
        "first" => *vals.first()?,
        "min" => vals.iter().cloned().reduce(f64::min)?,
        "max" => vals.iter().cloned().reduce(f64::max)?,
        "sum" => vals.iter().sum(),
        "count" => vals.len() as f64,
        key => data.iter().find(|d| d.0 == key)?.1,
    };
    Some(layout::fmt_num(v))
}

/// Decoded images by the name an `image` node uses (a path on disk, or whatever the host registered).
pub type Images = BTreeMap<String, tiny_skia::Pixmap>;

pub struct Renderer {
    pub fonts: text::Fonts,
    pub width: f32,
    pub theme: Theme,
    pub images: Images,
    /// device pixels per CSS pixel for painting; layout and boxes stay in CSS pixels
    pub scale: f32,
}

pub struct Rendered {
    pub laid: layout::Laid,
    pub boxes: BTreeMap<String, MxBox>,
    pub diags: Vec<Diag>,
}

impl Renderer {
    pub fn new() -> Renderer {
        Renderer { fonts: text::Fonts::new(), width: SURFACE_W, theme: Theme::light(), images: Images::new(), scale: 1.0 }
    }

    /// Register a PNG (or any image tiny-skia decodes) under the name `image` nodes refer to it by.
    pub fn add_image(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let pm = tiny_skia::Pixmap::decode_png(bytes).map_err(|e| e.to_string())?;
        self.images.insert(name.to_string(), pm);
        Ok(())
    }

    /// Load every image a surface names from disk, if not registered already (a no-op on wasm, where
    /// the host registers bytes with `add_image`).
    pub fn load_images(&mut self, s: &Surface) {
        for n in &s.nodes {
            if n.ty == "image" {
                // a v1 image names its source as the label; a painted one in `href`
                let src = if n.args.pos.is_some() { n.args.href.clone() } else { n.label.clone() }.unwrap_or_default();
                if !self.images.contains_key(&src) {
                    if let Ok(d) = std::fs::read(&src) {
                        let _ = self.add_image(&src, &d);
                    }
                }
            }
        }
    }

    pub fn layout(&self, s: &Surface) -> Rendered {
        let laid = layout::layout(s, &self.fonts, self.width, &self.images);
        let boxes = laid.boxes();
        let diags = layout::diagnostics(&laid, &self.fonts, &self.theme);
        Rendered { laid, boxes, diags }
    }

    /// Perception from the engine's own boxes and diagnostics.
    pub fn observe(&self, s: &Surface, level: u8, focus: Option<&str>) -> Result<String, mx::MXError> {
        let r = self.layout(s);
        mx::perceive(s, level, PerceiveOptions { boxes: Some(&r.boxes), diags: &r.diags, rev: None, focus })
    }

    pub fn render(&self, s: &Surface) -> (Rendered, tiny_skia::Pixmap) {
        let r = self.layout(s);
        let pm = draw::render(&r.laid, s, &self.fonts, &self.theme, &self.images, self.scale);
        (r, pm)
    }

    pub fn render_png(&self, s: &Surface) -> Vec<u8> {
        self.render(s).1.encode_png().expect("png")
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Renderer::new()
    }
}
