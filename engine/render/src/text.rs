//! Text measurement and glyph rasterization with Inter (OFL, bundled). CSS pixel sizes: a 14px font here
//! is a 14px font in the DOM renderer, so widths and line boxes can be compared.
use ab_glyph::{Font, FontRef, Glyph, GlyphId, PxScale, ScaleFont};

pub struct Fonts {
    regular: FontRef<'static>,
    medium: FontRef<'static>,
    semibold: FontRef<'static>,
    bold: FontRef<'static>,
}

static REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");
static MEDIUM: &[u8] = include_bytes!("../fonts/Inter-Medium.ttf");
static SEMIBOLD: &[u8] = include_bytes!("../fonts/Inter-SemiBold.ttf");
static BOLD: &[u8] = include_bytes!("../fonts/Inter-Bold.ttf");

impl Fonts {
    pub fn new() -> Fonts {
        Fonts {
            regular: FontRef::try_from_slice(REGULAR).expect("Inter Regular"),
            medium: FontRef::try_from_slice(MEDIUM).expect("Inter Medium"),
            semibold: FontRef::try_from_slice(SEMIBOLD).expect("Inter SemiBold"),
            bold: FontRef::try_from_slice(BOLD).expect("Inter Bold"),
        }
    }

    /// CSS font-weight to the static face a browser would pick.
    pub fn font(&self, weight: u16) -> &FontRef<'static> {
        match weight {
            0..=450 => &self.regular,
            451..=550 => &self.medium,
            551..=640 => &self.semibold,
            _ => &self.bold,
        }
    }

    /// The ab_glyph scale for a CSS pixel size: ab_glyph's scale is ascent minus descent, not the em.
    pub fn scale(&self, font: &FontRef<'static>, px: f32) -> PxScale {
        let upem = font.units_per_em().unwrap_or(1000.0);
        PxScale::from(px * font.height_unscaled() / upem)
    }

    pub fn width(&self, text: &str, px: f32, weight: u16) -> f32 {
        let font = self.font(weight);
        let sf = font.as_scaled(self.scale(font, px));
        let mut w = 0.0;
        let mut prev: Option<GlyphId> = None;
        for c in text.chars() {
            let id = sf.glyph_id(c);
            if let Some(p) = prev {
                w += sf.kern(p, id);
            }
            w += sf.h_advance(id);
            prev = Some(id);
        }
        w
    }

    /// Greedy word wrap, the way a browser breaks a paragraph at spaces.
    pub fn wrap(&self, text: &str, px: f32, weight: u16, max_w: f32) -> Vec<String> {
        let mut lines: Vec<String> = vec![];
        let mut cur = String::new();
        for word in text.split(' ') {
            let cand = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
            if cur.is_empty() || self.width(&cand, px, weight) <= max_w + 0.01 {
                cur = cand;
            } else {
                lines.push(cur);
                cur = word.to_string();
            }
        }
        lines.push(cur);
        lines
    }

    /// Ascent and descent in CSS pixels at this size (descent is negative).
    pub fn metrics(&self, px: f32, weight: u16) -> (f32, f32) {
        let font = self.font(weight);
        let sf = font.as_scaled(self.scale(font, px));
        (sf.ascent(), sf.descent())
    }

    /// Rasterize one line of text at a baseline origin into an RGBA8 premultiplied pixmap.
    pub fn draw(&self, pm: &mut tiny_skia::Pixmap, text: &str, x: f32, baseline: f32, px: f32, weight: u16, color: tiny_skia::Color) {
        let font = self.font(weight);
        let scale = self.scale(font, px);
        let sf = font.as_scaled(scale);
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        let data = pm.data_mut();
        let (cr, cg, cb, ca) = (color.red(), color.green(), color.blue(), color.alpha());
        let mut pen = x;
        let mut prev: Option<GlyphId> = None;
        for c in text.chars() {
            let id = sf.glyph_id(c);
            if let Some(p) = prev {
                pen += sf.kern(p, id);
            }
            let glyph = Glyph { id, scale, position: ab_glyph::point(pen, baseline) };
            if let Some(og) = font.outline_glyph(glyph) {
                let b = og.px_bounds();
                og.draw(|gx, gy, cov| {
                    let px_x = b.min.x as i32 + gx as i32;
                    let px_y = b.min.y as i32 + gy as i32;
                    if px_x < 0 || px_y < 0 || px_x >= w || px_y >= h || cov <= 0.0 {
                        return;
                    }
                    let a = (cov * ca).min(1.0);
                    let i = ((px_y * w + px_x) * 4) as usize;
                    // source-over, premultiplied
                    let inv = 1.0 - a;
                    data[i] = (cr * a * 255.0 + data[i] as f32 * inv) as u8;
                    data[i + 1] = (cg * a * 255.0 + data[i + 1] as f32 * inv) as u8;
                    data[i + 2] = (cb * a * 255.0 + data[i + 2] as f32 * inv) as u8;
                    data[i + 3] = (a * 255.0 + data[i + 3] as f32 * inv) as u8;
                });
            }
            pen += sf.h_advance(id);
            prev = Some(id);
        }
    }
}

impl Default for Fonts {
    fn default() -> Self {
        Fonts::new()
    }
}
