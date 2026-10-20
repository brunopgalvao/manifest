//! The host: owns every surface, lays it out and paints it, runs its lifecycle, queues events and answers
//! observe and patch. A port of packages/renderer/src/host.ts without a DOM: the window, the wasm
//! playground and the MCP server all drive this one struct. Time comes in through `tick(now_ms)`, so the
//! host has no clock of its own and lifecycle is testable.
use crate::layout::{Hit, Item};
use crate::{Rendered, Renderer};
use mx::perceive::PerceiveOptions;
use mx::{MXError, Surface, Value};
use std::collections::BTreeMap;
use tiny_skia::{Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Live,
    Archived,
    Gone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Residue {
    None,
    Line,
    Archive,
}

/// A change being tweened: the node's previous numbers, the moment it started and how long it takes.
#[derive(Clone, Debug)]
pub struct Anim {
    pub id: String,
    pub start: u64,
    pub dur: u64,
    pub data: Vec<(String, f64, bool)>,
    pub pos: Option<(f32, f32)>,
    pub size: Option<(f32, f32)>,
    pub radius: Option<f32>,
    pub value: Option<f64>,
}

pub struct Entry {
    pub surface: Surface,
    pub anims: Vec<Anim>,
    pub history: BTreeMap<u32, Surface>,
    pub state: State,
    pub residue: Residue,
    pub ephemeral: bool,
    pub until: Option<String>,
    pub ttl: Option<(String, u64)>,
    pub seen: Option<(String, u64)>,
    pub seen_ms: u64,
    pub in_view: bool,
    pub dissolved_reason: Option<String>,
    pub rendered: Option<Rendered>,
    pub pixmap: Option<Pixmap>,
    /// the field being typed into, and the text as typed so far
    pub editing: Option<(String, String)>,
    /// caret position in characters while editing
    pub caret: usize,
}

fn num_value(v: &Value) -> Option<f64> {
    match v {
        Value::Num(x) => Some(*x),
        _ => None,
    }
}

/// `ttl:30s`, `seen:10s` and the like, in milliseconds.
pub fn duration(s: &str) -> Result<u64, MXError> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let unit = &s[digits.len()..];
    let n: f64 = digits.parse().map_err(|_| MXError::new(format!("bad duration {}", serde_json::to_string(s).unwrap())))?;
    let mult = match unit {
        "" | "s" => 1000.0,
        "ms" => 1.0,
        "m" => 60_000.0,
        "h" => 3_600_000.0,
        _ => return Err(MXError::new(format!("bad duration {}", serde_json::to_string(s).unwrap()))),
    };
    Ok((n * mult) as u64)
}

/// A surface or residue line on a composed screen: id, top, width, height, in CSS pixels.
#[derive(Clone, Debug)]
pub struct Placed {
    pub id: String,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub live: bool,
}

pub const GAP: f32 = 16.0;
pub const COL_W: f32 = 760.0;
/// the dismiss cross's hit square, top right of every live surface
pub const CLOSE: f32 = 28.0;

pub struct Host {
    pub renderer: Renderer,
    pub entries: Vec<Entry>,
    pub events: Vec<String>,
    now: u64,
    last_tick: u64,
}

impl Host {
    pub fn new(renderer: Renderer) -> Host {
        Host { renderer, entries: vec![], events: vec![], now: 0, last_tick: 0 }
    }

    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.surface.id == id)
    }

    fn entry_mut(&mut self, id: &str) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.surface.id == id)
    }

    fn live(&mut self, id: &str) -> Result<usize, MXError> {
        let i = self.entries.iter().position(|e| e.surface.id == id).ok_or_else(|| MXError::new(format!("no surface {}", serde_json::to_string(id).unwrap())))?;
        if self.entries[i].state != State::Live {
            return Err(MXError::new(format!("surface {} has dissolved", serde_json::to_string(id).unwrap())));
        }
        Ok(i)
    }

    /// `manifest.render`: a full document. Returns the L0 line.
    pub fn render(&mut self, text: &str) -> Result<String, MXError> {
        let surface = mx::parse(text)?;
        let mut residue = Residue::None;
        let mut ephemeral = false;
        let mut until = None;
        let mut ttl = None;
        let mut seen = None;
        for w in &surface.life {
            let (k, v) = match w.find(':') {
                Some(i) => (&w[..i], &w[i + 1..]),
                None => (w.as_str(), ""),
            };
            match k {
                "residue" => {
                    residue = match v {
                        "none" | "" => Residue::None,
                        "line" => Residue::Line,
                        "archive" => Residue::Archive,
                        _ => return Err(MXError::new(format!("unknown residue {}", serde_json::to_string(v).unwrap()))),
                    }
                }
                "ephemeral" => ephemeral = true,
                "until" => until = Some(v.to_string()),
                "ttl" => ttl = Some((v.to_string(), self.now + duration(v)?)),
                "seen" => seen = Some((v.to_string(), duration(v)?)),
                _ => {}
            }
        }
        if let Some(i) = self.entries.iter().position(|e| e.surface.id == surface.id) {
            self.entries.remove(i);
        }
        self.renderer.load_images(&surface);
        let mut history = BTreeMap::new();
        history.insert(0, surface.clone());
        let mut e = Entry { surface, anims: vec![], history, state: State::Live, residue, ephemeral, until, ttl, seen, seen_ms: 0, in_view: true, dissolved_reason: None, rendered: None, pixmap: None, editing: None, caret: 0 };
        Self::draw(&self.renderer, &mut e);
        let l0 = Self::l0(&e);
        self.entries.push(e);
        Ok(l0)
    }

    /// `manifest.patch`. Returns the L0 line after the patch.
    pub fn patch(&mut self, id: &str, patch: &str) -> Result<String, MXError> {
        let i = self.live(id)?;
        let next = mx::apply(&self.entries[i].surface, patch, Some(id))?;
        let now = self.now;
        let e = &mut self.entries[i];
        Self::start_anims(e, &next, now);
        e.history.insert(next.rev, next.clone());
        e.surface = next;
        Self::draw(&self.renderer, e);
        Ok(Self::l0(e))
    }

    /// `manifest.observe`.
    pub fn observe(&self, id: &str, level: u8, since: Option<u32>, focus: Option<&str>) -> Result<String, MXError> {
        let e = self.entry(id).ok_or_else(|| MXError::new(format!("no surface {}", serde_json::to_string(id).unwrap())))?;
        if e.state == State::Gone {
            return Ok(if e.residue == Residue::Line && e.dissolved_reason.is_some() { mx::residue_line(&e.surface) } else { format!("@{} dissolved rev={}", id, e.surface.rev) });
        }
        if let Some(since) = since {
            if since >= e.surface.rev {
                return Ok(format!("~{}", e.surface.rev));
            }
            if let Some(base) = e.history.get(&since) {
                if let Some(d) = mx::diff(base, &e.surface) {
                    return Ok(d);
                }
            }
        }
        let empty = BTreeMap::new();
        let (boxes, diags): (&BTreeMap<String, mx::Box>, &[mx::Diag]) = match &e.rendered {
            Some(r) => (&r.boxes, &r.diags),
            None => (&empty, &[]),
        };
        let text = mx::perceive(&e.surface, level, PerceiveOptions { boxes: Some(boxes), diags, rev: None, focus })?;
        Ok(if e.state == State::Archived {
            match text.find('\n') {
                Some(i) => format!("{} archived{}", &text[..i], &text[i..]),
                None => format!("{} archived", text),
            }
        } else {
            text
        })
    }

    /// L0 for every surface the host knows about.
    pub fn list(&self) -> String {
        let lines: Vec<String> = self
            .entries
            .iter()
            .filter(|e| e.state != State::Gone || (e.residue == Residue::Line && e.dissolved_reason.is_some()))
            .map(|e| if e.state == State::Gone { mx::residue_line(&e.surface) } else { Self::l0(e) })
            .collect();
        if lines.is_empty() {
            "(no surfaces)".into()
        } else {
            lines.join("\n")
        }
    }

    /// Events since the last drain, oldest first.
    pub fn drain(&mut self) -> Vec<String> {
        std::mem::take(&mut self.events)
    }

    /// A new model turn: ephemeral surfaces dissolve.
    pub fn turn(&mut self) {
        let ids: Vec<String> = self.entries.iter().filter(|e| e.state == State::Live && e.ephemeral).map(|e| e.surface.id.clone()).collect();
        for id in ids {
            self.dissolve(&id, "ephemeral");
        }
    }

    pub fn dissolve(&mut self, id: &str, reason: &str) {
        let Some(e) = self.entry_mut(id) else { return };
        if e.state != State::Live {
            return;
        }
        e.dissolved_reason = Some(reason.to_string());
        e.state = if e.residue == Residue::Archive { State::Archived } else { State::Gone };
        e.pixmap = None;
        e.editing = None;
        let rev = e.surface.rev;
        self.events.push(format!("@{} dissolved reason={} rev={}", id, reason, rev));
    }

    /// Nodes with `anim:` whose numbers changed between `old` and `new` start a tween.
    fn start_anims(e: &mut Entry, next: &Surface, now: u64) {
        let old = &e.surface;
        for (id, n) in &next.ids {
            let nn = &next.nodes[*n];
            let Some(spec) = nn.args.style.get("anim") else { continue };
            let Ok(dur) = duration(spec) else { continue };
            let Some(o) = old.get(id).map(|k| &old.nodes[k]) else { continue };
            let changed = o.args.data != nn.args.data || o.args.pos != nn.args.pos || o.args.size != nn.args.size || o.args.radius != nn.args.radius || o.args.value != nn.args.value;
            if !changed || dur == 0 {
                continue;
            }
            // a running tween restarts from where it is now
            let from = e.anims.iter().find(|a| a.id == *id).map(|a| Self::interp(a, o, now)).unwrap_or_else(|| (o.args.data.clone(), o.args.pos, o.args.size, o.args.radius, num_value(&o.args.value)));
            e.anims.retain(|a| a.id != *id);
            e.anims.push(Anim { id: id.clone(), start: now, dur, data: from.0, pos: from.1, size: from.2, radius: from.3, value: from.4 });
        }
    }

    /// Where a tween is at `now`, between its stored start and the node's current (target) numbers.
    fn interp(a: &Anim, target: &mx::Node, now: u64) -> (Vec<(String, f64, bool)>, Option<(f32, f32)>, Option<(f32, f32)>, Option<f32>, Option<f64>) {
        let t = ((now.saturating_sub(a.start)) as f64 / a.dur.max(1) as f64).clamp(0.0, 1.0);
        let t = t * t * (3.0 - 2.0 * t); // ease in and out
        let lf = |x: f32, y: f32| x + (y - x) * t as f32;
        let ld = |x: f64, y: f64| x + (y - x) * t;
        let data = target
            .args
            .data
            .iter()
            .map(|(k, v, h)| match a.data.iter().find(|d| &d.0 == k) {
                Some(d) => (k.clone(), ld(d.1, *v), *h),
                None => (k.clone(), *v, *h),
            })
            .collect();
        let pos = match (a.pos, target.args.pos) {
            (Some(p), Some(q)) => Some((lf(p.0, q.0), lf(p.1, q.1))),
            (_, q) => q,
        };
        let size = match (a.size, target.args.size) {
            (Some(p), Some(q)) => Some((lf(p.0, q.0), lf(p.1, q.1))),
            (_, q) => q,
        };
        let radius = match (a.radius, target.args.radius) {
            (Some(p), Some(q)) => Some(lf(p, q)),
            (_, q) => q,
        };
        let value = match (a.value, num_value(&target.args.value)) {
            (Some(p), Some(q)) => Some(ld(p, q)),
            (_, q) => q,
        };
        (data, pos, size, radius, value)
    }

    /// The surface as it should look right now: the real one with tweened numbers substituted.
    fn display(e: &Entry, now: u64) -> Option<Surface> {
        if e.anims.is_empty() {
            return None;
        }
        let mut s = e.surface.clone();
        for a in &e.anims {
            let Some(n) = s.get(&a.id) else { continue };
            let (data, pos, size, radius, value) = Self::interp(a, &e.surface.nodes[n], now);
            let nd = &mut s.nodes[n];
            nd.args.data = data;
            nd.args.pos = pos;
            nd.args.size = size;
            nd.args.radius = radius;
            if let (Some(v), Value::Num(_)) = (value, &nd.args.value) {
                nd.args.value = Value::Num(v);
            }
        }
        Some(s)
    }

    /// Whether any surface is mid-tween (callers then tick at frame rate).
    pub fn animating(&self) -> bool {
        self.entries.iter().any(|e| e.state == State::Live && !e.anims.is_empty())
    }

    /// Advance the clock: `ttl` deadlines, `seen` time (counted while the surface is in view), tweens.
    pub fn tick(&mut self, now_ms: u64) {
        let dt = now_ms.saturating_sub(self.last_tick);
        self.now = now_ms;
        self.last_tick = now_ms;
        for e in &mut self.entries {
            if e.state == State::Live && !e.anims.is_empty() {
                let before = e.anims.len();
                e.anims.retain(|a| now_ms < a.start + a.dur);
                Self::draw_at(&self.renderer, e, now_ms);
                let _ = before;
            }
        }
        let mut due: Vec<(String, String)> = vec![];
        for e in &mut self.entries {
            if e.state != State::Live {
                continue;
            }
            if let Some((spec, at)) = &e.ttl {
                if now_ms >= *at {
                    due.push((e.surface.id.clone(), format!("ttl:{}", spec)));
                    continue;
                }
            }
            if let (Some((spec, target)), true) = (&e.seen, e.in_view) {
                e.seen_ms += dt;
                if e.seen_ms >= *target {
                    due.push((e.surface.id.clone(), format!("seen:{}", spec)));
                }
            }
        }
        for (id, reason) in due {
            self.dissolve(&id, &reason);
        }
    }

    pub fn set_in_view(&mut self, id: &str, in_view: bool) {
        if let Some(e) = self.entry_mut(id) {
            e.in_view = in_view;
        }
    }

    /// The pixels of a live surface.
    pub fn pixmap(&self, id: &str) -> Option<&Pixmap> {
        self.entry(id).and_then(|e| e.pixmap.as_ref())
    }

    /// `manifest.look`: a PNG of one node, or the whole surface.
    pub fn look(&self, id: &str, node: Option<&str>) -> Result<(Vec<u8>, u32, u32), MXError> {
        let e = self.entry(id).ok_or_else(|| MXError::new(format!("no surface {}", serde_json::to_string(id).unwrap())))?;
        let (Some(pm), Some(r)) = (&e.pixmap, &e.rendered) else {
            return Err(MXError::new(format!("surface {} is not on screen", serde_json::to_string(id).unwrap())));
        };
        let b = match node {
            Some(n) => *r.boxes.get(n).ok_or_else(|| MXError::new(format!("no node {} on screen in {}", n, id)))?,
            None => mx::Box { x: 0, y: 0, w: pm.width() as i32, h: pm.height() as i32 },
        };
        let sc = self.renderer.scale;
        let (x, y) = ((b.x.max(0) as f32 * sc) as u32, (b.y.max(0) as f32 * sc) as u32);
        let w = ((b.w.max(1) as f32 * sc) as u32).min(pm.width().saturating_sub(x).max(1));
        let h = ((b.h.max(1) as f32 * sc) as u32).min(pm.height().saturating_sub(y).max(1));
        let rect = tiny_skia::IntRect::from_xywh(x as i32, y as i32, w, h).ok_or_else(|| MXError::new("empty box"))?;
        let crop = pm.clone_rect(rect).ok_or_else(|| MXError::new("empty box"))?;
        Ok((crop.encode_png().map_err(|e| MXError::new(e.to_string()))?, w, h))
    }

    /// The interactive item under a surface-relative point: the smallest one wins.
    pub fn hit(&self, id: &str, x: f32, y: f32) -> Option<(Hit, mx::Box)> {
        let e = self.entry(id)?;
        let r = e.rendered.as_ref()?;
        let mut best: Option<&Item> = None;
        for it in &r.laid.items {
            if it.hit == Hit::None || x < it.x || y < it.y || x > it.x + it.w || y > it.y + it.h {
                continue;
            }
            if best.map(|b| it.w * it.h < b.w * b.h).unwrap_or(true) {
                best = Some(it);
            }
        }
        let mut best = best.map(|it| (it.hit.clone(), mx::Box { x: it.x.round() as i32, y: it.y.round() as i32, w: it.w.round() as i32, h: it.h.round() as i32 }));
        for (hit, b) in r.laid.space_hits(&e.surface) {
            let inside = x >= b.x as f32 && y >= b.y as f32 && x <= (b.x + b.w) as f32 && y <= (b.y + b.h) as f32;
            if inside && best.as_ref().map(|(_, bb)| b.w * b.h < bb.w * bb.h).unwrap_or(true) {
                best = Some((hit, b));
            }
        }
        best
    }

    /// A pointer press at a surface-relative point. Returns the event it produced, if any.
    pub fn click(&mut self, id: &str, x: f32, y: f32) -> Option<String> {
        let i = self.live(id).ok()?;
        let (hit, bx) = match self.hit(id, x, y) {
            Some(h) => h,
            None => {
                self.commit_edit(i);
                return None;
            }
        };
        if !matches!(hit, Hit::Field(_)) {
            self.commit_edit(i);
        }
        match hit {
            Hit::None => None,
            Hit::Button(nid) => {
                let e = &self.entries[i];
                let ev = format!("@{} action {} rev={} {}", id, nid, e.surface.rev, mx::data_model(&e.surface));
                self.events.push(ev.clone());
                if let Some(u) = e.until.clone() {
                    if u == "action" || u == nid {
                        self.dissolve(id, &if u == "action" { "until:action".to_string() } else { format!("until:{}", nid) });
                    }
                }
                Some(ev)
            }
            Hit::Check(nid) | Hit::Toggle(nid) => {
                let e = &mut self.entries[i];
                let n = e.surface.get(&nid)?;
                let on = e.surface.nodes[n].args.value == Value::Bool(true);
                Some(self.input(i, &nid, Value::Bool(!on)))
            }
            Hit::Tab(tabs, label) => {
                let e = &mut self.entries[i];
                let t = e.surface.get(&tabs)?;
                for c in e.surface.nodes[t].children.clone() {
                    let sel = e.surface.nodes[c].label.as_deref() == Some(label.as_str());
                    if sel {
                        e.surface.nodes[c].flags.insert("selected".into());
                    } else {
                        e.surface.nodes[c].flags.remove("selected");
                    }
                }
                e.surface.rev += 1;
                e.history.insert(e.surface.rev, e.surface.clone());
                let ev = format!("@{} input {} {} rev={}", id, tabs, serde_json::to_string(&label).unwrap(), e.surface.rev);
                self.events.push(ev.clone());
                Self::draw(&self.renderer, &mut self.entries[i]);
                Some(ev)
            }
            Hit::Slider(nid, min, max) => {
                let frac = ((x - bx.x as f32) / bx.w.max(1) as f32).clamp(0.0, 1.0) as f64;
                let v = (min + frac * (max - min)).round();
                Some(self.input(i, &nid, Value::Num(v)))
            }
            Hit::Select(nid) => {
                let e = &self.entries[i];
                let n = e.surface.get(&nid)?;
                let opts = e.surface.nodes[n].args.options.clone();
                if opts.is_empty() {
                    return None;
                }
                let cur = match &e.surface.nodes[n].args.value {
                    Value::Str(s) => opts.iter().position(|o| o == s),
                    Value::List(l) => l.first().and_then(|s| opts.iter().position(|o| o == s)),
                    _ => None,
                };
                let next = opts[(cur.map(|c| c + 1).unwrap_or(0)) % opts.len()].clone();
                Some(self.input(i, &nid, Value::Str(next)))
            }
            Hit::Field(nid) => {
                let e = &mut self.entries[i];
                if e.editing.as_ref().map(|(f, _)| f != &nid).unwrap_or(true) {
                    self.commit_edit(i);
                    let e = &mut self.entries[i];
                    let n = e.surface.get(&nid)?;
                    let cur = match &e.surface.nodes[n].args.value {
                        Value::Str(s) => s.clone(),
                        _ => String::new(),
                    };
                    e.caret = cur.chars().count();
                    e.editing = Some((nid, cur));
                    Self::draw(&self.renderer, &mut self.entries[i]);
                }
                None
            }
            Hit::Link(_, href) => {
                let ev = format!("@{} link {} rev={}", id, href, self.entries[i].surface.rev);
                self.events.push(ev.clone());
                Some(ev)
            }
        }
    }

    /// Typed text for the focused field. Enter commits, Escape cancels, backspace and delete edit at the
    /// caret, and the control characters \u{1}..\u{4} move it (left, right, home, end).
    pub fn key(&mut self, id: &str, text: &str) -> Option<String> {
        let i = self.live(id).ok()?;
        let e = &mut self.entries[i];
        let (nid, buf) = e.editing.as_mut()?;
        let mut chars: Vec<char> = buf.chars().collect();
        let mut caret = e.caret.min(chars.len());
        match text {
            "\n" | "\r" => return self.commit_edit(i),
            "\u{1b}" => {
                // cancel: back to the committed value
                let nid = nid.clone();
                e.editing = None;
                if let Some(n) = e.surface.get(&nid) {
                    let committed = e.history.get(&e.surface.rev).and_then(|s| s.get(&nid)).map(|k| e.history[&e.surface.rev].nodes[k].args.value.clone());
                    if let Some(v) = committed {
                        e.surface.nodes[n].args.value = v;
                    }
                }
                Self::draw(&self.renderer, e);
                return None;
            }
            "\u{8}" => {
                if caret > 0 {
                    chars.remove(caret - 1);
                    caret -= 1;
                }
            }
            "\u{7f}" => {
                if caret < chars.len() {
                    chars.remove(caret);
                }
            }
            "\u{1}" => caret = caret.saturating_sub(1),
            "\u{2}" => caret = (caret + 1).min(chars.len()),
            "\u{3}" => caret = 0,
            "\u{4}" => caret = chars.len(),
            t => {
                for c in t.chars() {
                    chars.insert(caret, c);
                    caret += 1;
                }
            }
        }
        *buf = chars.into_iter().collect();
        e.caret = caret;
        let n = e.surface.get(nid)?;
        e.surface.nodes[n].args.value = Value::Str(buf.clone());
        Self::draw(&self.renderer, e);
        None
    }

    /// The field being edited, if any.
    pub fn editing(&self, id: &str) -> Option<&str> {
        self.entry(id).and_then(|e| e.editing.as_ref()).map(|(f, _)| f.as_str())
    }

    fn commit_edit(&mut self, i: usize) -> Option<String> {
        let e = &mut self.entries[i];
        let (nid, buf) = e.editing.take()?;
        let n = e.surface.get(&nid)?;
        let before = e.history.get(&e.surface.rev).and_then(|s| s.get(&nid)).map(|k| e.history[&e.surface.rev].nodes[k].args.value.clone());
        if before.as_ref() == Some(&Value::Str(buf.clone())) {
            e.surface.nodes[n].args.value = Value::Str(buf);
            return None;
        }
        Some(self.input(i, &nid, Value::Str(buf)))
    }

    fn input(&mut self, i: usize, nid: &str, v: Value) -> String {
        let e = &mut self.entries[i];
        let id = e.surface.id.clone();
        let n = e.surface.get(nid).expect("input node");
        e.surface.nodes[n].args.value = v.clone();
        e.surface.rev += 1;
        e.history.insert(e.surface.rev, e.surface.clone());
        let ev = format!("@{} input {} {} rev={}", id, nid, mx::fmt_value(&e.surface.nodes[n].ty, &v), e.surface.rev);
        self.events.push(ev.clone());
        Self::draw(&self.renderer, e);
        ev
    }

    /// Lay out and paint a surface again (after a theme change).
    pub fn repaint(&mut self, id: &str) -> Result<(), MXError> {
        let i = self.live(id)?;
        Self::draw(&self.renderer, &mut self.entries[i]);
        Ok(())
    }

    /// Where the column of surfaces starts in a screen `css_w` wide: centred, at most 760 wide, like #surfaces.
    pub fn column_x(css_w: f32) -> f32 {
        (css_w - COL_W.min(css_w)) / 2.0 + GAP
    }

    /// Compose every surface and residue line into one screen of `w`x`h` device pixels at `sc` pixels per
    /// CSS pixel, scrolled by `scroll` CSS pixels. What the window and the wasm page show.
    pub fn compose(&self, w: u32, h: u32, sc: f32, scroll: f32) -> (Pixmap, Vec<Placed>) {
        let mut pm = Pixmap::new(w.max(1), h.max(1)).unwrap();
        let theme = self.renderer.theme;
        pm.fill(theme.color("bg"));
        let x0 = Self::column_x(w as f32 / sc);
        let mut y = GAP - scroll;
        let mut placed = vec![];
        let t = Transform::from_scale(sc, sc);
        for e in &self.entries {
            if e.state == State::Live {
                let Some(p) = &e.pixmap else { continue };
                pm.draw_pixmap((x0 * sc) as i32, (y * sc) as i32, p.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
                let (pw, ph) = (p.width() as f32 / sc, p.height() as f32 / sc);
                let (cx, cy) = (x0 + pw - 18.0, y + 14.0);
                let mut pb = PathBuilder::new();
                pb.move_to(cx - 4.0, cy - 4.0);
                pb.line_to(cx + 4.0, cy + 4.0);
                pb.move_to(cx + 4.0, cy - 4.0);
                pb.line_to(cx - 4.0, cy + 4.0);
                let mut paint = Paint::default();
                paint.set_color(theme.color("fg2"));
                paint.anti_alias = true;
                pm.stroke_path(&pb.finish().unwrap(), &paint, &Stroke { width: 1.5, ..Default::default() }, t, None);
                placed.push(Placed { id: e.surface.id.clone(), y, w: pw, h: ph, live: true });
                y += ph + GAP;
            } else if e.state == State::Gone && e.residue == Residue::Line && e.dissolved_reason.is_some() {
                let line = mx::residue_line(&e.surface);
                let lh = 13.0 * 1.45;
                let hh = lh + 16.0;
                let mut paint = Paint::default();
                paint.set_color(theme.color("line"));
                if let Some(r) = Rect::from_xywh(x0 * sc, y * sc, 3.0 * sc, hh * sc) {
                    pm.fill_rect(r, &paint, Transform::identity(), None);
                }
                let (asc, desc) = self.renderer.fonts.metrics(13.0 * sc, 400);
                self.renderer.fonts.draw(&mut pm, &line, (x0 + 12.0) * sc, (y + 8.0) * sc + (lh * sc - (asc - desc)) / 2.0 + asc, 13.0 * sc, 400, theme.color("fg2"));
                placed.push(Placed { id: e.surface.id.clone(), y, w: COL_W - 2.0 * GAP, h: hh, live: false });
                y += hh + GAP;
            }
        }
        if placed.is_empty() {
            let msg = "Nothing rendered yet.";
            let tw = self.renderer.fonts.width(msg, 14.0, 400);
            let cx = (w as f32 / sc - tw) / 2.0;
            self.renderer.fonts.draw(&mut pm, msg, cx * sc, (48.0 + 14.0) * sc, 14.0 * sc, 400, theme.color("fg2"));
        }
        (pm, placed)
    }

    /// The live surface under a screen point (CSS pixels, after scrolling): index into `placed` and the
    /// point in surface coordinates.
    pub fn under(placed: &[Placed], css_w: f32, x: f32, y: f32) -> Option<(usize, f32, f32)> {
        let x0 = Self::column_x(css_w);
        placed.iter().enumerate().filter(|(_, p)| p.live).find(|(_, p)| x >= x0 && x < x0 + p.w && y >= p.y && y < p.y + p.h).map(|(i, p)| (i, x - x0, y - p.y))
    }

    fn l0(e: &Entry) -> String {
        let diags: &[mx::Diag] = e.rendered.as_ref().map(|r| r.diags.as_slice()).unwrap_or(&[]);
        mx::perceive(&e.surface, 0, PerceiveOptions { diags, ..Default::default() }).unwrap_or_default()
    }

    fn draw(renderer: &Renderer, e: &mut Entry) {
        // layout, boxes and diagnostics come from the real surface; pixels from the tweened one
        let (r, mut pm) = renderer.render(&e.surface);
        if let Some((nid, buf)) = &e.editing {
            // the caret: a 1px line after the typed text, inside the field's input box
            if let Some(it) = r.laid.items.iter().find(|it| matches!(&it.hit, crate::layout::Hit::Field(f) if f == nid)) {
                let sc = renderer.scale;
                let head: String = buf.chars().take(e.caret).collect();
                let shown = if it.paint.text.as_ref().map(|t| t.text.starts_with('•')).unwrap_or(false) { "•".repeat(head.chars().count()) } else { head };
                let x = it.x + it.pad.0 + it.border + renderer.fonts.width(&shown, 14.0, 400);
                let (y0, y1) = (it.y + it.pad.1 + it.border + 2.0, it.y + it.h - it.pad.1 - it.border - 2.0);
                let mut paint = Paint::default();
                paint.set_color(renderer.theme.color("fg"));
                if let Some(rect) = Rect::from_xywh(x * sc, y0 * sc, 1.0 * sc, (y1 - y0) * sc) {
                    pm.fill_rect(rect, &paint, Transform::identity(), None);
                }
            }
        }
        e.rendered = Some(r);
        e.pixmap = Some(pm);
        if let Some(d) = Self::display(e, e.anims.iter().map(|a| a.start).max().unwrap_or(0)) {
            e.pixmap = Some(renderer.render(&d).1);
        }
    }

    fn draw_at(renderer: &Renderer, e: &mut Entry, now: u64) {
        match Self::display(e, now) {
            Some(d) => e.pixmap = Some(renderer.render(&d).1),
            None => e.pixmap = Some(renderer.render(&e.surface).1),
        }
    }
}
