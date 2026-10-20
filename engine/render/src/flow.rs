//! Layered flow-diagram layout, a port of packages/renderer/src/flow.ts: layers by topological order,
//! back edges routed below, nodes 44px high, columns as wide as their widest label.
use crate::text::Fonts;
use mx::{Edge, Node, Surface};
use std::collections::{BTreeMap, BTreeSet};

pub const NODE_H: f32 = 44.0;
pub const GAP_X: f32 = 56.0;
pub const GAP_Y: f32 = 24.0;
pub const PAD: f32 = 16.0;
pub const LANE: f32 = 28.0;

#[derive(Debug, Clone)]
pub struct PlacedNode {
    pub node: usize,
    pub id: String,
    pub label: String,
    pub tone: Option<String>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    Forward,
    Below,
    Above,
}

#[derive(Debug, Clone)]
pub struct RoutedEdge {
    pub id: String,
    pub route: Route,
    /// Polyline or, for a forward edge between different rows, a cubic: (x1,y1) C (mx,y1) (mx,y2) (x2,y2).
    pub points: Vec<(f32, f32)>,
    pub curve: bool,
    pub dashed: bool,
    pub label: Option<(String, f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct FlowLayout {
    pub nodes: Vec<PlacedNode>,
    pub edges: Vec<RoutedEdge>,
    pub width: f32,
    pub height: f32,
}

pub fn layout_flow(s: &Surface, flow: &Node, fonts: &Fonts) -> FlowLayout {
    let nodes: Vec<usize> = flow.args.nodes.clone();
    let edges: &Vec<Edge> = &flow.args.edges;
    let flagged = |e: &Edge| e.flags.contains("below") || e.flags.contains("above");
    // back edges by DFS over unflagged edges
    let mut back: BTreeSet<usize> = BTreeSet::new();
    let mut state: BTreeMap<usize, u8> = BTreeMap::new();
    let mut out: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, e) in edges.iter().enumerate() {
        if !flagged(e) {
            out.entry(e.from).or_default().push(i);
        }
    }
    fn dfs(id: usize, edges: &[Edge], out: &BTreeMap<usize, Vec<usize>>, state: &mut BTreeMap<usize, u8>, back: &mut BTreeSet<usize>) {
        state.insert(id, 1);
        if let Some(es) = out.get(&id) {
            for &ei in es {
                let to = edges[ei].to;
                match state.get(&to) {
                    Some(1) => {
                        back.insert(ei);
                    }
                    Some(2) => {}
                    _ => dfs(to, edges, out, state, back),
                }
            }
        }
        state.insert(id, 2);
    }
    for &n in &nodes {
        if !state.contains_key(&n) {
            dfs(n, edges, &out, &mut state, &mut back);
        }
    }
    let forward: Vec<usize> = (0..edges.len()).filter(|&i| !flagged(&edges[i]) && !back.contains(&i)).collect();
    // layers by Kahn
    let mut layer: BTreeMap<usize, usize> = nodes.iter().map(|&n| (n, 0)).collect();
    let mut indeg: BTreeMap<usize, usize> = nodes.iter().map(|&n| (n, 0)).collect();
    for &ei in &forward {
        *indeg.entry(edges[ei].to).or_insert(0) += 1;
    }
    let mut queue: Vec<usize> = nodes.iter().copied().filter(|n| indeg[n] == 0).collect();
    let mut qi = 0;
    while qi < queue.len() {
        let id = queue[qi];
        qi += 1;
        for &ei in &forward {
            let e = &edges[ei];
            if e.from == id {
                let l = layer[&id] + 1;
                if l > layer[&e.to] {
                    layer.insert(e.to, l);
                }
                let d = indeg.get_mut(&e.to).unwrap();
                *d -= 1;
                if *d == 0 {
                    queue.push(e.to);
                }
            }
        }
    }
    let mut cols: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &n in &nodes {
        cols.entry(layer[&n]).or_default().push(n);
    }
    let max_rows = cols.values().map(|c| c.len()).max().unwrap_or(1) as f32;
    let total_h = max_rows * NODE_H + (max_rows - 1.0) * GAP_Y;
    let mut placed: BTreeMap<usize, PlacedNode> = BTreeMap::new();
    let mut x = PAD;
    let mut l = 0;
    while let Some(col) = cols.get(&l) {
        let w = col.iter().map(|&n| (fonts.width(s.nodes[n].label.as_deref().unwrap_or(""), 13.0, 400) + 28.0).ceil()).fold(90.0_f32, f32::max);
        let col_h = col.len() as f32 * NODE_H + (col.len() as f32 - 1.0) * GAP_Y;
        let y0 = PAD + (total_h - col_h) / 2.0;
        for (i, &n) in col.iter().enumerate() {
            let nd = &s.nodes[n];
            placed.insert(n, PlacedNode { node: n, id: nd.id.clone(), label: nd.label.clone().unwrap_or_default(), tone: nd.args.tone.clone(), x, y: y0 + i as f32 * (NODE_H + GAP_Y), w, h: NODE_H });
        }
        let labels = forward.iter().filter(|&&ei| layer[&edges[ei].from] == l && edges[ei].label.is_some()).map(|&ei| fonts.width(edges[ei].label.as_deref().unwrap(), 11.0, 400).ceil() + 16.0);
        x += w + labels.fold(GAP_X, f32::max);
        l += 1;
    }
    let width = x - GAP_X + PAD;
    let routed: Vec<(usize, Route)> = (0..edges.len())
        .map(|i| {
            let e = &edges[i];
            let r = if e.flags.contains("above") { Route::Above } else if e.flags.contains("below") || back.contains(&i) { Route::Below } else { Route::Forward };
            (i, r)
        })
        .collect();
    let n_below = routed.iter().filter(|r| r.1 == Route::Below).count() as f32;
    let n_above = routed.iter().filter(|r| r.1 == Route::Above).count() as f32;
    let top = PAD + n_above * LANE;
    for p in placed.values_mut() {
        p.y += n_above * LANE;
    }
    let height = top + total_h + PAD + n_below * LANE;
    let bottom = top + total_h;
    let mut k_below = 0.0;
    let mut k_above = 0.0;
    let mut out_edges = vec![];
    for (i, route) in routed {
        let e = &edges[i];
        let a = &placed[&e.from];
        let b = &placed[&e.to];
        let (points, curve, lx, ly) = match route {
            Route::Forward => {
                let (x1, y1, x2, y2) = (a.x + a.w, a.y + a.h / 2.0, b.x, b.y + b.h / 2.0);
                if (y1 - y2).abs() < 1.0 {
                    (vec![(x1, y1), (x2, y2)], false, (x1 + x2) / 2.0, y1.min(y2) - 6.0)
                } else {
                    let mx = (x1 + x2) / 2.0;
                    (vec![(x1, y1), (mx, y1), (mx, y2), (x2, y2)], true, (x1 + x2) / 2.0, (y1 + y2) / 2.0 - 6.0)
                }
            }
            Route::Below => {
                k_below += 1.0;
                let lane = bottom + LANE * k_below;
                let (x1, x2) = (a.x + a.w / 2.0, b.x + b.w / 2.0);
                (vec![(x1, a.y + a.h), (x1, lane), (x2, lane), (x2, b.y + b.h)], false, (x1 + x2) / 2.0, lane - 6.0)
            }
            Route::Above => {
                let lane = PAD + LANE * k_above;
                k_above += 1.0;
                let (x1, x2) = (a.x + a.w / 2.0, b.x + b.w / 2.0);
                (vec![(x1, a.y), (x1, lane), (x2, lane), (x2, b.y)], false, (x1 + x2) / 2.0, lane - 6.0)
            }
        };
        out_edges.push(RoutedEdge { id: e.id.clone(), route, points, curve, dashed: e.flags.contains("dashed"), label: e.label.clone().map(|l| (l, lx, ly)) });
    }
    let mut nodes_out: Vec<PlacedNode> = placed.into_values().collect();
    nodes_out.sort_by(|a, b| nodes.iter().position(|&n| n == a.node).cmp(&nodes.iter().position(|&n| n == b.node)));
    FlowLayout { nodes: nodes_out, edges: out_edges, width, height }
}
