//! MX tree -> taffy tree, mirroring the DOM renderer's stylesheet (packages/renderer/src/styles.ts) rule
//! for rule, so the two renderers lay out the same boxes. Every MX node becomes one Item carrying its
//! paint description; internal items (metric lines, table cells, tab bars) carry no id.
use crate::space::{hit_of, layout_space, SpaceLayout};
use crate::flow::{layout_flow, FlowLayout};
use crate::text::Fonts;
use crate::theme::{contrast, over, Theme};
use crate::Images;
use mx::{Box as MxBox, Diag, Surface, Value};
use std::collections::BTreeMap;
use taffy::prelude::*;
use taffy::{compute_leaf_layout, AlignmentSafety, LayoutInput, LayoutOutput, TaffyTree};

pub const SURFACE_W: f32 = 728.0;
const BODY_LH: f32 = 14.0 * 1.45;

#[derive(Clone, Debug)]
pub enum Leaf {
    Text { text: String, px: f32, weight: u16, lh: f32, nowrap: bool },
    Fixed { w: f32, h: f32 },
    /// width 100%, height = width * ratio (chart SVGs with a viewBox)
    Aspect { ratio: f32 },
    /// intrinsic size, scaled down proportionally to fit (flow SVGs)
    Scaled { w: f32, h: f32 },
}

#[derive(Clone, Debug, Default)]
pub struct TextSpec {
    pub text: String,
    pub px: f32,
    pub weight: u16,
    pub lh: f32,
    pub color: String,
    pub nowrap: bool,
    pub center: bool,
}

#[derive(Clone, Debug, Default)]
pub enum Shape {
    #[default]
    None,
    Check(bool),
    Switch(bool),
    Range(f32),
    Chart(usize),
    Flow(usize, FlowLayout),
    Image(String),
    Space(usize, SpaceLayout),
}

#[derive(Clone, Debug, Default)]
pub struct Paint {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub radius: f32,
    pub text: Option<TextSpec>,
    pub shape: Shape,
    pub opacity: f32,
    /// 1px line along the bottom edge (table cells, tab bars)
    pub bottom_line: Option<String>,
    /// 2px line along the bottom edge (selected tab)
    pub underline: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Role {
    #[default]
    None,
    /// an interactive target: flagged `!tiny` under 24px
    Target,
}

/// What a pointer or a key does to an item: the renderer's event model, shared by the window and the wasm host.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Hit {
    #[default]
    None,
    Button(String),
    Check(String),
    Toggle(String),
    Tab(String, String),
    Slider(String, f64, f64),
    Field(String),
    Select(String),
    Link(String, String),
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: Option<String>,
    pub role: Role,
    pub hit: Hit,
    pub clip_addr: Option<String>,
    pub leaf: Option<Leaf>,
    pub paint: Paint,
    pub pad: (f32, f32),
    pub border: f32,
    pub children: Vec<usize>,
    pub node: NodeId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug)]
pub struct Laid {
    pub items: Vec<Item>,
    pub root: usize,
    pub width: f32,
    pub height: f32,
}

impl Laid {
    /// What takes input inside spaces: positioned interactive nodes by their box, and the items of nested
    /// default looks (a positioned select's control, say), all in surface pixels.
    pub fn space_hits(&self, s: &Surface) -> Vec<(Hit, MxBox)> {
        let mut out = vec![];
        for it in &self.items {
            if let Shape::Space(_, lay) = &it.paint.shape {
                let sc = if lay.width > 0.0 { it.w / lay.width } else { 1.0 };
                for p in &lay.placed {
                    let nd = &s.nodes[p.node];
                    if p.interactive && p.sub.is_none() {
                        let b = MxBox { x: (it.x + p.x * sc).round() as i32, y: (it.y + p.y * sc).round() as i32, w: (p.w * sc).round() as i32, h: (p.h * sc).round() as i32 };
                        out.push((hit_of(nd), b));
                    }
                    if let Some(sub) = &p.sub {
                        for si in &sub.items {
                            if si.hit != Hit::None {
                                out.push((si.hit.clone(), MxBox { x: (it.x + (p.x + si.x) * sc).round() as i32, y: (it.y + (p.y + si.y) * sc).round() as i32, w: (si.w * sc).round() as i32, h: (si.h * sc).round() as i32 }));
                            }
                        }
                    }
                }
            }
        }
        out
    }

    fn space_hits_ids(&self, lay: &SpaceLayout, it: &Item, sc: f32) -> Vec<(String, MxBox)> {
        let _ = self;
        lay.placed.iter().filter(|p| p.interactive).map(|p| (p.id.clone(), MxBox { x: (it.x + p.x * sc).round() as i32, y: (it.y + p.y * sc).round() as i32, w: (p.w * sc).round() as i32, h: (p.h * sc).round() as i32 })).collect()
    }

    pub fn boxes(&self) -> BTreeMap<String, MxBox> {
        let mut out = BTreeMap::new();
        for it in &self.items {
            if let Some(id) = &it.id {
                if !id.ends_with('\u{0}') {
                    out.insert(id.clone(), MxBox { x: it.x.round() as i32, y: it.y.round() as i32, w: it.w.round() as i32, h: it.h.round() as i32 });
                }
            }
            if let Shape::Space(_, lay) = &it.paint.shape {
                let sc = if lay.width > 0.0 { it.w / lay.width } else { 1.0 };
                for p in &lay.placed {
                    out.insert(p.id.clone(), MxBox { x: (it.x + p.x * sc).round() as i32, y: (it.y + p.y * sc).round() as i32, w: (p.w * sc).round() as i32, h: (p.h * sc).round() as i32 });
                }
            }
            if let Shape::Flow(_, fl) = &it.paint.shape {
                let sc = if fl.width > 0.0 { it.w / fl.width } else { 1.0 };
                for p in &fl.nodes {
                    out.insert(p.id.clone(), MxBox { x: (it.x + p.x * sc).round() as i32, y: (it.y + p.y * sc).round() as i32, w: (p.w * sc).round() as i32, h: (p.h * sc).round() as i32 });
                }
            }
        }
        out
    }
}

fn lpv(v: LengthPercentage) -> f32 {
    let c = v.into_raw();
    if c.is_length_or_percentage() { c.value() } else { 0.0 }
}
fn ai(k: AlignItemsKeyword) -> AlignItems {
    AlignItems { keyword: k, safety: AlignmentSafety::default() }
}
fn jc(k: AlignContentKeyword) -> JustifyContent {
    JustifyContent { keyword: k, safety: AlignmentSafety::default() }
}
fn rect_lp(x: f32, y: f32) -> Rect<LengthPercentage> {
    Rect { left: length(x), right: length(x), top: length(y), bottom: length(y) }
}
fn col_style(gap: f32) -> Style {
    Style { display: Display::Flex, flex_direction: FlexDirection::Column, gap: Size { width: length(0.0), height: length(gap) }, ..Default::default() }
}
fn row_style(gap: f32) -> Style {
    Style { display: Display::Flex, flex_direction: FlexDirection::Row, gap: Size { width: length(gap), height: length(0.0) }, align_items: ai(AlignItemsKeyword::Center), ..Default::default() }
}
fn leaf_style() -> Style {
    Style { min_size: Size { width: length(0.0), height: auto() }, ..Default::default() }
}

/// One element of a buffer while an `each` template is instantiated: `=key`, `=value`, `=index`.
#[derive(Clone, Debug)]
struct Ctx {
    key: String,
    value: f64,
    index: usize,
}

struct Builder<'a> {
    s: &'a Surface,
    fonts: &'a Fonts,
    images: &'a Images,
    tree: TaffyTree<Leaf>,
    items: Vec<Item>,
    ctx: Option<Ctx>,
}

impl<'a> Builder<'a> {
    /// A binding's text: the element's key, value or index inside an `each`, else the buffer's value.
    fn resolve(&self, bind: &str) -> Option<String> {
        if let Some(c) = &self.ctx {
            match bind {
                "key" => return Some(c.key.clone()),
                "value" => return Some(fmt_num(c.value)),
                "index" => return Some((c.index + 1).to_string()),
                _ => {}
            }
        }
        crate::bind_value(self.s, bind)
    }

    fn add(&mut self, mut style: Style, leaf: Option<Leaf>, paint: Paint, id: Option<String>, children: Vec<usize>) -> usize {
        // inside an `each`, every instance of a template node gets its element's key as a suffix
        let id = match (&id, &self.ctx) {
            (Some(i), Some(c)) if !i.ends_with('\u{0}') => Some(format!("{}.{}", i, c.key)),
            _ => id,
        };
        let pad = (lpv(style.padding.left), lpv(style.padding.top));
        let border = lpv(style.border.left);
        if leaf.is_some() && style.min_size.width == auto() {
            style.min_size.width = length(0.0);
        }
        let node = match &leaf {
            Some(l) => self.tree.new_leaf_with_context(style, l.clone()).unwrap(),
            None => {
                let kids: Vec<NodeId> = children.iter().map(|&c| self.items[c].node).collect();
                self.tree.new_with_children(style, &kids).unwrap()
            }
        };
        self.items.push(Item { id, role: Role::None, hit: Hit::None, clip_addr: None, leaf, paint, pad, border, children, node, x: 0.0, y: 0.0, w: 0.0, h: 0.0 });
        self.items.len() - 1
    }

    fn text(&mut self, text: &str, px: f32, weight: u16, lh: f32, color: &str, nowrap: bool, style: Style, paint: Paint) -> usize {
        let spec = TextSpec { text: text.to_string(), px, weight, lh, color: color.to_string(), nowrap, center: false };
        let paint = Paint { text: Some(spec), ..paint };
        self.add(style, Some(Leaf::Text { text: text.to_string(), px, weight, lh, nowrap }), paint, None, vec![])
    }

    fn tone_color(tone: Option<&str>) -> String {
        tone.unwrap_or("fg").to_string()
    }

    fn build(&mut self, n: usize, in_row: bool) -> usize {
        let nd = &self.s.nodes[n];
        let ty = nd.ty.as_str();
        let id = Some(nd.id.clone());
        let nd_id = nd.id.clone();
        let a = &nd.args;
        let label = nd.label.clone().unwrap_or_default();
        let tone = a.tone.clone();
        let idx = match ty {
            "col" | "stack" => {
                let kids = self.kids(n, false);
                self.add(col_style(12.0), None, Paint::default(), id, kids)
            }
            "row" => {
                let kids = self.kids(n, true);
                let mut st = row_style(12.0);
                if nd.flags.contains("end") {
                    st.justify_content = jc(AlignContentKeyword::FlexEnd);
                } else if nd.flags.contains("center") {
                    st.justify_content = jc(AlignContentKeyword::Center);
                } else if nd.flags.contains("between") {
                    st.justify_content = jc(AlignContentKeyword::SpaceBetween);
                }
                self.add(st, None, Paint::default(), id, kids)
            }
            "grid" => {
                let kids = self.kids(n, false);
                let cols = a.grid_cols.unwrap_or(2) as u16;
                let st = Style { display: Display::Grid, grid_template_columns: evenly_sized_tracks(cols), gap: Size { width: length(12.0), height: length(12.0) }, ..Default::default() };
                self.add(st, None, Paint::default(), id, kids)
            }
            "card" => {
                let mut kids = vec![];
                if nd.label.is_some() {
                    let mut st = leaf_style();
                    st.margin.top = length(-2.0);
                    kids.push(self.text(&label, 15.0, 600, 15.0 * 1.45, "fg", true, st, Paint::default()));
                }
                kids.extend(self.kids(n, false));
                let mut st = col_style(12.0);
                st.padding = rect_lp(16.0, 14.0);
                st.border = rect_lp(1.0, 1.0);
                let paint = Paint { fill: Some("card".into()), stroke: Some("line".into()), radius: 8.0, ..Default::default() };
                self.add(st, None, paint, id, kids)
            }
            "tabs" => {
                let tabs = nd.children.clone();
                let mut buttons = vec![];
                let mut panel: Option<usize> = None;
                for &t in &tabs {
                    let tn = &self.s.nodes[t];
                    let sel = tn.flags.contains("selected");
                    let tl = tn.label.clone().unwrap_or_default();
                    let mut st = leaf_style();
                    st.padding = rect_lp(12.0, 8.0);
                    st.border.bottom = length(2.0);
                    st.margin.bottom = length(-1.0);
                    let paint = Paint { underline: if sel { Some("primary".into()) } else { None }, ..Default::default() };
                    let b = self.text(&tl, 14.0, if sel { 600 } else { 400 }, BODY_LH, if sel { "fg" } else { "fg2" }, true, st, paint);
                    self.items[b].id = Some(tn.id.clone());
                    self.items[b].role = Role::Target;
                    self.items[b].clip_addr = Some(tn.id.clone());
                    self.items[b].hit = Hit::Tab(nd_id.clone(), tl.clone());
                    buttons.push(b);
                    if sel && panel.is_none() {
                        let kids = self.kids(t, false);
                        let mut st = col_style(12.0);
                        st.padding.top = length(12.0);
                        panel = Some(self.add(st, None, Paint::default(), None, kids));
                    }
                }
                let mut bar_st = row_style(4.0);
                bar_st.align_items = ai(AlignItemsKeyword::Stretch);
                bar_st.border.bottom = length(1.0);
                let bar = self.add(bar_st, None, Paint { bottom_line: Some("line".into()), ..Default::default() }, None, buttons);
                let mut kids = vec![bar];
                if let Some(p) = panel {
                    kids.push(p);
                }
                self.add(col_style(0.0), None, Paint::default(), id, kids)
            }
            "h1" | "h2" | "h3" => {
                let label = a.bind.as_deref().and_then(|b| self.resolve(b)).unwrap_or(label.clone());
                let px = match ty {
                    "h1" => 20.0,
                    "h2" => 17.0,
                    _ => 15.0,
                };
                self.text(&label, px, 650, px * 1.25, &Self::tone_color(tone.as_deref()), false, leaf_style(), Paint::default())
            }
            "text" => {
                let shown = a.bind.as_deref().and_then(|b| self.resolve(b)).unwrap_or(label.clone());
                self.text(&shown, 14.0, 400, BODY_LH, &Self::tone_color(tone.as_deref()), false, leaf_style(), Paint::default())
            }
            "each" => {
                let data = a.bind.as_deref().and_then(|b| self.s.get(b)).map(|b| self.s.nodes[b].args.data.clone()).unwrap_or_default();
                let template = nd.children.clone();
                let saved = self.ctx.clone();
                let mut kids = vec![];
                for (i, d) in data.iter().enumerate() {
                    self.ctx = Some(Ctx { key: d.0.clone(), value: d.1, index: i });
                    let in_row = nd.flags.contains("row");
                    for &c in &template {
                        kids.push(self.build(c, in_row));
                    }
                }
                self.ctx = saved;
                let st = if let Some(cols) = a.grid_cols {
                    Style { display: Display::Grid, grid_template_columns: evenly_sized_tracks(cols as u16), gap: Size { width: length(12.0), height: length(12.0) }, ..Default::default() }
                } else if nd.flags.contains("row") {
                    row_style(12.0)
                } else {
                    col_style(12.0)
                };
                self.add(st, None, Paint::default(), id, kids)
            }
            "series" => {
                let st = Style { display: Display::None, ..Default::default() };
                self.add(st, Some(Leaf::Fixed { w: 0.0, h: 0.0 }), Paint::default(), id, vec![])
            }
            _ if mx::parse::is_space(nd) && !nd.children.is_empty() => {
                // a sized node with positioned children: the space, with a caption for a labelled region or figure
                // and the card look for a card
                let lay = layout_space(self.s, n, self.fonts, self.images);
                let mut kids = vec![];
                if nd.label.is_some() && ["region", "figure", "item"].contains(&ty) {
                    let mut st = leaf_style();
                    st.margin.bottom = length(4.0);
                    kids.push(self.text(&label, 12.0, 400, 12.0 * 1.45, "fg2", false, st, Paint::default()));
                }
                let (w, h) = (lay.width, lay.height);
                let mut st = leaf_style();
                st.max_size.width = percent(1.0);
                st.align_self = Some(ai(AlignItemsKeyword::FlexStart));
                let fill = a.style.get("fill").cloned().or(if ty == "card" { Some("card".into()) } else { None });
                let stroke = a.style.get("stroke").cloned().or(if ty == "card" { Some("line".into()) } else { None });
                let radius = a.style.get("radius").and_then(|r| r.parse().ok()).unwrap_or(if ty == "card" { 8.0 } else { 0.0 });
                let sp = self.add(st, Some(Leaf::Scaled { w, h }), Paint { shape: Shape::Space(n, lay), fill, stroke, radius, ..Default::default() }, id, vec![]);
                kids.push(sp);
                let wrap = self.add(col_style(0.0), None, Paint::default(), None, kids);
                self.items[wrap].id = Some(format!("{}\u{0}", nd_id)); // never read: keeps the wrapper from taking the id below
                wrap
            }
            "link" => {
                let l = self.text(&label, 14.0, 400, BODY_LH, "primary", false, leaf_style(), Paint::default());
                self.items[l].hit = Hit::Link(nd_id.clone(), a.href.clone().unwrap_or_default());
                l
            }
            "badge" => {
                let label = a.bind.as_deref().and_then(|b| self.resolve(b)).unwrap_or(label.clone());
                let t = tone.clone().unwrap_or_else(|| "muted".into());
                let mut st = leaf_style();
                st.padding = rect_lp(8.0, 1.0);
                let paint = Paint { fill: Some(format!("{}-soft", t)), radius: 999.0, ..Default::default() };
                self.text(&label, 12.0, 600, 18.0, &t, true, st, paint)
            }
            "image" => {
                let src = label.clone();
                let (w, h) = self.images.get(&src).map(|p| (p.width() as f32, p.height() as f32)).unwrap_or((0.0, 0.0));
                let mut st = leaf_style();
                st.align_self = Some(ai(AlignItemsKeyword::FlexStart));
                st.max_size.width = percent(1.0);
                self.add(st, Some(Leaf::Fixed { w, h }), Paint { shape: Shape::Image(src), ..Default::default() }, id, vec![])
            }
            "metric" => {
                let label = a.bind_label.as_deref().and_then(|b| self.resolve(b)).unwrap_or(label.clone());
                let l = self.text(&label, 12.0, 400, 12.0 * 1.45, "fg2", true, leaf_style(), Paint::default());
                let shown = a.bind.as_deref().and_then(|b| self.resolve(b)).unwrap_or_else(|| value_str(&a.value));
                let v = self.text(&shown, 22.0, 650, 22.0 * 1.2, "fg", true, leaf_style(), Paint::default());
                let mut kids = vec![l, v];
                if let Some(d) = &a.delta {
                    kids.push(self.text(d, 12.0, 600, 12.0 * 1.45, &Self::tone_color(tone.as_deref()), false, leaf_style(), Paint::default()));
                }
                for &k in &kids {
                    self.items[k].clip_addr = Some(nd.id.clone());
                }
                let mut st = col_style(0.0);
                st.padding = rect_lp(14.0, 10.0);
                st.border = rect_lp(1.0, 1.0);
                st.min_size.width = length(0.0);
                let paint = Paint { fill: Some("card".into()), stroke: Some("line".into()), radius: 8.0, ..Default::default() };
                self.add(st, None, paint, id, kids)
            }
            "bar" | "line" | "area" | "pie" => {
                let mut kids = vec![];
                if nd.label.is_some() {
                    let mut st = leaf_style();
                    st.margin.bottom = length(4.0);
                    kids.push(self.text(&label, 12.0, 400, 12.0 * 1.45, "fg2", false, st, Paint::default()));
                }
                let svg = self.add(leaf_style(), Some(Leaf::Aspect { ratio: 200.0 / 640.0 }), Paint { shape: Shape::Chart(n), ..Default::default() }, None, vec![]);
                kids.push(svg);
                self.add(col_style(0.0), None, Paint::default(), id, kids)
            }
            "flow" => {
                let fl = layout_flow(self.s, nd, self.fonts);
                let mut kids = vec![];
                if nd.label.is_some() {
                    let mut st = leaf_style();
                    st.margin.bottom = length(4.0);
                    kids.push(self.text(&label, 12.0, 400, 12.0 * 1.45, "fg2", false, st, Paint::default()));
                }
                let (w, h) = (fl.width, fl.height);
                let mut st = leaf_style();
                st.max_size.width = percent(1.0);
                st.align_self = Some(ai(AlignItemsKeyword::FlexStart));
                let svg = self.add(st, Some(Leaf::Scaled { w, h }), Paint { shape: Shape::Flow(n, fl), ..Default::default() }, None, vec![]);
                kids.push(svg);
                self.add(col_style(0.0), None, Paint::default(), id, kids)
            }
            "table" => {
                let cols = a.cols.clone();
                let rows = a.rows.clone();
                let cell_style = |pad_y: f32, last: bool| -> Style {
                    let mut st = leaf_style();
                    st.padding = rect_lp(8.0, pad_y);
                    if !last {
                        st.border.bottom = length(1.0);
                    }
                    st.flex_grow = 1.0;
                    st.flex_basis = length(0.0);
                    st.min_size.width = length(0.0);
                    st
                };
                let mut row_items = vec![];
                let mut hcells = vec![];
                for (c, _) in &cols {
                    let line = Paint { bottom_line: Some("line".into()), ..Default::default() };
                    hcells.push(self.text(c, 12.0, 600, 12.0 * 1.45, "fg2", true, cell_style(6.0, false), line));
                }
                let mut hst = row_style(0.0);
                hst.align_items = ai(AlignItemsKeyword::Stretch);
                row_items.push(self.add(hst, None, Paint::default(), None, hcells));
                let nrows = rows.len();
                for (ri, r) in rows.iter().enumerate() {
                    let last = ri + 1 == nrows;
                    let mut cells = vec![];
                    for (ci, (cname, kind)) in cols.iter().enumerate() {
                        let cell = r.get(ci).cloned().unwrap_or_default();
                        let line = Paint { bottom_line: if last { None } else { Some("line".into()) }, ..Default::default() };
                        let it = if kind.as_deref() == Some("badge") {
                            let (txt, t) = mx::badge_tone(&cell);
                            let mut bst = leaf_style();
                            bst.padding = rect_lp(8.0, 1.0);
                            bst.flex_shrink = 0.0;
                            let pill = self.text(&txt, 12.0, 600, 18.0, &t, true, bst, Paint { fill: Some(format!("{}-soft", t)), radius: 999.0, ..Default::default() });
                            let mut cst = cell_style(7.0, last);
                            cst.min_size.height = length(if last { 35.0 } else { 36.0 });
                            cst.display = Display::Flex;
                            cst.flex_direction = FlexDirection::Row;
                            cst.align_items = ai(AlignItemsKeyword::Center);
                            self.add(cst, None, line, None, vec![pill])
                        } else {
                            self.text(&cell, 14.0, 400, BODY_LH, "fg", true, cell_style(7.0, last), line)
                        };
                        self.items[it].clip_addr = Some(format!("{}.{}.{}", nd.id, r.first().cloned().unwrap_or_default(), cname));
                        cells.push(it);
                    }
                    let mut rst = row_style(0.0);
                    rst.align_items = ai(AlignItemsKeyword::Stretch);
                    row_items.push(self.add(rst, None, Paint::default(), None, cells));
                }
                self.add(col_style(0.0), None, Paint::default(), id, row_items)
            }
            "button" => {
                let disabled = nd.flags.contains("disabled");
                let (fill, color, stroke) = match tone.as_deref() {
                    Some("primary") => ("primary", "primary-fg", "primary"),
                    Some("danger") => ("danger", "on-tone", "danger"),
                    Some("success") => ("success", "on-tone", "success"),
                    _ => ("card", "fg", "line"),
                };
                let mut st = leaf_style();
                st.padding = rect_lp(14.0, 7.0);
                st.border = rect_lp(1.0, 1.0);
                st.max_size.width = percent(1.0);
                let paint = Paint { fill: Some(fill.into()), stroke: Some(stroke.into()), radius: 8.0, opacity: if disabled { 0.5 } else { 1.0 }, ..Default::default() };
                let b = self.text(&label, 14.0, 600, BODY_LH, color, true, st, paint);
                self.items[b].paint.text.as_mut().unwrap().center = true;
                self.items[b].role = Role::Target;
                self.items[b].clip_addr = Some(nd.id.clone());
                if !disabled {
                    self.items[b].hit = Hit::Button(nd_id.clone());
                }
                b
            }
            "field" => {
                let lab = if nd.flags.contains("required") { format!("{} *", label) } else { label.clone() };
                let l = self.text(&lab, 12.0, 400, 12.0 * 1.45, "fg2", false, leaf_style(), Paint::default());
                let kind = a.kind.clone().unwrap_or_else(|| "text".into());
                let h = if kind == "date" { BODY_LH + 2.3 } else { BODY_LH };
                let val = match &a.value {
                    Value::Str(v) => v.clone(),
                    v => value_str(v),
                };
                let mut st = leaf_style();
                st.padding = rect_lp(10.0, 7.0);
                st.border = rect_lp(1.0, 1.0);
                let spec = TextSpec { text: if kind == "password" { "•".repeat(val.chars().count()) } else { val }, px: 14.0, weight: 400, lh: BODY_LH, color: "fg".into(), nowrap: true, center: false };
                let paint = Paint { fill: Some("card".into()), stroke: Some("line".into()), radius: 8.0, text: Some(spec), ..Default::default() };
                let input = self.add(st, Some(Leaf::Fixed { w: 0.0, h }), paint, None, vec![]);
                self.items[input].role = Role::Target;
                self.items[input].clip_addr = Some(nd.id.clone());
                self.items[input].hit = Hit::Field(nd_id.clone());
                let mut cst = col_style(4.0);
                cst.min_size.width = length(0.0);
                self.add(cst, None, Paint::default(), id, vec![l, input])
            }
            "select" => {
                let l = self.text(&label, 12.0, 400, 12.0 * 1.45, "fg2", false, leaf_style(), Paint::default());
                let chosen = match &a.value {
                    Value::List(v) => v.first().cloned().unwrap_or_default(),
                    Value::Str(v) => v.clone(),
                    _ => String::new(),
                };
                let widest = a.options.iter().map(|o| self.fonts.width(o, 14.0, 400)).fold(0.0_f32, f32::max);
                let mut st = leaf_style();
                st.padding = rect_lp(10.0, 7.0);
                st.border = rect_lp(1.0, 1.0);
                let spec = TextSpec { text: chosen, px: 14.0, weight: 400, lh: BODY_LH - 2.7, color: "fg".into(), nowrap: true, center: false };
                let paint = Paint { fill: Some("card".into()), stroke: Some("line".into()), radius: 8.0, text: Some(spec), ..Default::default() };
                let sel = self.add(st, Some(Leaf::Fixed { w: widest.ceil() + 16.0, h: BODY_LH - 2.7 }), paint, None, vec![]);
                self.items[sel].role = Role::Target;
                self.items[sel].hit = Hit::Select(nd_id.clone());
                let mut cst = col_style(4.0);
                if in_row {
                    cst.align_items = ai(AlignItemsKeyword::FlexStart);
                }
                self.add(cst, None, Paint::default(), id, vec![l, sel])
            }
            "check" => {
                let on = a.value == Value::Bool(true);
                let mut bst = leaf_style();
                bst.flex_shrink = 0.0;
                let bx = self.add(bst, Some(Leaf::Fixed { w: 16.0, h: 16.0 }), Paint { shape: Shape::Check(on), ..Default::default() }, None, vec![]);
                let l = self.text(&label, 14.0, 400, BODY_LH, "fg", false, leaf_style(), Paint::default());
                let mut st = row_style(10.0);
                st.min_size.height = length(24.0);
                let it = self.add(st, None, Paint::default(), id, vec![bx, l]);
                self.items[it].role = Role::Target;
                self.items[it].hit = Hit::Check(nd_id.clone());
                it
            }
            "toggle" => {
                let on = a.value == Value::Bool(true);
                let l = self.text(&label, 14.0, 400, BODY_LH, "fg", false, leaf_style(), Paint::default());
                let mut sst = leaf_style();
                sst.flex_shrink = 0.0;
                let sw = self.add(sst, Some(Leaf::Fixed { w: 36.0, h: 20.0 }), Paint { shape: Shape::Switch(on), ..Default::default() }, None, vec![]);
                let mut st = row_style(10.0);
                st.justify_content = jc(AlignContentKeyword::SpaceBetween);
                st.padding = rect_lp(0.0, 6.0);
                st.min_size.height = length(36.0);
                let it = self.add(st, None, Paint::default(), id, vec![l, sw]);
                self.items[it].role = Role::Target;
                self.items[it].hit = Hit::Toggle(nd_id.clone());
                it
            }
            "slider" => {
                let (min, max) = (a.min.unwrap_or(0.0), a.max.unwrap_or(100.0));
                let v = match a.value {
                    Value::Num(x) => x,
                    _ => min,
                };
                let frac = if max > min { ((v - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
                let l = self.text(&label, 14.0, 400, BODY_LH, "fg", false, leaf_style(), Paint::default());
                let vt = self.text(&crate::layout::fmt_num(v), 14.0, 400, BODY_LH, "fg", true, leaf_style(), Paint::default());
                let mut hst = row_style(0.0);
                hst.justify_content = jc(AlignContentKeyword::SpaceBetween);
                let head = self.add(hst, None, Paint::default(), None, vec![l, vt]);
                let range = self.add(leaf_style(), Some(Leaf::Fixed { w: 0.0, h: 16.0 }), Paint { shape: Shape::Range(frac as f32), ..Default::default() }, None, vec![]);
                self.items[range].hit = Hit::Slider(nd_id.clone(), min, max);
                self.add(col_style(2.0), None, Paint::default(), id, vec![head, range])
            }
            "region" | "item" | "figure" | "decoration" => {
                // without children a generic role is just its label
                self.text(&label, 14.0, 400, BODY_LH, "fg2", false, leaf_style(), Paint::default())
            }
            _ => self.text(&format!("{}?", ty), 14.0, 400, BODY_LH, "danger", false, leaf_style(), Paint::default()),
        };
        if self.items[idx].id.is_none() {
            self.items[idx].id = Some(nd_id.clone());
        }
        if in_row {
            let node = self.items[idx].node;
            let mut st = self.tree.style(node).unwrap().clone();
            st.min_size.width = length(0.0);
            if ty == "metric" || ty == "card" {
                st.flex_grow = 1.0;
                st.flex_basis = length(0.0);
                st.align_self = Some(ai(AlignItemsKeyword::Stretch));
            }
            self.tree.set_style(node, st).unwrap();
        }
        idx
    }

    fn kids(&mut self, n: usize, in_row: bool) -> Vec<usize> {
        let children = self.s.nodes[n].children.clone();
        children.into_iter().map(|c| self.build(c, in_row)).collect()
    }
}

pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

pub fn value_str(v: &Value) -> String {
    match v {
        Value::None => String::new(),
        Value::Str(s) => s.clone(),
        Value::Num(n) => fmt_num(*n),
        Value::Bool(b) => if *b { "on".into() } else { "off".into() },
        Value::List(l) => l.join(", "),
    }
}

/// Lay out a surface at the standard width. Returns every item with absolute, surface-relative positions.
pub fn layout(s: &Surface, fonts: &Fonts, width: f32, images: &Images) -> Laid {
    let mut b = Builder { s, fonts, images, tree: TaffyTree::new(), items: vec![], ctx: None };
    let kids = b.kids(s.root(), false);
    let mut st = col_style(12.0);
    st.padding = rect_lp(16.0, 16.0);
    st.border = rect_lp(1.0, 1.0);
    st.size.width = length(width);
    let root = b.add(st, None, Paint { fill: Some("bg".into()), stroke: Some("line".into()), radius: 12.0, ..Default::default() }, Some("root".into()), kids);
    let root_node = b.items[root].node;
    compute(&mut b, root_node, Size { width: AvailableSpace::Definite(width), height: AvailableSpace::MaxContent }, fonts);
    let Builder { tree, mut items, .. } = b;
    place(&tree, &mut items, root, 0.0, 0.0);
    let (w, h) = (items[root].w, items[root].h);
    Laid { items, root, width: w, height: h }
}

fn compute(b: &mut Builder, root_node: NodeId, avail: Size<AvailableSpace>, fonts: &Fonts) {
    b.tree
        .compute_layout_with_measure(root_node, avail, |inputs: LayoutInput, _id, ctx: Option<&mut Leaf>, style: &Style| -> LayoutOutput {
            compute_leaf_layout(inputs, style, |_, _| 0.0, |known: Size<Option<f32>>, avail: Size<AvailableSpace>| {
                let avail_w = match avail.width {
                    AvailableSpace::Definite(w) => Some(w),
                    _ => None,
                };
                match ctx {
                    Some(Leaf::Text { text, px, weight, lh, nowrap }) => {
                        if *nowrap {
                            return Size { width: known.width.unwrap_or_else(|| fonts.width(text, *px, *weight).ceil()), height: known.height.unwrap_or(*lh) };
                        }
                        let max_w = known.width.or(avail_w).unwrap_or(f32::INFINITY);
                        let lines = fonts.wrap(text, *px, *weight, max_w);
                        let widest = lines.iter().map(|l| fonts.width(l, *px, *weight)).fold(0.0_f32, f32::max);
                        Size { width: known.width.unwrap_or(widest.min(max_w).ceil()), height: known.height.unwrap_or(lines.len() as f32 * *lh) }
                    }
                    Some(Leaf::Fixed { w, h }) => Size { width: known.width.unwrap_or(*w), height: known.height.unwrap_or(*h) },
                    Some(Leaf::Aspect { ratio }) => {
                        let w = known.width.or(avail_w).unwrap_or(640.0);
                        Size { width: w, height: known.height.unwrap_or(w * *ratio) }
                    }
                    Some(Leaf::Scaled { w, h }) => {
                        let ww = known.width.unwrap_or_else(|| avail_w.map(|a| a.min(*w)).unwrap_or(*w));
                        Size { width: ww, height: known.height.unwrap_or(if *w > 0.0 { *h * ww / *w } else { *h }) }
                    }
                    None => Size::ZERO,
                }
            })
        })
        .unwrap();
}

fn place(tree: &TaffyTree<Leaf>, items: &mut Vec<Item>, i: usize, ox: f32, oy: f32) {
    let l = *tree.layout(items[i].node).unwrap();
    items[i].x = ox + l.location.x;
    items[i].y = oy + l.location.y;
    items[i].w = l.size.width;
    items[i].h = l.size.height;
    let (x, y) = (items[i].x, items[i].y);
    let kids = items[i].children.clone();
    for c in kids {
        place(tree, items, c, x, y);
    }
}

/// Lay out one node on its own (the default look of a positioned node): at `width` if given, else as wide
/// as its content wants. Positions are relative to the node's own top-left.
pub fn layout_sub(s: &Surface, n: usize, fonts: &Fonts, images: &Images, width: Option<f32>) -> Laid {
    let mut b = Builder { s, fonts, images, tree: TaffyTree::new(), items: vec![], ctx: None };
    let inner = b.build(n, false);
    let mut st = col_style(0.0);
    if let Some(w) = width {
        st.size.width = length(w);
    }
    let root = b.add(st, None, Paint::default(), None, vec![inner]);
    let root_node = b.items[root].node;
    let avail = Size { width: width.map(AvailableSpace::Definite).unwrap_or(AvailableSpace::MaxContent), height: AvailableSpace::MaxContent };
    compute(&mut b, root_node, avail, fonts);
    let Builder { tree, mut items, .. } = b;
    place(&tree, &mut items, root, 0.0, 0.0);
    let (w, h) = (items[root].w, items[root].h);
    Laid { items, root, width: w, height: h }
}

/// Renderer diagnostics from the layout: clipped text, small targets, overlapping diagram parts.
pub fn diagnostics(laid: &Laid, fonts: &Fonts, theme: &Theme) -> Vec<Diag> {
    let mut out = vec![];
    for it in &laid.items {
        if let (Some(addr), Some(spec)) = (&it.clip_addr, &it.paint.text) {
            if spec.nowrap {
                let needed = (fonts.width(&spec.text, spec.px, spec.weight) + 2.0 * it.pad.0).ceil() as i32;
                let shown = (it.w - 2.0 * it.border).round() as i32;
                if needed > shown + 1 {
                    out.push(Diag::Overflow { addr: addr.clone(), shown, needed });
                }
            }
        }
    }
    for it in &laid.items {
        if it.role == Role::Target && (it.w < 24.0 || it.h < 24.0) {
            if let Some(addr) = it.id.clone().or_else(|| it.clip_addr.clone()) {
                out.push(Diag::Tiny { addr, w: it.w.round() as i32, h: it.h.round() as i32 });
            }
        }
    }
    // Text contrast below WCAG AA, once per node (worst ratio), against the effective background.
    let mut worst: Vec<(String, f64)> = vec![];
    fn walk(laid: &Laid, i: usize, bg: tiny_skia::Color, owner: &str, theme: &Theme, worst: &mut Vec<(String, f64)>) {
        // Computed colours, as the DOM's getComputedStyle reports them: a disabled button's opacity is a
        // composite effect and does not change its colour.
        let it = &laid.items[i];
        let bg = match &it.paint.fill {
            Some(f) => over(theme.color(f), 1.0, bg),
            None => bg,
        };
        let owner = it.id.as_deref().or(it.clip_addr.as_deref()).unwrap_or(owner);
        if let Some(t) = &it.paint.text {
            if !t.text.trim().is_empty() {
                let fg = theme.color(&t.color);
                let ratio = contrast(fg, bg);
                let need = if t.px >= 24.0 || (t.px >= 18.66 && t.weight >= 600) { 3.0 } else { 4.5 };
                if ratio < need {
                    match worst.iter_mut().find(|w| w.0 == owner) {
                        Some(w) => w.1 = w.1.min(ratio),
                        None => worst.push((owner.to_string(), ratio)),
                    }
                }
            }
        }
        for &c in &it.children {
            walk(laid, c, bg, owner, theme, worst);
        }
    }
    walk(laid, laid.root, theme.color("bg"), "root", theme, &mut worst);
    for (addr, ratio) in worst {
        out.push(Diag::Contrast { addr, ratio: (ratio * 100.0).round() / 100.0 });
    }
    for it in &laid.items {
        if let Shape::Space(_, lay) = &it.paint.shape {
            let sc = if lay.width > 0.0 { it.w / lay.width } else { 1.0 };
            for p in &lay.placed {
                if p.off {
                    out.push(Diag::OffCanvas { addr: p.id.clone() });
                }
            }
            for (hit, b) in laid.space_hits_ids(lay, it, sc) {
                if b.w < 24 || b.h < 24 {
                    out.push(Diag::Tiny { addr: hit, w: b.w, h: b.h });
                }
            }
        }
    }
    for it in &laid.items {
        if let Shape::Flow(_, fl) = &it.paint.shape {
            let s = if fl.width > 0.0 { it.w / fl.width } else { 1.0 };
            let mut rects: Vec<(String, f32, f32, f32, f32)> = fl.nodes.iter().map(|p| (p.id.clone(), p.x * s, p.y * s, p.w * s, p.h * s)).collect();
            for e in &fl.edges {
                if let Some((lab, lx, ly)) = &e.label {
                    let w = fonts.width(lab, 11.0, 400) * s;
                    rects.push((e.id.clone(), (lx - w / 2.0) * s, (ly - 11.0) * s, w, 13.0 * s));
                }
            }
            for i in 0..rects.len() {
                for j in i + 1..rects.len() {
                    let (a, b) = (&rects[i], &rects[j]);
                    let ox = (a.1 + a.3).min(b.1 + b.3) - a.1.max(b.1);
                    let oy = (a.2 + a.4).min(b.2 + b.4) - a.2.max(b.2);
                    if ox > 2.0 && oy > 2.0 {
                        out.push(Diag::Overlap { addr: a.0.clone(), other: b.0.clone() });
                    }
                }
            }
        }
    }
    out
}
