use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A node's primary value: a field's text, a slider's number, a toggle's state, a select's choices.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(untagged)]
pub enum Value {
    #[default]
    None,
    Str(String),
    Num(f64),
    Bool(bool),
    List(Vec<String>),
}

/// A flow edge. `from` and `to` index the surface's node arena; `id` is `from>to` once ids are known.
#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub label: Option<String>,
    pub flags: BTreeSet<String>,
    pub id: String,
}

/// Typed arguments. The TypeScript core keeps a loose record; here every argument a node type can
/// carry has a field, and unknown `key=value` words land in `kv`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Args {
    pub tone: Option<String>,
    pub value: Value,
    pub delta: Option<String>,
    pub cols: Vec<(String, Option<String>)>,
    pub rows: Vec<Vec<String>>,
    pub data: Vec<(String, f64, bool)>,
    pub options: Vec<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub kind: Option<String>,
    pub href: Option<String>,
    pub alt: Option<String>,
    pub grid_cols: Option<u32>,
    pub nodes: Vec<usize>,
    pub edges: Vec<Edge>,
    pub kv: BTreeMap<String, String>,
    // -- paint layer (MX v2): positioned primitives inside a canvas
    /// `@x,y`
    pub pos: Option<(f32, f32)>,
    /// `WxH`
    pub size: Option<(f32, f32)>,
    /// a line's second point
    pub pos2: Option<(f32, f32)>,
    /// `r:N`
    pub radius: Option<f32>,
    /// SVG path data
    pub path: Option<String>,
    /// style words `fill:primary stroke:#333 width:2 opacity:0.5 size:14 weight:600 anchor:middle rotate:15 scale:2`
    pub style: BTreeMap<String, String>,
    /// `=buffer` binding (charts and text bound to a `series`)
    pub bind: Option<String>,
    /// `metric =key =value`: the label's binding
    pub bind_label: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Node {
    pub ty: String,
    pub label: Option<String>,
    pub args: Args,
    pub flags: BTreeSet<String>,
    pub children: Vec<usize>,
    pub id: String,
    pub explicit: Option<String>,
}

impl Node {
    pub fn new(ty: &str) -> Node {
        Node { ty: ty.to_string(), ..Default::default() }
    }
}

/// A parsed surface. `nodes[0]` is the root column; `ids` is in insertion (document) order, root first.
#[derive(Debug, Clone)]
pub struct Surface {
    pub id: String,
    pub life: Vec<String>,
    pub nodes: Vec<Node>,
    pub rev: u32,
    pub ids: Vec<(String, usize)>,
    index: BTreeMap<String, usize>,
}

impl Surface {
    pub fn new() -> Surface {
        let mut root = Node::new("col");
        root.id = "root".into();
        let mut s = Surface { id: String::new(), life: vec![], nodes: vec![root], rev: 0, ids: vec![], index: BTreeMap::new() };
        s.register("root", 0);
        s
    }
    pub fn root(&self) -> usize {
        0
    }
    pub fn register(&mut self, id: &str, idx: usize) {
        self.ids.push((id.to_string(), idx));
        self.index.insert(id.to_string(), idx);
    }
    pub fn get(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }
    pub fn unregister(&mut self, id: &str) {
        self.index.remove(id);
        self.ids.retain(|(i, _)| i != id);
    }
    pub fn node(&self, idx: usize) -> &Node {
        &self.nodes[idx]
    }
    pub fn node_mut(&mut self, idx: usize) -> &mut Node {
        &mut self.nodes[idx]
    }
    pub fn push(&mut self, n: Node) -> usize {
        self.nodes.push(n);
        self.nodes.len() - 1
    }
}

impl Default for Surface {
    fn default() -> Self {
        Surface::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Box {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind")]
pub enum Diag {
    #[serde(rename = "overflow")]
    Overflow { addr: String, shown: i32, needed: i32 },
    #[serde(rename = "overlap")]
    Overlap { addr: String, other: String },
    #[serde(rename = "contrast")]
    Contrast { addr: String, ratio: f64 },
    #[serde(rename = "tiny")]
    Tiny { addr: String, w: i32, h: i32 },
    /// a painted shape lies entirely outside its canvas
    #[serde(rename = "offcanvas")]
    OffCanvas { addr: String },
}

impl Diag {
    pub fn addr(&self) -> &str {
        match self {
            Diag::Overflow { addr, .. } | Diag::Overlap { addr, .. } | Diag::Contrast { addr, .. } | Diag::Tiny { addr, .. } | Diag::OffCanvas { addr } => addr,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MXError {
    pub message: String,
    pub line: Option<usize>,
}

impl MXError {
    pub fn new(message: impl Into<String>) -> MXError {
        MXError { message: message.into(), line: None }
    }
    pub fn at(self, line: usize) -> MXError {
        MXError { message: self.message, line: Some(line) }
    }
}

impl fmt::Display for MXError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(l) => write!(f, "line={} {}", l, self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for MXError {}

pub type Result<T> = std::result::Result<T, MXError>;
