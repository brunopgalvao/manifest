//! Rasterize a laid-out surface with tiny-skia, in either palette of styles.ts.
use crate::flow::Route;
use crate::layout::{Item, Laid, Shape};
use crate::text::Fonts;
use crate::theme::Theme;
use crate::Images;
use mx::Surface;
use tiny_skia::{Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

fn with_alpha(c: Color, a: f32) -> Color {
    Color::from_rgba(c.red(), c.green(), c.blue(), c.alpha() * a).unwrap()
}

pub fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let mut pb = PathBuilder::new();
    if r <= 0.0 {
        pb.push_rect(Rect::from_xywh(x, y, w, h).unwrap());
        return pb.finish().unwrap();
    }
    let k = 0.5523 * r;
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish().unwrap()
}

fn fill(pm: &mut Pixmap, path: &tiny_skia::Path, c: Color, t: Transform) {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    pm.fill_path(path, &p, FillRule::Winding, t, None);
}

fn stroke(pm: &mut Pixmap, path: &tiny_skia::Path, c: Color, width: f32, dash: Option<(f32, f32)>, t: Transform) {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    let mut st = Stroke { width, line_cap: LineCap::Butt, ..Default::default() };
    if let Some((on, off)) = dash {
        st.dash = tiny_skia::StrokeDash::new(vec![on, off], 0.0);
    }
    pm.stroke_path(path, &p, &st, t, None);
}

fn line(pm: &mut Pixmap, x1: f32, y1: f32, x2: f32, y2: f32, c: Color, width: f32, t: Transform) {
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    stroke(pm, &pb.finish().unwrap(), c, width, None, t);
}

fn circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, c: Color, t: Transform) {
    if let Some(p) = PathBuilder::from_circle(cx, cy, r) {
        fill(pm, &p, c, t);
    }
}

pub struct Canvas<'a> {
    pub pm: Pixmap,
    pub fonts: &'a Fonts,
    pub theme: &'a Theme,
    pub images: &'a Images,
    /// device pixels per CSS pixel; geometry is written in CSS pixels and scaled here
    pub sc: f32,
    pub t: Transform,
}

impl<'a> Canvas<'a> {
    /// Paint a nested layout (a positioned node's default look) with its own transform.
    pub fn paint_laid(&mut self, laid: &Laid, s: &Surface, t: Transform) {
        let saved = self.t;
        self.t = t;
        self.item(laid, s, laid.root);
        self.t = saved;
    }

    fn txt(&mut self, text: &str, x: f32, baseline: f32, px: f32, weight: u16, color: Color) {
        self.fonts.draw(&mut self.pm, text, x * self.sc, baseline * self.sc, px * self.sc, weight, color);
    }

    fn text_line(&mut self, text: &str, x: f32, y_top: f32, px: f32, weight: u16, lh: f32, color: Color, max_w: Option<f32>, center_in: Option<f32>) {
        let (asc, desc) = self.fonts.metrics(px, weight);
        let baseline = y_top + (lh - (asc - desc)) / 2.0 + asc;
        let mut t = text.to_string();
        if let Some(mw) = max_w {
            if self.fonts.width(&t, px, weight) > mw + 0.5 {
                let mut chars: Vec<char> = t.chars().collect();
                while !chars.is_empty() && self.fonts.width(&format!("{}…", chars.iter().collect::<String>()), px, weight) > mw {
                    chars.pop();
                }
                t = format!("{}…", chars.iter().collect::<String>());
            }
        }
        let mut xx = x;
        if let Some(cw) = center_in {
            xx = x + (cw - self.fonts.width(&t, px, weight)) / 2.0;
        }
        self.txt(&t, xx, baseline, px, weight, color);
    }

    fn item(&mut self, laid: &Laid, s: &Surface, i: usize) {
        let it: &Item = &laid.items[i];
        let (x, y, w, h) = (it.x, it.y, it.w, it.h);
        let op = if it.paint.opacity == 0.0 { 1.0 } else { it.paint.opacity };
        if w <= 0.0 || h <= 0.0 {
            for &c in &it.children {
                self.item(laid, s, c);
            }
            return;
        }
        if let Some(f) = &it.paint.fill {
            fill(&mut self.pm, &rounded(x, y, w, h, it.paint.radius), with_alpha(self.theme.color(f), op), self.t);
        }
        if let Some(st) = &it.paint.stroke {
            stroke(&mut self.pm, &rounded(x + 0.5, y + 0.5, w - 1.0, h - 1.0, (it.paint.radius - 0.5).max(0.0)), with_alpha(self.theme.color(st), op), 1.0, None, self.t);
        }
        if let Some(c) = &it.paint.bottom_line {
            line(&mut self.pm, x, y + h - 0.5, x + w, y + h - 0.5, self.theme.color(c), 1.0, self.t);
        }
        if let Some(c) = &it.paint.underline {
            line(&mut self.pm, x, y + h - 1.0, x + w, y + h - 1.0, self.theme.color(c), 2.0, self.t);
        }
        match &it.paint.shape {
            Shape::None => {}
            Shape::Check(on) => {
                let p = rounded(x + 0.5, y + 0.5, w - 1.0, h - 1.0, 3.0);
                if *on {
                    fill(&mut self.pm, &rounded(x, y, w, h, 3.5), self.theme.color("primary"), self.t);
                    let mut pb = PathBuilder::new();
                    pb.move_to(x + 3.5, y + 8.5);
                    pb.line_to(x + 6.8, y + 11.6);
                    pb.line_to(x + 12.5, y + 4.8);
                    stroke(&mut self.pm, &pb.finish().unwrap(), self.theme.color("white"), 2.0, None, self.t);
                } else {
                    stroke(&mut self.pm, &p, self.theme.color("muted"), 1.0, None, self.t);
                }
            }
            Shape::Switch(on) => {
                fill(&mut self.pm, &rounded(x, y, w, h, 10.0), self.theme.color(if *on { "primary" } else { "muted" }), self.t);
                circle(&mut self.pm, x + if *on { 26.0 } else { 10.0 }, y + 10.0, 8.0, self.theme.color("white"), self.t);
            }
            Shape::Range(frac) => {
                let cy = y + h / 2.0;
                fill(&mut self.pm, &rounded(x, cy - 2.0, w, 4.0, 2.0), self.theme.color("muted-soft"), self.t);
                fill(&mut self.pm, &rounded(x, cy - 2.0, (w * frac).max(0.0), 4.0, 2.0), self.theme.color("primary"), self.t);
                circle(&mut self.pm, x + w * frac, cy, 7.0, self.theme.color("primary"), self.t);
            }
            Shape::Chart(n) => self.chart(s, *n, x, y, w, h),
            Shape::Flow(_, fl) => {
                let sc = if fl.width > 0.0 { w / fl.width } else { 1.0 };
                let fl = fl.clone();
                for e in &fl.edges {
                    let pts: Vec<(f32, f32)> = e.points.iter().map(|(px, py)| (x + px * sc, y + py * sc)).collect();
                    let mut pb = PathBuilder::new();
                    pb.move_to(pts[0].0, pts[0].1);
                    if e.curve && pts.len() == 4 {
                        pb.cubic_to(pts[1].0, pts[1].1, pts[2].0, pts[2].1, pts[3].0, pts[3].1);
                    } else {
                        for p in &pts[1..] {
                            pb.line_to(p.0, p.1);
                        }
                    }
                    stroke(&mut self.pm, &pb.finish().unwrap(), self.theme.color("fg2"), 1.2 * sc, if e.dashed { Some((5.0 * sc, 4.0 * sc)) } else { None }, self.t);
                    // arrow head at the last point, pointing along the last segment
                    let (ex, ey) = pts[pts.len() - 1];
                    let (px, py) = pts[pts.len() - 2];
                    let (dx, dy) = (ex - px, ey - py);
                    let len = (dx * dx + dy * dy).sqrt().max(0.001);
                    let (ux, uy) = (dx / len, dy / len);
                    let sz = 8.0 * sc;
                    let mut ab = PathBuilder::new();
                    ab.move_to(ex, ey);
                    ab.line_to(ex - ux * sz - uy * sz * 0.5, ey - uy * sz + ux * sz * 0.5);
                    ab.line_to(ex - ux * sz + uy * sz * 0.5, ey - uy * sz - ux * sz * 0.5);
                    ab.close();
                    fill(&mut self.pm, &ab.finish().unwrap(), self.theme.color("fg2"), self.t);
                    if let Some((lab, lx, ly)) = &e.label {
                        let px = 11.0 * sc;
                        let tw = self.fonts.width(lab, px, 400);
                        self.txt(lab, x + lx * sc - tw / 2.0, y + ly * sc, px, 400, self.theme.color("fg2"));
                    }
                }
                for p in &fl.nodes {
                    let (fx, fy, fw, fh) = (x + p.x * sc, y + p.y * sc, p.w * sc, p.h * sc);
                    let (f, st) = match p.tone.as_deref() {
                        Some(t) if ["primary", "success", "danger", "warning", "info", "muted"].contains(&t) => (format!("{}-soft", t), t.to_string()),
                        _ => ("card".to_string(), "fg2".to_string()),
                    };
                    fill(&mut self.pm, &rounded(fx, fy, fw, fh, 8.0 * sc), self.theme.color(&f), self.t);
                    stroke(&mut self.pm, &rounded(fx + 0.6, fy + 0.6, fw - 1.2, fh - 1.2, 7.4 * sc), self.theme.color(&st), 1.2 * sc, None, self.t);
                    let px = 13.0 * sc;
                    let tw = self.fonts.width(&p.label, px, 400);
                    let (asc, desc) = self.fonts.metrics(px, 400);
                    self.txt(&p.label, fx + (fw - tw) / 2.0, fy + fh / 2.0 + (asc + desc) / 2.0, px, 400, self.theme.color("fg"));
                }
            }
            Shape::Space(_, lay) => {
                let sc = if lay.width > 0.0 { w / lay.width } else { 1.0 };
                let t = self.t.pre_concat(Transform::from_translate(x, y)).pre_scale(sc, sc);
                crate::space::paint_space(self, s, lay, t);
            }
            Shape::Image(src) => {
                if let Some(img) = self.images.get(src) {
                    let sc = if img.width() as f32 > w { w / img.width() as f32 } else { 1.0 };
                    self.pm.draw_pixmap(0, 0, img.as_ref(), &PixmapPaint::default(), Transform::from_row(sc, 0.0, 0.0, sc, x, y).post_scale(self.sc, self.sc), None);
                }
            }
        }
        if let Some(t) = &it.paint.text {
            let color = with_alpha(self.theme.color(&t.color), op);
            let inner_w = w - 2.0 * it.pad.0 - 2.0 * it.border;
            let (tx, ty) = (x + it.pad.0 + it.border, y + it.pad.1 + it.border);
            if t.nowrap {
                self.text_line(&t.text, tx, ty, t.px, t.weight, t.lh, color, Some(inner_w), if t.center { Some(inner_w) } else { None });
            } else {
                let lines = self.fonts.wrap(&t.text, t.px, t.weight, inner_w.max(1.0));
                for (k, l) in lines.iter().enumerate() {
                    self.text_line(l, tx, ty + k as f32 * t.lh, t.px, t.weight, t.lh, color, None, None);
                }
            }
        }
        for &c in &it.children {
            self.item(laid, s, c);
        }
    }

    fn chart(&mut self, s: &Surface, n: usize, x: f32, y: f32, w: f32, _h: f32) {
        let nd = &s.nodes[n];
        let data = &crate::chart_data(s, n);
        let sc = w / 640.0;
        let t = |v: f32| v * sc;
        let (bw_, bh) = (640.0, 200.0);
        if data.is_empty() {
            return;
        }
        if nd.ty == "pie" {
            let total: f64 = data.iter().map(|d| d.1.max(0.0)).sum::<f64>().max(1e-9);
            let (cx, cy, r) = (bh / 2.0, bh / 2.0, bh / 2.0 - 10.0);
            let colors = ["primary", "success", "warning", "info", "danger", "muted"];
            let mut a0 = -std::f32::consts::FRAC_PI_2;
            for (i, (k, v, hi)) in data.iter().enumerate() {
                let a1 = a0 + (v.max(0.0) / total) as f32 * std::f32::consts::PI * 2.0;
                let mut pb = PathBuilder::new();
                pb.move_to(x + t(cx), y + t(cy));
                let steps = 24;
                for st in 0..=steps {
                    let a = a0 + (a1 - a0) * st as f32 / steps as f32;
                    pb.line_to(x + t(cx + r * a.cos()), y + t(cy + r * a.sin()));
                }
                pb.close();
                let c = self.theme.color(colors[i % colors.len()]);
                fill(&mut self.pm, &pb.finish().unwrap(), with_alpha(c, if *hi { 1.0 } else { 0.8 }), self.t);
                let ly = 24.0 + i as f32 * 20.0;
                fill(&mut self.pm, &rounded(x + t(bh + 20.0), y + t(ly - 10.0), t(12.0), t(12.0), t(2.0)), c, self.t);
                let pct = ((v / total) * 100.0).round() as i64;
                let label = format!("{} {} ({}%)", k, crate::layout::fmt_num(*v), pct);
                let px = t(11.0);
                let (asc, desc) = self.fonts.metrics(px, if *hi { 600 } else { 400 });
                self.txt(&label, x + t(bh + 38.0), y + t(ly) + (asc + desc) / 2.0, px, if *hi { 600 } else { 400 }, self.theme.color(if *hi { "fg" } else { "fg2" }));
                a0 = a1;
            }
            return;
        }
        let (pad_l, pad_r, pad_t, pad_b) = (8.0, 8.0, 18.0, 22.0);
        let iw = bw_ - pad_l - pad_r;
        let ih = bh - pad_t - pad_b;
        let max = data.iter().map(|d| d.1).fold(0.0_f64, f64::max).max(0.0);
        let max = if max == 0.0 { 1.0 } else { max };
        let min = data.iter().map(|d| d.1).fold(0.0_f64, f64::min).min(0.0);
        let span = if max - min == 0.0 { 1.0 } else { max - min };
        let yv = |v: f64| -> f32 { (pad_t as f64 + ih as f64 - ((v - min) / span) * ih as f64) as f32 };
        line(&mut self.pm, x + t(pad_l), y + t(yv(0.0)), x + t(bw_ - pad_r), y + t(yv(0.0)), self.theme.color("line"), 1.0, self.t);
        let slot = iw / data.len() as f32;
        let px = t(11.0);
        if nd.ty == "bar" {
            let bw = (slot * 0.6).min(64.0);
            for (i, (k, v, hi)) in data.iter().enumerate() {
                let cx = pad_l + slot * i as f32 + slot / 2.0;
                let top = yv(*v).min(yv(0.0));
                let hh = (yv(0.0) - yv(*v)).abs().max(1.0);
                let c = if *hi { self.theme.color("primary") } else { with_alpha(self.theme.color("muted"), 0.55) };
                fill(&mut self.pm, &rounded(x + t(cx - bw / 2.0), y + t(top), t(bw), t(hh), t(3.0)), c, self.t);
                let vs = crate::layout::fmt_num(*v);
                let tw = self.fonts.width(&vs, px, 600);
                self.txt(&vs, x + t(cx) - tw / 2.0, y + t(top - 4.0), px, 600, self.theme.color("fg"));
                let kw = self.fonts.width(k, px, 400);
                self.txt(k, x + t(cx) - kw / 2.0, y + t(bh - 6.0), px, 400, self.theme.color("fg2"));
            }
        } else {
            let pts: Vec<(f32, f32)> = data.iter().enumerate().map(|(i, d)| (pad_l + slot * i as f32 + slot / 2.0, yv(d.1))).collect();
            if nd.ty == "area" {
                let mut pb = PathBuilder::new();
                pb.move_to(x + t(pts[0].0), y + t(pts[0].1));
                for p in &pts[1..] {
                    pb.line_to(x + t(p.0), y + t(p.1));
                }
                pb.line_to(x + t(pts[pts.len() - 1].0), y + t(yv(0.0)));
                pb.line_to(x + t(pts[0].0), y + t(yv(0.0)));
                pb.close();
                fill(&mut self.pm, &pb.finish().unwrap(), with_alpha(self.theme.color("primary"), 0.15), self.t);
            }
            let mut pb = PathBuilder::new();
            pb.move_to(x + t(pts[0].0), y + t(pts[0].1));
            for p in &pts[1..] {
                pb.line_to(x + t(p.0), y + t(p.1));
            }
            stroke(&mut self.pm, &pb.finish().unwrap(), self.theme.color("primary"), t(2.0), None, self.t);
            for (i, (k, v, hi)) in data.iter().enumerate() {
                let (cx, cy) = pts[i];
                circle(&mut self.pm, x + t(cx), y + t(cy), t(if *hi { 5.0 } else { 3.0 }), self.theme.color(if *hi { "danger" } else { "primary" }), self.t);
                let vs = crate::layout::fmt_num(*v);
                let tw = self.fonts.width(&vs, px, 600);
                self.txt(&vs, x + t(cx) - tw / 2.0, y + t(cy - 8.0), px, 600, self.theme.color("fg"));
                let kw = self.fonts.width(k, px, 400);
                self.txt(k, x + t(cx) - kw / 2.0, y + t(bh - 6.0), px, 400, self.theme.color("fg2"));
            }
        }
    }
}

/// Paint a laid-out surface into a pixmap: `scale` device pixels per CSS pixel (2.0 on a Retina screen).
pub fn render(laid: &Laid, s: &Surface, fonts: &Fonts, theme: &Theme, images: &Images, scale: f32) -> Pixmap {
    let mut pm = Pixmap::new((laid.width * scale).ceil().max(1.0) as u32, (laid.height * scale).ceil().max(1.0) as u32).unwrap();
    pm.fill(theme.color("bg"));
    let mut cv = Canvas { pm, fonts, theme, images, sc: scale, t: Transform::from_scale(scale, scale) };
    cv.item(laid, s, laid.root);
    let _ = Route::Forward;
    cv.pm
}
