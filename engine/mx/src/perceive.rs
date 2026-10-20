use crate::model::*;
use crate::parse::{is_chart, is_container, is_labelled, is_painted, is_space, is_textlike, is_tone, metric_default_tone, slug};
use std::collections::{BTreeMap, BTreeSet};

/// The ids the rules give the whole tree, ignoring stored ids. Keyed by node index.
pub fn derive_all(s: &Surface) -> BTreeMap<usize, String> {
    let mut order: Vec<(usize, Option<usize>)> = vec![];
    fn walk(s: &Surface, n: usize, container: Option<usize>, order: &mut Vec<(usize, Option<usize>)>) {
        for &c in &s.nodes[n].children {
            order.push((c, container));
            let cont = if is_container(&s.nodes[c]) { Some(c) } else { container };
            if s.nodes[c].ty == "flow" {
                for &x in &s.nodes[c].args.nodes {
                    order.push((x, Some(c)));
                }
            }
            walk(s, c, cont, order);
        }
    }
    walk(s, s.root(), None, &mut order);
    let mut bases: Vec<(usize, String, Option<usize>)> = vec![];
    let mut seen_title = false;
    for &(n, cont) in &order {
        let nd = &s.nodes[n];
        let b = if let Some(e) = &nd.explicit {
            e.clone()
        } else if nd.ty == "h1" && !seen_title {
            seen_title = true;
            "title".to_string()
        } else if is_labelled(&nd.ty) && nd.label.is_some() {
            slug(nd.label.as_deref()).unwrap_or_else(|| nd.ty.clone())
        } else {
            nd.ty.clone()
        };
        bases.push((n, b, cont));
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, b, _) in &bases {
        *counts.entry(b.as_str()).or_insert(0) += 1;
    }
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let mut out: BTreeMap<usize, String> = BTreeMap::new();
    for (n, b, cont) in &bases {
        let cand: Vec<String> = if s.nodes[*n].explicit.is_some() {
            vec![b.clone()]
        } else if counts[b.as_str()] > 1 && cont.is_some() {
            vec![format!("{}.{}", out[&cont.unwrap()], b), b.clone()]
        } else {
            vec![b.clone()]
        };
        let mut nid = cand.into_iter().find(|c| !taken.contains(c));
        if nid.is_none() {
            let mut k = 2;
            while taken.contains(&format!("{}{}", b, k)) {
                k += 1;
            }
            nid = Some(format!("{}{}", b, k));
        }
        let nid = nid.unwrap();
        taken.insert(nid.clone());
        out.insert(*n, nid);
    }
    out
}

/// Quote a value the way perception prints it: JSON string when it has a space, a quote, or is empty.
pub fn q(v: &str) -> String {
    if v.contains(' ') || v.is_empty() || v.contains('"') {
        serde_json::to_string(v).unwrap()
    } else {
        v.to_string()
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

fn value_str(v: &Value) -> String {
    match v {
        Value::None => "null".into(),
        Value::Str(s) => s.clone(),
        Value::Num(n) => fmt_num(*n),
        Value::Bool(b) => b.to_string(),
        Value::List(l) => l.join(","),
    }
}

/// One node as a perception line: `type args flags [#id] [@box]`.
pub fn line_of(n: &Node, bx: Option<&Box>, derived_id: Option<&str>) -> String {
    let t = n.ty.as_str();
    let a = &n.args;
    let mut parts: Vec<String> = vec![t.to_string()];
    let mut tone: Option<String> = a.tone.clone();
    if is_painted(n) {
        if t == "image" {
            if let Some(h) = &a.href {
                parts.push(h.clone());
            }
        }
        if let Some(p) = a.pos {
            parts.push(format!("@{},{}", fmt_num(p.0 as f64), fmt_num(p.1 as f64)));
        }
        if let Some(p) = a.pos2 {
            parts.push(format!("@{},{}", fmt_num(p.0 as f64), fmt_num(p.1 as f64)));
        }
        if let Some(s) = a.size {
            parts.push(format!("{}x{}", fmt_num(s.0 as f64), fmt_num(s.1 as f64)));
        }
        if let Some(r) = a.radius {
            parts.push(format!("r:{}", fmt_num(r as f64)));
        }
        if let Some(pd) = &a.path {
            parts.push(serde_json::to_string(pd).unwrap());
        }
        for (k, v) in &a.style {
            parts.push(format!("{}:{}", k, if v.contains(' ') { serde_json::to_string(v).unwrap() } else { v.clone() }));
        }
        if let Some(b) = &a.bind {
            parts.push(format!("={}", b));
        }
        if let Some(l) = &n.label {
            parts.push(serde_json::to_string(l).unwrap());
        }
        for f in &n.flags {
            parts.push(f.clone());
        }
        return finish(parts, n, bx, derived_id);
    }
    if t == "metric" {
        match &a.bind_label {
            Some(b) => parts.push(format!("={}", b)),
            None => parts.push(q(n.label.as_deref().unwrap_or(""))),
        }
        match &a.bind {
            Some(b) => parts.push(format!("={}", b)),
            None => parts.push(q(&value_str(&a.value))),
        }
        if let Some(d) = &a.delta {
            parts.push(q(d));
        }
        if let Some(tn) = &tone {
            if Some(tn.as_str()) != metric_default_tone(a.delta.as_deref()) {
                parts.push(tn.clone());
            }
        }
        tone = None;
    } else if is_chart(t) {
        if let Some(l) = &n.label {
            parts.push(q(l));
        }
        if let Some(b) = &a.bind {
            parts.push(format!("={}", b));
        }
        for (k, v, h) in &a.data {
            parts.push(format!("{}{}:{}", if *h { "*" } else { "" }, k, fmt_num(*v)));
        }
    } else if t == "flow" {
        if let Some(l) = &n.label {
            parts.push(q(l));
        }
        parts.push(format!("[{} nodes]", a.nodes.len()));
    } else if t == "each" {
        if let Some(b) = &a.bind {
            parts.push(format!("={}", b));
        }
        if let Some(c) = a.grid_cols {
            parts.push(c.to_string());
        }
    } else if t == "series" {
        parts.push(n.label.clone().unwrap_or_default());
        let vals: Vec<f64> = a.data.iter().map(|d| d.1).collect();
        parts.push(format!("[{} points]", vals.len()));
        if let (Some(min), Some(max), Some(last)) = (vals.iter().cloned().reduce(f64::min), vals.iter().cloned().reduce(f64::max), vals.last()) {
            parts.push(format!("min:{} max:{} last:{}", fmt_num(min), fmt_num(max), fmt_num(*last)));
        }
        if let Some(an) = a.style.get("anim") {
            parts.push(format!("anim:{}", an));
        }
        return finish(parts, n, bx, derived_id);
    } else if t == "table" {
        if let Some(l) = &n.label {
            parts.push(q(l));
        }
        for (c, k) in &a.cols {
            parts.push(match k {
                Some(k) => format!("{}:{}", c, k),
                None => c.clone(),
            });
        }
    } else if t == "select" {
        let sel: Vec<String> = match &a.value {
            Value::List(l) => l.clone(),
            Value::Str(s) => vec![s.clone()],
            _ => vec![],
        };
        parts.push(q(n.label.as_deref().unwrap_or("")));
        for o in &a.options {
            parts.push(format!("{}{}", if sel.contains(o) { "*" } else { "" }, q(o)));
        }
    } else if t == "toggle" || t == "check" {
        parts.push(q(n.label.as_deref().unwrap_or("")));
        parts.push(if a.value == Value::Bool(true) { "on".into() } else { "off".into() });
    } else if t == "slider" {
        parts.push(q(n.label.as_deref().unwrap_or("")));
        parts.push(value_str(&a.value));
        parts.push(format!("{}..{}", fmt_num(a.min.unwrap_or(0.0)), fmt_num(a.max.unwrap_or(100.0))));
    } else if t == "field" {
        parts.push(q(n.label.as_deref().unwrap_or("")));
        if a.kind.as_deref() != Some("text") {
            parts.push(a.kind.clone().unwrap_or_default());
        }
        if let Value::Str(v) = &a.value {
            if !v.is_empty() {
                parts.push(format!("={}", serde_json::to_string(v).unwrap()));
            }
        }
    } else if t == "tab" {
        parts.push(format!("{}{}", if n.flags.contains("selected") { "*" } else { "" }, q(n.label.as_deref().unwrap_or(""))));
    } else if t == "grid" {
        if let Some(c) = a.grid_cols {
            parts.push(c.to_string());
        }
    } else if is_textlike(t) && a.bind.is_some() {
        parts.push(format!("={}", a.bind.as_deref().unwrap()));
    } else if n.label.is_some() && is_textlike(t) {
        let lab = n.label.as_deref().unwrap();
        let last = lab.split(' ').last().unwrap_or("");
        parts.push(if is_tone(last) || last.starts_with('#') || lab.is_empty() { q(lab) } else { lab.to_string() });
    } else if let Some(l) = &n.label {
        parts.push(q(l));
    }
    if t == "link" {
        if let Some(h) = &a.href {
            parts.push(h.clone());
        }
    }
    if t == "image" {
        if let Some(alt) = &a.alt {
            parts.push(q(alt));
        }
    }
    if let Some(p) = a.pos {
        parts.push(format!("@{},{}", fmt_num(p.0 as f64), fmt_num(p.1 as f64)));
    }
    if let Some(sz) = a.size {
        parts.push(format!("{}x{}", fmt_num(sz.0 as f64), fmt_num(sz.1 as f64)));
    }
    for (k, v) in &a.style {
        parts.push(format!("{}:{}", k, if v.contains(' ') { serde_json::to_string(v).unwrap() } else { v.clone() }));
    }
    if let Some(tn) = tone {
        parts.push(tn);
    }
    for f in n.flags.iter().filter(|f| f.as_str() != "selected") {
        parts.push(f.clone());
    }
    finish(parts, n, bx, derived_id)
}

fn finish(mut parts: Vec<String>, n: &Node, bx: Option<&Box>, derived_id: Option<&str>) -> String {
    if let Some(d) = derived_id {
        if (n.explicit.is_some() && n.ty != "series") || n.id != d {
            parts.push(format!("#{}", n.id));
        }
    }
    if let Some(b) = bx {
        parts.push(format!("@{},{},{},{}", b.x, b.y, b.w, b.h));
    }
    parts.join(" ")
}

/// How a sized node reads at level 1 and 2: `[N shapes, M labelled]`.
pub fn space_summary(s: &Surface, n: usize) -> String {
    fn count(s: &Surface, n: usize, shapes: &mut usize, labelled: &mut usize) {
        for &c in &s.nodes[n].children {
            let nd = &s.nodes[c];
            if is_painted(nd) {
                *shapes += 1;
            }
            if nd.label.is_some() && nd.ty != "decoration" {
                *labelled += 1;
            }
            count(s, c, shapes, labelled);
        }
    }
    let (mut shapes, mut labelled) = (0, 0);
    count(s, n, &mut shapes, &mut labelled);
    format!("[{} shapes, {} labelled]", shapes, labelled)
}

pub fn hidden_summary(s: &Surface, n: usize, limit: usize) -> String {
    let labs: Vec<String> = s.nodes[n].children.iter().map(|&c| s.nodes[c].label.clone().unwrap_or_else(|| s.nodes[c].ty.clone())).collect();
    if labs.is_empty() {
        return "[0 hidden]".into();
    }
    let more = if labs.len() > limit { format!(", +{}", labs.len() - limit) } else { String::new() };
    format!("[hidden: {}{}]", labs[..labs.len().min(limit)].join(", "), more)
}

pub fn format_diag(d: &Diag) -> String {
    match d {
        Diag::Overflow { addr, shown, needed } => format!("!overflow {} {}/{}px", addr, shown, needed),
        Diag::Overlap { addr, other } => format!("!overlap {} {}", addr, other),
        Diag::Contrast { addr, ratio } => format!("!contrast {} {:.1}", addr, ratio),
        Diag::Tiny { addr, w, h } => format!("!tiny {} {}x{}px", addr, w, h),
        Diag::OffCanvas { addr } => format!("!offcanvas {}", addr),
    }
}

#[derive(Default)]
pub struct PerceiveOptions<'a> {
    pub boxes: Option<&'a BTreeMap<String, Box>>,
    pub diags: &'a [Diag],
    pub rev: Option<u32>,
    pub focus: Option<&'a str>,
}

fn contains(s: &Surface, n: usize, target: usize) -> bool {
    s.nodes[n].children.iter().any(|&c| c == target || contains(s, c, target))
}

/// Perception at level 0, 1 or 2. Same output, byte for byte, as the TypeScript `perceive`.
pub fn perceive(s: &Surface, level: u8, opts: PerceiveOptions) -> Result<String> {
    let empty = BTreeMap::new();
    let boxes = opts.boxes.unwrap_or(&empty);
    let derived = derive_all(s);
    let mut head = format!("@{}{} rev={}", s.id, if s.life.is_empty() { String::new() } else { format!(" {}", s.life.join(" ")) }, opts.rev.unwrap_or(s.rev));
    if level == 0 {
        head.push_str(&format!(" nodes={}", s.ids.len() - 1));
        if !opts.diags.is_empty() {
            head.push_str(&format!(" diags={}", opts.diags.len()));
        }
        return Ok(head);
    }
    let bx = |n: usize| -> Option<Box> { if level < 2 { None } else { boxes.get(&s.nodes[n].id).copied() } };
    let focus = match opts.focus {
        Some(f) => Some(s.get(f).ok_or_else(|| MXError::new(format!("no node {}", serde_json::to_string(f).unwrap())))?),
        None => None,
    };
    let on_path = |n: usize| -> bool { focus.map(|f| n == f || contains(s, n, f)).unwrap_or(true) };
    let mut out: Vec<String> = vec![];
    fn walk(s: &Surface, n: usize, d: usize, in_focus: bool, level: u8, focus: Option<usize>, derived: &BTreeMap<usize, String>, bx: &dyn Fn(usize) -> Option<Box>, on_path: &dyn Fn(usize) -> bool, out: &mut Vec<String>) {
        for &c in &s.nodes[n].children {
            let keep = in_focus || on_path(c);
            if !keep {
                continue;
            }
            let now_in = in_focus || Some(c) == focus;
            let nd = &s.nodes[c];
            let indent = "  ".repeat(d);
            if nd.ty == "tab" && !nd.flags.contains("selected") {
                out.push(format!("{}{} {}", indent, line_of(nd, bx(c).as_ref(), derived.get(&c).map(String::as_str)), hidden_summary(s, c, 3)));
                continue;
            }
            if nd.ty == "each" {
                let count = nd.args.bind.as_deref().and_then(|b| s.get(b)).map(|b| s.nodes[b].args.data.len()).unwrap_or(0);
                out.push(format!("{}{} [{} instances]", indent, line_of(nd, bx(c).as_ref(), derived.get(&c).map(String::as_str)), count));
                walk(s, c, d + 1, now_in, level, focus, derived, bx, on_path, out);
                continue;
            }
            if is_space(nd) && level < 3 {
                out.push(format!("{}{} {}", indent, line_of(nd, bx(c).as_ref(), derived.get(&c).map(String::as_str)), crate::perceive::space_summary(s, c)));
                walk_space(s, c, d + 1, level, derived, bx, out);
                continue;
            }
            out.push(format!("{}{}", indent, line_of(nd, bx(c).as_ref(), derived.get(&c).map(String::as_str))));
            if nd.ty == "table" {
                for r in &nd.args.rows {
                    out.push(format!("{}{}", "  ".repeat(d + 1), r.join(" | ")));
                }
            } else if nd.ty == "flow" {
                let mut seen: BTreeSet<usize> = BTreeSet::new();
                let mut nm = |x: usize| -> String {
                    let xn = &s.nodes[x];
                    let r = format!("{}{}", q(xn.label.as_deref().unwrap_or("")), match &xn.args.tone {
                        Some(t) if !seen.contains(&x) => format!(":{}", t),
                        _ => String::new(),
                    });
                    seen.insert(x);
                    r
                };
                let mut chain: Vec<(String, String)> = vec![];
                let mut cur: Option<usize> = None;
                let mut curfl: Option<String> = None;
                for e in &nd.args.edges {
                    let lab = format!(">{}", e.label.as_deref().map(q).unwrap_or_default());
                    let fl = e.flags.iter().cloned().collect::<Vec<_>>().join(" ");
                    if cur == Some(e.from) && curfl.as_deref() == Some(fl.as_str()) {
                        let to = nm(e.to);
                        chain.last_mut().unwrap().0.push_str(&format!(" {} {}", lab, to));
                    } else {
                        let from = nm(e.from);
                        let to = nm(e.to);
                        chain.push((format!("{} {} {}", from, lab, to), fl.clone()));
                    }
                    cur = Some(e.to);
                    curfl = Some(fl);
                }
                for (txt, fl) in chain {
                    out.push(format!("{}{}{}", "  ".repeat(d + 1), txt, if fl.is_empty() { String::new() } else { format!(" {}", fl) }));
                }
                if level >= 2 {
                    for &x in &nd.args.nodes {
                        if let Some(b) = bx(x) {
                            out.push(format!("{}{} @{},{},{},{}", "  ".repeat(d + 1), s.nodes[x].id, b.x, b.y, b.w, b.h));
                        }
                    }
                }
            }
            walk(s, c, d + 1, now_in, level, focus, derived, bx, on_path, out);
        }
    }
    /// Inside a sized node below level 3: labelled nodes only (no unlabelled shapes, no decorations), still nested.
    fn walk_space(s: &Surface, n: usize, d: usize, level: u8, derived: &BTreeMap<usize, String>, bx: &dyn Fn(usize) -> Option<Box>, out: &mut Vec<String>) {
        for &c in &s.nodes[n].children {
            let nd = &s.nodes[c];
            let decoration = nd.ty == "decoration";
            let show = nd.label.is_some() && !decoration;
            if show {
                out.push(format!("{}{}", "  ".repeat(d), line_of(nd, bx(c).as_ref(), derived.get(&c).map(String::as_str))));
            }
            if !decoration {
                walk_space(s, c, if show { d + 1 } else { d }, level, derived, bx, out);
            }
        }
    }
    if level >= 2 {
        if let Some(b) = boxes.get("root") {
            head.push_str(&format!(" @0,0,{},{}", b.w, b.h));
        }
    }
    out.push(head);
    walk(s, s.root(), 0, focus.is_none(), level, focus, &derived, &bx, &on_path, &mut out);
    for dg in opts.diags {
        let keep = match focus {
            Some(f) => dg.addr().starts_with(&s.nodes[f].id),
            None => true,
        };
        if keep {
            out.push(format_diag(dg));
        }
    }
    Ok(out.join("\n"))
}
