//! Coordinate spaces: a sized node holds positioned children. Primitives paint; a positioned node with
//! children shows them (its own look); a positioned node without children gets its default look from a
//! nested layout. This module resolves every child's absolute box through the chain of origins and
//! transforms, paints a space, and lists what takes input.
use crate::layout::{layout_sub, Hit, Laid};
use crate::text::Fonts;
use crate::theme::{hex, Theme};
use crate::Images;
use mx::parse::{is_interactive_role, is_painted};
use mx::{Node, Surface, Value};
use tiny_skia::{Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform};

/// One child of a space with its box in the space's own pixels (surface-relative after the caller's offset).
#[derive(Clone, Debug)]
pub struct Placed {
    pub node: usize,
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// the parent chain's transform at this node (translate, then rotate about the origin, then scale)
    pub transform: Transform,
    pub opacity: f32,
    /// entirely outside the space
    pub off: bool,
    /// a node with an interactive role
    pub interactive: bool,
    /// the default look of a positioned node without paint children, laid out on its own
    pub sub: Option<Box<Laid>>,
}

#[derive(Clone, Debug)]
pub struct SpaceLayout {
    pub placed: Vec<Placed>,
    pub width: f32,
    pub height: f32,
}

/// A colour word: a tone (`primary`), a tone with a lightness step (`primary-70`, that fraction of the tone
/// over the background), a theme key (`fg`, `line`, `white`), a hex (`#1a2b3c`, `#abc`) or `none`.
pub fn color(theme: &Theme, word: &str) -> Option<Color> {
    if word == "none" || word.is_empty() {
        return None;
    }
    if let Some(h) = word.strip_prefix('#') {
        let full = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect::<String>() } else { h.to_string() };
        return Some(hex(&full));
    }
    if let Some((tone, step)) = word.rsplit_once('-') {
        if let Ok(pct) = step.parse::<f32>() {
            if tone != "on" && tone != "primary-fg" {
                let base = theme.color(tone);
                return Some(crate::theme::over(base, (pct / 100.0).clamp(0.0, 1.0), theme.color("bg")));
            }
        }
    }
    Some(theme.color(word))
}

fn style_f(n: &Node, key: &str) -> Option<f32> {
    n.args.style.get(key).and_then(|v| v.parse().ok())
}

/// The hit a positioned node with an interactive role takes.
pub fn hit_of(n: &Node) -> Hit {
    let id = n.id.clone();
    match n.ty.as_str() {
        "button" | "tab" => Hit::Button(id),
        "link" => Hit::Link(id, n.args.href.clone().unwrap_or_default()),
        "check" => Hit::Check(id),
        "toggle" => Hit::Toggle(id),
        "field" => Hit::Field(id),
        "select" => Hit::Select(id),
        "slider" => Hit::Slider(id, n.args.min.unwrap_or(0.0), n.args.max.unwrap_or(100.0)),
        _ => Hit::None,
    }
}

/// Size and origin of a child in its parent's space. For a node without paint children, the nested
/// layout's size; for one with children, the declared size or the union of the children.
fn local_box(s: &Surface, n: usize, fonts: &Fonts, images: &Images, subs: &mut Vec<(usize, Laid)>) -> (f32, f32, f32, f32) {
    let nd = &s.nodes[n];
    let a = &nd.args;
    let (x, y) = a.pos.unwrap_or((0.0, 0.0));
    match nd.ty.as_str() {
        "rect" | "image" if is_painted(nd) => {
            let (w, h) = a.size.unwrap_or_else(|| a.href.as_deref().and_then(|h| images.get(h)).map(|p| (p.width() as f32, p.height() as f32)).unwrap_or((0.0, 0.0)));
            (x, y, w, h)
        }
        "circle" => {
            let r = a.radius.unwrap_or(0.0);
            (x - r, y - r, 2.0 * r, 2.0 * r)
        }
        "line" if is_painted(nd) => {
            let (x2, y2) = a.pos2.unwrap_or((x, y));
            (x.min(x2), y.min(y2), (x2 - x).abs(), (y2 - y).abs())
        }
        "path" => path_bounds(a.path.as_deref().unwrap_or("")).unwrap_or((x, y, 0.0, 0.0)),
        "text" if is_painted(nd) => {
            let px = style_f(nd, "size").unwrap_or(14.0);
            let weight = style_f(nd, "weight").unwrap_or(400.0) as u16;
            let text = nd.label.as_deref().unwrap_or("");
            let w = fonts.width(text, px, weight);
            let (asc, desc) = fonts.metrics(px, weight);
            let x0 = match a.style.get("anchor").map(String::as_str) {
                Some("middle") => x - w / 2.0,
                Some("end") => x - w,
                _ => x,
            };
            (x0, y - asc, w, asc - desc)
        }
        _ if nd.children.is_empty() && !mx::parse::is_generic(&nd.ty) => {
            // default look, laid out on its own at its declared width or its natural width
            let laid = layout_sub(s, n, fonts, images, a.size.map(|sz| sz.0));
            let (w, h) = a.size.unwrap_or((laid.width, laid.height));
            subs.push((n, laid));
            (x, y, w, h)
        }
        _ => {
            // a node that shows its children: the declared size, or the union of the children's boxes
            let mut b: Option<(f32, f32, f32, f32)> = None;
            for &c in &nd.children {
                let cb = local_box(s, c, fonts, images, subs);
                b = Some(match b {
                    None => cb,
                    Some(u) => {
                        let x0 = u.0.min(cb.0);
                        let y0 = u.1.min(cb.1);
                        (x0, y0, (u.0 + u.2).max(cb.0 + cb.2) - x0, (u.1 + u.3).max(cb.1 + cb.3) - y0)
                    }
                });
            }
            let (bx, by, bw, bh) = b.unwrap_or((0.0, 0.0, 0.0, 0.0));
            match a.size {
                Some((w, h)) => (x, y, w, h),
                None => (x + bx, y + by, bw, bh),
            }
        }
    }
}

/// Bounding box of SVG path data, control points included.
pub fn path_bounds(d: &str) -> Option<(f32, f32, f32, f32)> {
    let mut pts = vec![];
    let mut pb = PathBuilder::new();
    build_path(d, &mut pb, Some(&mut pts));
    if pts.is_empty() {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    Some((x0, y0, x1 - x0, y1 - y0))
}

/// Parse SVG path data (M L H V C S Q T A Z, absolute and relative) into a tiny-skia path.
pub fn svg_path(d: &str) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    build_path(d, &mut pb, None);
    pb.finish()
}

fn build_path(d: &str, pb: &mut PathBuilder, mut pts: Option<&mut Vec<(f32, f32)>>) {
    let mut toks: Vec<String> = vec![];
    let mut cur = String::new();
    for c in d.chars() {
        if c.is_ascii_alphabetic() {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
            toks.push(c.to_string());
        } else if c == ',' || c.is_whitespace() {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
        } else if c == '-' && !cur.is_empty() && !cur.ends_with('e') {
            toks.push(std::mem::take(&mut cur));
            cur.push(c);
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    let (mut x, mut y, mut sx, mut sy, mut cx, mut cy) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    let mut cmd = 'M';
    let mut i = 0;
    let num = |t: &str| t.parse::<f32>().unwrap_or(0.0);
    let mark = |px: f32, py: f32, pts: &mut Option<&mut Vec<(f32, f32)>>| {
        if let Some(v) = pts {
            v.push((px, py));
        }
    };
    while i < toks.len() {
        let t = &toks[i];
        if t.len() == 1 && t.chars().next().unwrap().is_ascii_alphabetic() {
            cmd = t.chars().next().unwrap();
            i += 1;
            if cmd == 'Z' || cmd == 'z' {
                pb.close();
                x = sx;
                y = sy;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let take = |k: usize, i: &mut usize| -> Vec<f32> {
            let v: Vec<f32> = toks[*i..(*i + k).min(toks.len())].iter().map(|t| num(t)).collect();
            *i += k;
            v
        };
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                let v = take(2, &mut i);
                if v.len() < 2 {
                    break;
                }
                x = ox + v[0];
                y = oy + v[1];
                sx = x;
                sy = y;
                pb.move_to(x, y);
                mark(x, y, &mut pts);
                cmd = if rel { 'l' } else { 'L' };
            }
            'L' => {
                let v = take(2, &mut i);
                if v.len() < 2 {
                    break;
                }
                x = ox + v[0];
                y = oy + v[1];
                pb.line_to(x, y);
                mark(x, y, &mut pts);
            }
            'H' => {
                let v = take(1, &mut i);
                if v.is_empty() {
                    break;
                }
                x = ox + v[0];
                pb.line_to(x, y);
                mark(x, y, &mut pts);
            }
            'V' => {
                let v = take(1, &mut i);
                if v.is_empty() {
                    break;
                }
                y = oy + v[0];
                pb.line_to(x, y);
                mark(x, y, &mut pts);
            }
            'C' => {
                let v = take(6, &mut i);
                if v.len() < 6 {
                    break;
                }
                let (x1, y1, x2, y2, x3, y3) = (ox + v[0], oy + v[1], ox + v[2], oy + v[3], ox + v[4], oy + v[5]);
                pb.cubic_to(x1, y1, x2, y2, x3, y3);
                mark(x1, y1, &mut pts);
                mark(x2, y2, &mut pts);
                mark(x3, y3, &mut pts);
                cx = x2;
                cy = y2;
                x = x3;
                y = y3;
            }
            'S' => {
                let v = take(4, &mut i);
                if v.len() < 4 {
                    break;
                }
                let (x1, y1) = (2.0 * x - cx, 2.0 * y - cy);
                let (x2, y2, x3, y3) = (ox + v[0], oy + v[1], ox + v[2], oy + v[3]);
                pb.cubic_to(x1, y1, x2, y2, x3, y3);
                mark(x2, y2, &mut pts);
                mark(x3, y3, &mut pts);
                cx = x2;
                cy = y2;
                x = x3;
                y = y3;
            }
            'Q' => {
                let v = take(4, &mut i);
                if v.len() < 4 {
                    break;
                }
                let (x1, y1, x2, y2) = (ox + v[0], oy + v[1], ox + v[2], oy + v[3]);
                pb.quad_to(x1, y1, x2, y2);
                mark(x1, y1, &mut pts);
                mark(x2, y2, &mut pts);
                cx = x1;
                cy = y1;
                x = x2;
                y = y2;
            }
            'T' => {
                let v = take(2, &mut i);
                if v.len() < 2 {
                    break;
                }
                let (x1, y1) = (2.0 * x - cx, 2.0 * y - cy);
                let (x2, y2) = (ox + v[0], oy + v[1]);
                pb.quad_to(x1, y1, x2, y2);
                mark(x2, y2, &mut pts);
                cx = x1;
                cy = y1;
                x = x2;
                y = y2;
            }
            'A' => {
                let v = take(7, &mut i);
                if v.len() < 7 {
                    break;
                }
                let (x2, y2) = (ox + v[5], oy + v[6]);
                for (c1x, c1y, c2x, c2y, ex, ey) in arc_to_cubics(x, y, v[0], v[1], v[2], v[3] != 0.0, v[4] != 0.0, x2, y2) {
                    pb.cubic_to(c1x, c1y, c2x, c2y, ex, ey);
                    mark(c1x, c1y, &mut pts);
                    mark(c2x, c2y, &mut pts);
                    mark(ex, ey, &mut pts);
                }
                x = x2;
                y = y2;
                cx = x;
                cy = y;
            }
            _ => break,
        }
    }
}

/// An SVG elliptical arc as cubic Béziers (the W3C centre parameterization, segments of at most 90°).
#[allow(clippy::too_many_arguments)]
fn arc_to_cubics(x1: f32, y1: f32, rx: f32, ry: f32, rot_deg: f32, large: bool, sweep: bool, x2: f32, y2: f32) -> Vec<(f32, f32, f32, f32, f32, f32)> {
    let mut out = vec![];
    if (x1 - x2).abs() < 1e-6 && (y1 - y2).abs() < 1e-6 {
        return out;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        out.push((x1, y1, x2, y2, x2, y2));
        return out;
    }
    let phi = rot_deg.to_radians();
    let (cos, sin) = (phi.cos(), phi.sin());
    let dx = (x1 - x2) / 2.0;
    let dy = (y1 - y2) / 2.0;
    let x1p = cos * dx + sin * dy;
    let y1p = -sin * dx + cos * dy;
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p).max(0.0);
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut coef = if den == 0.0 { 0.0 } else { (num / den).sqrt() };
    if large == sweep {
        coef = -coef;
    }
    let cxp = coef * rx * y1p / ry;
    let cyp = -coef * ry * x1p / rx;
    let cx = cos * cxp - sin * cyp + (x1 + x2) / 2.0;
    let cy = sin * cxp + cos * cyp + (y1 + y2) / 2.0;
    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| -> f32 {
        let d = (ux * vx + uy * vy) / ((ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt());
        let a = d.clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 { -a } else { a }
    };
    let theta1 = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut dtheta = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
    if !sweep && dtheta > 0.0 {
        dtheta -= 2.0 * std::f32::consts::PI;
    } else if sweep && dtheta < 0.0 {
        dtheta += 2.0 * std::f32::consts::PI;
    }
    let segs = ((dtheta.abs() / (std::f32::consts::PI / 2.0)).ceil() as usize).max(1);
    let delta = dtheta / segs as f32;
    let t = 4.0 / 3.0 * (delta / 4.0).tan();
    let point = |a: f32| -> (f32, f32) {
        let (ca, sa) = (a.cos(), a.sin());
        (cx + rx * ca * cos - ry * sa * sin, cy + rx * ca * sin + ry * sa * cos)
    };
    let deriv = |a: f32| -> (f32, f32) {
        let (ca, sa) = (a.cos(), a.sin());
        (-rx * sa * cos - ry * ca * sin, -rx * sa * sin + ry * ca * cos)
    };
    let mut a0 = theta1;
    for _ in 0..segs {
        let a1 = a0 + delta;
        let (p0x, p0y) = point(a0);
        let (p1x, p1y) = point(a1);
        let (d0x, d0y) = deriv(a0);
        let (d1x, d1y) = deriv(a1);
        out.push((p0x + t * d0x, p0y + t * d0y, p1x - t * d1x, p1y - t * d1y, p1x, p1y));
        a0 = a1;
    }
    out
}

/// Resolve every descendant's box inside a space, in the space's own pixels.
pub fn layout_space(s: &Surface, space: usize, fonts: &Fonts, images: &Images) -> SpaceLayout {
    let (cw, ch) = s.nodes[space].args.size.unwrap_or((0.0, 0.0));
    let mut placed = vec![];
    let mut subs: Vec<(usize, Laid)> = vec![];
    fn walk(s: &Surface, n: usize, fonts: &Fonts, images: &Images, t: Transform, opacity: f32, cw: f32, ch: f32, placed: &mut Vec<Placed>, subs: &mut Vec<(usize, Laid)>) {
        for &c in &s.nodes[n].children {
            let nd = &s.nodes[c];
            let op = opacity * style_f(nd, "opacity").unwrap_or(1.0);
            let (lx, ly, lw, lh) = local_box(s, c, fonts, images, subs);
            let corners = [(lx, ly), (lx + lw, ly), (lx, ly + lh), (lx + lw, ly + lh)].map(|(px, py)| {
                let mut p = tiny_skia::Point::from_xy(px, py);
                t.map_point(&mut p);
                (p.x, p.y)
            });
            let x0 = corners.iter().map(|p| p.0).fold(f32::MAX, f32::min);
            let y0 = corners.iter().map(|p| p.1).fold(f32::MAX, f32::min);
            let x1 = corners.iter().map(|p| p.0).fold(f32::MIN, f32::max);
            let y1 = corners.iter().map(|p| p.1).fold(f32::MIN, f32::max);
            let off = lw > 0.0 && lh > 0.0 && (x1 <= 0.0 || y1 <= 0.0 || x0 >= cw || y0 >= ch);
            let interactive = !is_painted(nd) && is_interactive_role(&nd.ty);
            let sub = subs.iter().position(|(k, _)| *k == c).map(|i| Box::new(subs.remove(i).1));
            placed.push(Placed { node: c, id: nd.id.clone(), x: x0, y: y0, w: x1 - x0, h: y1 - y0, transform: t, opacity: op, off, interactive, sub });
            if !is_painted(nd) && !nd.children.is_empty() {
                let (gx, gy) = nd.args.pos.unwrap_or((0.0, 0.0));
                let mut gt = Transform::from_translate(gx, gy);
                if let Some(r) = style_f(nd, "rotate") {
                    gt = gt.pre_rotate(r);
                }
                if let Some(sc) = style_f(nd, "scale") {
                    gt = gt.pre_scale(sc, sc);
                }
                walk(s, c, fonts, images, t.pre_concat(gt), op, cw, ch, placed, subs);
            }
        }
    }
    walk(s, space, fonts, images, Transform::identity(), 1.0, cw, ch, &mut placed, &mut subs);
    SpaceLayout { placed, width: cw, height: ch }
}

/// Paint a space. `t0` maps the space to device pixels (surface offset, fit scale, device scale).
#[allow(clippy::too_many_arguments)]
pub fn paint_space(cv: &mut crate::draw::Canvas, s: &Surface, lay: &SpaceLayout, t0: Transform) {
    let (fonts, theme, images) = (cv.fonts, cv.theme, cv.images);
    let clip = {
        let mut m = tiny_skia::Mask::new(cv.pm.width(), cv.pm.height()).unwrap();
        let mut pb = PathBuilder::new();
        pb.push_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, lay.width.max(1.0), lay.height.max(1.0)).unwrap());
        m.fill_path(&pb.finish().unwrap(), FillRule::Winding, true, t0);
        m
    };
    let mut subs: Vec<(usize, Laid)> = vec![];
    for p in &lay.placed {
        let nd = &s.nodes[p.node];
        let t = t0.pre_concat(p.transform);
        let fill = nd.args.style.get("fill").map(String::as_str);
        let stroke_c = nd.args.style.get("stroke").map(String::as_str);
        let width = style_f(nd, "width").unwrap_or(1.0);
        let dash = if nd.flags.contains("dashed") { Some((5.0, 4.0)) } else { None };
        let alpha = |c: Color| Color::from_rgba(c.red(), c.green(), c.blue(), c.alpha() * p.opacity).unwrap();
        let do_fill = |pm: &mut Pixmap, path: &tiny_skia::Path, c: Color| {
            let mut paint = Paint::default();
            paint.set_color(alpha(c));
            paint.anti_alias = true;
            pm.fill_path(path, &paint, FillRule::Winding, t, Some(&clip));
        };
        let do_stroke = |pm: &mut Pixmap, path: &tiny_skia::Path, c: Color| {
            let mut paint = Paint::default();
            paint.set_color(alpha(c));
            paint.anti_alias = true;
            let mut st = Stroke { width, line_cap: LineCap::Butt, ..Default::default() };
            if let Some((on, off)) = dash {
                st.dash = tiny_skia::StrokeDash::new(vec![on, off], 0.0);
            }
            pm.stroke_path(path, &paint, &st, t, Some(&clip));
        };
        let (lx, ly, lw, lh) = local_box(s, p.node, fonts, images, &mut subs);
        subs.clear();
        if !is_painted(nd) {
            // a node's own box: the declared fill and stroke; its default look if it has no children
            if fill.is_some() || stroke_c.is_some() {
                let (gx, gy) = nd.args.pos.unwrap_or((0.0, 0.0));
                let _ = (gx, gy);
                let path = crate::draw::rounded(lx, ly, lw, lh, style_f(nd, "radius").unwrap_or(0.0));
                if let Some(c) = fill.and_then(|f| color(theme, f)) {
                    do_fill(&mut cv.pm, &path, c);
                }
                if let Some(c) = stroke_c.and_then(|f| color(theme, f)) {
                    do_stroke(&mut cv.pm, &path, c);
                }
            }
            if let Some(sub) = &p.sub {
                cv.paint_laid(sub, s, t.pre_concat(Transform::from_translate(lx, ly)));
            }
            continue;
        }
        match nd.ty.as_str() {
            "rect" => {
                let path = crate::draw::rounded(lx, ly, lw, lh, style_f(nd, "radius").unwrap_or(0.0));
                if let Some(c) = fill.and_then(|f| color(theme, f)).or_else(|| if stroke_c.is_none() { Some(theme.color("fg")) } else { None }) {
                    do_fill(&mut cv.pm, &path, c);
                }
                if let Some(c) = stroke_c.and_then(|f| color(theme, f)) {
                    do_stroke(&mut cv.pm, &path, c);
                }
            }
            "circle" => {
                let (cx, cy) = nd.args.pos.unwrap_or((0.0, 0.0));
                if let Some(path) = PathBuilder::from_circle(cx, cy, nd.args.radius.unwrap_or(0.0)) {
                    if let Some(c) = fill.and_then(|f| color(theme, f)).or_else(|| if stroke_c.is_none() { Some(theme.color("fg")) } else { None }) {
                        do_fill(&mut cv.pm, &path, c);
                    }
                    if let Some(c) = stroke_c.and_then(|f| color(theme, f)) {
                        do_stroke(&mut cv.pm, &path, c);
                    }
                }
            }
            "line" => {
                let (x1, y1) = nd.args.pos.unwrap_or((0.0, 0.0));
                let (x2, y2) = nd.args.pos2.unwrap_or((x1, y1));
                let mut pb = PathBuilder::new();
                pb.move_to(x1, y1);
                pb.line_to(x2, y2);
                if let Some(path) = pb.finish() {
                    do_stroke(&mut cv.pm, &path, stroke_c.and_then(|f| color(theme, f)).unwrap_or_else(|| theme.color("fg")));
                }
            }
            "path" => {
                if let Some(path) = svg_path(nd.args.path.as_deref().unwrap_or("")) {
                    if let Some(c) = fill.and_then(|f| color(theme, f)) {
                        do_fill(&mut cv.pm, &path, c);
                    }
                    if let Some(c) = stroke_c.and_then(|f| color(theme, f)).or_else(|| if fill.is_none() { Some(theme.color("fg")) } else { None }) {
                        do_stroke(&mut cv.pm, &path, c);
                    }
                }
            }
            "text" => {
                let px = style_f(nd, "size").unwrap_or(14.0);
                let weight = style_f(nd, "weight").unwrap_or(400.0) as u16;
                let (_, baseline) = nd.args.pos.unwrap_or((0.0, 0.0));
                let c = alpha(fill.and_then(|f| color(theme, f)).unwrap_or_else(|| theme.color("fg")));
                let mut pt = tiny_skia::Point::from_xy(lx, baseline);
                t.map_point(&mut pt);
                let sc = t.sx.abs().max(0.01);
                fonts.draw(&mut cv.pm, nd.label.as_deref().unwrap_or(""), pt.x, pt.y, px * sc, weight, c);
            }
            "image" => {
                if let Some(img) = nd.args.href.as_deref().and_then(|h| images.get(h)) {
                    let (iw, ih) = (img.width() as f32, img.height() as f32);
                    let (w, h) = nd.args.size.unwrap_or((iw, ih));
                    let it = t.pre_concat(Transform::from_row(w / iw, 0.0, 0.0, h / ih, lx, ly));
                    let paint = PixmapPaint { opacity: p.opacity, ..Default::default() };
                    cv.pm.draw_pixmap(0, 0, img.as_ref(), &paint, it, Some(&clip));
                }
            }
            _ => {}
        }
    }
    let _ = Value::None;
}
