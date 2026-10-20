//! Patches (`~rev` blocks), the data model, the `since=` diff and residue lines: a port of the second
//! half of packages/core/src/index.ts. `apply` returns a new surface and leaves the input untouched.
use crate::model::*;
use crate::parse::{geometry_words, is_chart, is_container, is_input, is_labelled, is_painted, is_positioned, is_space, is_textlike, is_tone, metric_default_tone, num, paint_words, parse_child, slug, unq, words};
use crate::perceive::{derive_all, line_of, q};

fn jstr(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// Longest leading id in a dotted address -> (node index, rest).
pub fn resolve(s: &Surface, addr: &str) -> Result<(usize, String)> {
    let parts: Vec<&str> = addr.split('.').collect();
    for i in (1..=parts.len()).rev() {
        let cand = parts[..i].join(".");
        if let Some(n) = s.get(&cand) {
            return Ok((n, parts[i..].join(".")));
        }
    }
    Err(MXError::new(format!("no node {}", jstr(addr))))
}

pub struct PatchHead {
    pub surface: Option<String>,
    pub rev: u32,
}

pub fn parse_patch_head(line: &str) -> Result<PatchHead> {
    let Some(rest) = line.strip_prefix('~') else {
        return Err(MXError::new("patch must start with ~rev"));
    };
    let head: Vec<&str> = rest.split_whitespace().collect();
    let rev = |w: &str| -> Result<u32> { Ok(num(w)? as u32) };
    match head.len() {
        2 => Ok(PatchHead { surface: Some(head[0].to_string()), rev: rev(head[1])? }),
        _ => Ok(PatchHead { surface: None, rev: rev(head.first().copied().unwrap_or(""))? }),
    }
}

/// The parent of a node and its position among the siblings.
pub fn find_parent(s: &Surface, target: usize) -> Option<(usize, usize)> {
    fn walk(s: &Surface, n: usize, target: usize) -> Option<(usize, usize)> {
        for (i, &c) in s.nodes[n].children.iter().enumerate() {
            if c == target {
                return Some((n, i));
            }
            if let Some(r) = walk(s, c, target) {
                return Some(r);
            }
        }
        None
    }
    walk(s, s.root(), target)
}

fn base_of(n: &Node) -> String {
    if let Some(e) = &n.explicit {
        return e.clone();
    }
    if n.ty == "h1" {
        return "title".into();
    }
    if is_labelled(&n.ty) && n.label.is_some() {
        return slug(n.label.as_deref()).unwrap_or_else(|| n.ty.clone());
    }
    n.ty.clone()
}

/// Assign an id to one node inserted into an existing tree.
fn assign(s: &mut Surface, n: usize, container: Option<&str>) -> String {
    let mut base = base_of(&s.nodes[n]);
    if base == "title" && s.get("title").is_some() && s.nodes[n].explicit.is_none() {
        base = "h1".into();
    }
    let mut cands = vec![base.clone()];
    if let Some(c) = container {
        if s.get(&base).is_some() {
            cands.insert(0, format!("{}.{}", c, base));
        }
    }
    for c in cands {
        if s.get(&c).is_none() {
            s.nodes[n].id = c.clone();
            s.register(&c, n);
            return c;
        }
    }
    let mut k = 2;
    while s.get(&format!("{}{}", base, k)).is_some() {
        k += 1;
    }
    let id = format!("{}{}", base, k);
    s.nodes[n].id = id.clone();
    s.register(&id, n);
    id
}

/// The id of the titled card, tab or flow that scopes a node, if any.
fn container_of(s: &Surface, target: usize) -> Option<String> {
    fn walk(s: &Surface, n: usize, cont: Option<String>, target: usize) -> Option<Option<String>> {
        for &c in &s.nodes[n].children {
            if c == target {
                return Some(cont);
            }
            if s.nodes[c].ty == "flow" && s.nodes[c].args.nodes.contains(&target) {
                return Some(Some(s.nodes[c].id.clone()));
            }
            let next = if is_container(&s.nodes[c]) { Some(s.nodes[c].id.clone()) } else { cont.clone() };
            if let Some(r) = walk(s, c, next, target) {
                return Some(r);
            }
        }
        None
    }
    walk(s, s.root(), None, target).flatten()
}

/// Apply a patch and return the new surface. Errors name the patch line.
pub fn apply(s0: &Surface, patch: &str, surface: Option<&str>) -> Result<Surface> {
    let mut s = s0.clone();
    let lines: Vec<&str> = patch.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return Ok(s);
    }
    let head = parse_patch_head(lines[0].trim())?;
    if let Some(hs) = &head.surface {
        if let Some(call) = surface {
            if hs != call {
                return Err(MXError::new(format!("patch names surface {}, call names {}", jstr(hs), jstr(call))));
            }
        }
        if !s.id.is_empty() && *hs != s.id {
            return Err(MXError::new(format!("patch names surface {}, this is {}", jstr(hs), jstr(&s.id))));
        }
    }
    if head.rev != s.rev + 1 {
        return Err(MXError::new(format!("stale rev {} (surface at {})", head.rev, s.rev)));
    }
    for (i, raw) in lines.iter().enumerate().skip(1) {
        let l = raw.trim();
        let op = l.chars().next().unwrap();
        let body = &l[op.len_utf8()..];
        let r: Result<()> = (|| {
            match op {
                '*' => {
                    let (addr, rest) = match body.find(' ') {
                        Some(sp) => (&body[..sp], &body[sp + 1..]),
                        None => (body, ""),
                    };
                    set_at(&mut s, addr.trim(), &words(rest))
                }
                '-' => remove_at(&mut s, body.trim()),
                '+' => {
                    let Some(sp) = body.find(' ') else {
                        return Err(MXError::new("insert needs a node line"));
                    };
                    let addr = body[..sp].trim();
                    let rest = &body[sp + 1..];
                    let inside = addr.ends_with('/');
                    let (r, tail) = resolve(&s, addr.trim_end_matches('/'))?;
                    if s.nodes[r].ty == "series" && tail.is_empty() {
                        // `+sales Jul:47` appends a datum
                        for w in words(rest) {
                            let i = w.rfind(':').ok_or_else(|| MXError::new(format!("series expects key:value, got {}", jstr(&w))))?;
                            let v = num(&w[i + 1..])?;
                            s.nodes[r].args.data.push((w[..i].to_string(), v, false));
                        }
                        return Ok(());
                    }
                    if s.nodes[r].ty == "table" && (inside || !tail.is_empty()) {
                        let cells: Vec<String> = rest.split('|').map(|c| c.trim().to_string()).collect();
                        if inside && tail.is_empty() {
                            s.nodes[r].args.rows.push(cells);
                        } else if !tail.is_empty() {
                            let at = row_of(&s.nodes[r], &tail)?;
                            s.nodes[r].args.rows.insert(at + 1, cells);
                        } else {
                            return Err(MXError::new(format!("cannot insert at {}", jstr(addr))));
                        }
                        return Ok(());
                    }
                    if !tail.is_empty() {
                        return Err(MXError::new(format!("cannot insert at {}", jstr(addr))));
                    }
                    let in_space = if inside { is_space(&s.nodes[r]) || is_positioned(&s.nodes[r]) } else { find_parent(&s, r).map(|(p, _)| is_space(&s.nodes[p]) || is_positioned(&s.nodes[p])).unwrap_or(false) };
                    let nd = parse_child(rest, in_space)?;
                    let idx = s.push(nd);
                    let cont = if inside {
                        s.nodes[r].children.push(idx);
                        if is_container(&s.nodes[r]) { Some(s.nodes[r].id.clone()) } else { container_of(&s, r) }
                    } else {
                        let (p, at) = find_parent(&s, r).ok_or_else(|| MXError::new(format!("cannot insert at {}", jstr(addr))))?;
                        s.nodes[p].children.insert(at + 1, idx);
                        container_of(&s, r)
                    };
                    assign(&mut s, idx, cont.as_deref());
                    Ok(())
                }
                _ => Err(MXError::new(format!("unknown op {}", l))),
            }
        })();
        if let Err(e) = r {
            return Err(MXError::new(e.message).at(i + 1));
        }
    }
    s.rev += 1;
    Ok(s)
}

fn row_of(n: &Node, key: &str) -> Result<usize> {
    let bare = key.trim_start_matches('#');
    for k in [key.to_string(), bare.to_string(), format!("#{}", key)] {
        if let Some(i) = n.args.rows.iter().position(|r| r.first().map(String::as_str) == Some(k.as_str())) {
            return Ok(i);
        }
    }
    Err(MXError::new(format!("no row {} in {}", jstr(key), n.id)))
}

fn set_at(s: &mut Surface, addr: &str, ws: &[String]) -> Result<()> {
    if addr.contains('>') {
        for n in 0..s.nodes.len() {
            if let Some(e) = s.nodes[n].args.edges.iter_mut().find(|e| e.id == addr) {
                for w in ws {
                    if w == "solid" {
                        e.flags.remove("dashed");
                    } else {
                        e.flags.insert(w.clone());
                    }
                }
                return Ok(());
            }
        }
        return Err(MXError::new(format!("no edge {}", jstr(addr))));
    }
    let (n, rest) = resolve(s, addr)?;
    let t = s.nodes[n].ty.clone();
    if t == "table" && !rest.is_empty() {
        let (rk, col) = match rest.rfind('.') {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None => ("", rest.as_str()),
        };
        let ci = s.nodes[n].args.cols.iter().position(|c| c.0 == col);
        if let (Some(ci), false) = (ci, rk.is_empty()) {
            let ri = row_of(&s.nodes[n], rk)?;
            let row = &mut s.nodes[n].args.rows[ri];
            while row.len() <= ci {
                row.push(String::new());
            }
            row[ci] = ws.iter().map(|w| unq(w)).collect::<Vec<_>>().join(" ");
            return Ok(());
        }
        return Err(MXError::new(format!("no cell {}", jstr(addr))));
    }
    if (is_chart(&t) || t == "series") && !rest.is_empty() {
        let v = num(ws.first().map(String::as_str).unwrap_or(""))?;
        if let Some(d) = s.nodes[n].args.data.iter_mut().find(|d| d.0 == rest) {
            d.1 = v;
            return Ok(());
        }
        return Err(MXError::new(format!("no datum {} in {}", jstr(&rest), s.nodes[n].id)));
    }
    if !rest.is_empty() {
        return Err(MXError::new(format!("no node {}", jstr(addr))));
    }
    if is_painted(&s.nodes[n]) {
        // geometry, style and label words replace what they name; the rest is an error
        let mut probe = Node::new(&t);
        let left = paint_words(&mut probe, ws)?;
        if let Some(w) = left.first() {
            return Err(MXError::new(format!("unknown word {} on {}", jstr(w), t)));
        }
        let nd = &mut s.nodes[n];
        if probe.args.pos.is_some() {
            nd.args.pos = probe.args.pos;
        }
        if probe.args.pos2.is_some() {
            nd.args.pos2 = probe.args.pos2;
        }
        if probe.args.size.is_some() {
            nd.args.size = probe.args.size;
        }
        if probe.args.radius.is_some() {
            nd.args.radius = probe.args.radius;
        }
        if probe.label.is_some() {
            nd.label = probe.label;
        }
        if probe.args.bind.is_some() {
            nd.args.bind = probe.args.bind;
        }
        for (k, v) in probe.args.style {
            if v == "none" && k != "fill" && k != "stroke" {
                nd.args.style.remove(&k);
            } else {
                nd.args.style.insert(k, v);
            }
        }
        for f in probe.flags {
            nd.flags.insert(f);
        }
        return Ok(());
    }
    // geometry and style words on a node with a default look: `*export @10,10 100x32 fill:primary`
    let ws: Vec<String> = {
        let mut probe = Node::new(&t);
        let left = geometry_words(&mut probe, ws)?;
        let nd = &mut s.nodes[n];
        if probe.args.pos.is_some() {
            nd.args.pos = probe.args.pos;
        }
        if probe.args.size.is_some() {
            nd.args.size = probe.args.size;
        }
        for (k, v) in probe.args.style {
            if v == "none" && k != "fill" && k != "stroke" {
                nd.args.style.remove(&k);
            } else {
                nd.args.style.insert(k, v);
            }
        }
        left
    };
    let ws = &ws[..];
    let state = |w: &str| ["enabled", "disabled", "on", "off"].contains(&w);
    let pos: Vec<&String> = ws.iter().filter(|w| !(is_tone(w) || state(w)) && !(t == "select" && w.starts_with('+'))).collect();
    for w in ws {
        let nd = &mut s.nodes[n];
        if w == "enabled" {
            nd.flags.remove("disabled");
        } else if w == "disabled" {
            nd.flags.insert("disabled".into());
        } else if w == "on" || w == "off" {
            nd.args.value = Value::Bool(w == "on");
        } else if is_tone(w) {
            nd.args.tone = Some(w.clone());
        } else if t == "select" {
            if let Some(o) = w.strip_prefix('+') {
                nd.args.options.push(unq(o));
            }
        }
    }
    if t == "tabs" && !pos.is_empty() {
        let want = unq(pos[0]);
        let kids = s.nodes[n].children.clone();
        if !kids.iter().any(|&c| s.nodes[c].label.as_deref() == Some(want.as_str())) {
            return Err(MXError::new(format!("no tab {}", jstr(&want))));
        }
        for c in kids {
            s.nodes[c].flags.remove("selected");
            if s.nodes[c].label.as_deref() == Some(want.as_str()) {
                s.nodes[c].flags.insert("selected".into());
            }
        }
        return Ok(());
    }
    if t == "metric" {
        if let Some(v) = pos.first() {
            s.nodes[n].args.value = Value::Str(unq(v));
        }
        if pos.len() > 1 {
            s.nodes[n].args.delta = Some(unq(pos[1]));
        }
        return Ok(());
    }
    if ["field", "slider", "select"].contains(&t.as_str()) && !pos.is_empty() {
        let v = unq(pos[0]);
        s.nodes[n].args.value = if t == "slider" { Value::Num(num(&v)?) } else { Value::Str(v) };
        return Ok(());
    }
    if !pos.is_empty() {
        s.nodes[n].label = Some(pos.iter().map(|w| unq(w)).collect::<Vec<_>>().join(" "));
    }
    Ok(())
}

fn remove_at(s: &mut Surface, addr: &str) -> Result<()> {
    let (n, rest) = resolve(s, addr)?;
    if !rest.is_empty() && (s.nodes[n].ty == "series" || is_chart(&s.nodes[n].ty)) {
        let i = s.nodes[n].args.data.iter().position(|d| d.0 == rest).ok_or_else(|| MXError::new(format!("no datum {} in {}", jstr(&rest), s.nodes[n].id)))?;
        s.nodes[n].args.data.remove(i);
        return Ok(());
    }
    if !rest.is_empty() && s.nodes[n].ty == "table" {
        let ri = row_of(&s.nodes[n], &rest)?;
        s.nodes[n].args.rows.remove(ri);
        return Ok(());
    }
    if !rest.is_empty() {
        return Err(MXError::new(format!("no node {}", jstr(addr))));
    }
    if n == s.root() {
        return Err(MXError::new("cannot remove root"));
    }
    let (p, i) = find_parent(s, n).ok_or_else(|| MXError::new(format!("no node {}", jstr(addr))))?;
    s.nodes[p].children.remove(i);
    fn drop(s: &mut Surface, x: usize) {
        let id = s.nodes[x].id.clone();
        s.unregister(&id);
        for c in s.nodes[x].children.clone() {
            drop(s, c);
        }
        for y in s.nodes[x].args.nodes.clone() {
            let yid = s.nodes[y].id.clone();
            s.unregister(&yid);
        }
    }
    drop(s, n);
    Ok(())
}

/// The input values keyed by id, in document order, as the JSON an `action` event carries.
pub fn data_model(s: &Surface) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for (id, n) in &s.ids {
        let nd = &s.nodes[*n];
        if is_input(&nd.ty) {
            m.insert(id.clone(), value_json(&nd.args.value));
        }
    }
    serde_json::Value::Object(m)
}

pub fn value_json(v: &Value) -> serde_json::Value {
    match v {
        Value::None => serde_json::Value::Null,
        Value::Str(s) => serde_json::Value::String(s.clone()),
        Value::Num(n) => serde_json::json!(n),
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::List(l) => serde_json::json!(l),
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// The value part of a `*id ...` line that reproduces a node's current state.
pub fn primary_value(s: &Surface, n: usize) -> String {
    let nd = &s.nodes[n];
    let a = &nd.args;
    let mut extra: Vec<String> = vec![];
    if let Some(t) = &a.tone {
        extra.push(t.clone());
    }
    if nd.flags.contains("disabled") {
        extra.push("disabled".into());
    }
    if is_painted(nd) {
        // every geometry, style and label word: the set form re-applies the whole state
        let mut v: Vec<String> = vec![];
        if let Some(p) = a.pos {
            v.push(format!("@{},{}", fmt_num(p.0 as f64), fmt_num(p.1 as f64)));
        }
        if let Some(p) = a.pos2 {
            v.push(format!("@{},{}", fmt_num(p.0 as f64), fmt_num(p.1 as f64)));
        }
        if let Some(sz) = a.size {
            v.push(format!("{}x{}", fmt_num(sz.0 as f64), fmt_num(sz.1 as f64)));
        }
        if let Some(r) = a.radius {
            v.push(format!("r:{}", fmt_num(r as f64)));
        }
        for (k, val) in &a.style {
            v.push(format!("{}:{}", k, if val.contains(' ') { jstr(val) } else { val.clone() }));
        }
        if let Some(b) = &a.bind {
            v.push(format!("={}", b));
        }
        if let Some(l) = &nd.label {
            v.push(jstr(l));
        }
        v.extend(nd.flags.iter().cloned());
        return v.join(" ");
    }
    let geometry = |mut v: String| -> String {
        if let Some(p) = a.pos {
            v.push_str(&format!(" @{},{}", fmt_num(p.0 as f64), fmt_num(p.1 as f64)));
        }
        if let Some(sz) = a.size {
            v.push_str(&format!(" {}x{}", fmt_num(sz.0 as f64), fmt_num(sz.1 as f64)));
        }
        for (k, val) in &a.style {
            if k != "anim" {
                v.push_str(&format!(" {}:{}", k, if val.contains(' ') { jstr(val) } else { val.clone() }));
            }
        }
        v
    };
    return geometry(primary_plain(s, n));
}

fn primary_plain(s: &Surface, n: usize) -> String {
    let nd = &s.nodes[n];
    let a = &nd.args;
    let mut extra: Vec<String> = vec![];
    if let Some(t) = &a.tone {
        extra.push(t.clone());
    }
    if nd.flags.contains("disabled") {
        extra.push("disabled".into());
    }
    let with = |first: String, extra: &[String]| -> String {
        let mut v = vec![first];
        v.extend(extra.iter().cloned());
        v.join(" ")
    };
    match nd.ty.as_str() {
        "metric" => {
            let mut v = vec![q(&match &a.value {
                Value::Str(s) => s.clone(),
                Value::None => "null".into(),
                other => fmt_num(match other {
                    Value::Num(x) => *x,
                    _ => 0.0,
                }),
            })];
            if let Some(d) = &a.delta {
                v.push(q(d));
            }
            if let Some(t) = &a.tone {
                if Some(t.as_str()) != metric_default_tone(a.delta.as_deref()) {
                    v.push(t.clone());
                }
            }
            v.join(" ")
        }
        "field" => with(format!("={}", jstr(match &a.value { Value::Str(s) => s, _ => "" })), &extra),
        "slider" => with(match &a.value { Value::Num(x) => fmt_num(*x), _ => "0".into() }, &extra),
        "select" => with(q(match &a.value { Value::List(l) => l.first().map(String::as_str).unwrap_or(""), Value::Str(s) => s, _ => "" }), &extra),
        "toggle" | "check" => with(if a.value == Value::Bool(true) { "on".into() } else { "off".into() }, &extra),
        "tabs" => q(nd.children.iter().find(|&&c| s.nodes[c].flags.contains("selected")).and_then(|&c| s.nodes[c].label.as_deref()).unwrap_or("")),
        t if is_chart(t) => a.data.iter().map(|(k, v, h)| format!("{}{}:{}", if *h { "*" } else { "" }, k, fmt_num(*v))).collect::<Vec<_>>().join(" "),
        _ => with(q(nd.label.as_deref().unwrap_or("")), &extra),
    }
}

/// Patch lines for a diagram's nodes and edges: relabelled or retoned nodes, edges made dashed or solid.
/// False when the diagram changed shape (a node or edge added, removed or relabelled on an edge).
fn flow_ops(a: &Surface, b: &Surface, na: usize, nb: usize, ops: &mut Vec<String>) -> bool {
    let (fa, fb) = (&a.nodes[na].args, &b.nodes[nb].args);
    if fa.nodes.len() != fb.nodes.len() || fa.edges.len() != fb.edges.len() {
        return false;
    }
    for &x in &fb.nodes {
        let xb = &b.nodes[x];
        let Some(xa) = a.get(&xb.id).map(|i| &a.nodes[i]) else { return false };
        if xa.ty != "node" {
            return false;
        }
        let mut ws: Vec<String> = vec![];
        if xa.label != xb.label {
            ws.push(q(xb.label.as_deref().unwrap_or("")));
        }
        if xa.args.tone != xb.args.tone {
            match &xb.args.tone {
                Some(t) => ws.push(t.clone()),
                None => return false,
            }
        }
        if !ws.is_empty() {
            ops.push(format!("*{} {}", xb.id, ws.join(" ")));
        }
    }
    for (ea, eb) in fa.edges.iter().zip(&fb.edges) {
        if ea.id != eb.id || ea.label != eb.label {
            return false;
        }
        if ea.flags != eb.flags {
            let mut ws: Vec<String> = eb.flags.difference(&ea.flags).cloned().collect();
            for gone in ea.flags.difference(&eb.flags) {
                if gone == "dashed" {
                    ws.push("solid".into());
                } else {
                    return false;
                }
            }
            ops.push(format!("*{} {}", eb.id, ws.join(" ")));
        }
    }
    true
}

/// The patch that turns `a` into `b`, for `observe since=<rev>`. None when a change has no patch form
/// (a node changed type, or a diagram changed shape); the caller then sends the full tree.
pub fn diff(a: &Surface, b: &Surface) -> Option<String> {
    let mut ops: Vec<String> = vec![];
    let derived = derive_all(b);
    let dline = |n: usize| line_of(&b.nodes[n], None, derived.get(&n).map(String::as_str));
    fn sub(b: &Surface, n: usize, ops: &mut Vec<String>, dline: &dyn Fn(usize) -> String) -> bool {
        for &c in &b.nodes[n].children {
            ops.push(format!("+{}/ {}", b.nodes[n].id, dline(c)));
            if b.nodes[c].ty == "flow" {
                return false;
            }
            if !sub(b, c, ops, dline) {
                return false;
            }
        }
        true
    }
    fn walk(a: &Surface, b: &Surface, p: usize, ops: &mut Vec<String>, dline: &dyn Fn(usize) -> String) -> bool {
        let kids = &b.nodes[p].children;
        for (i, &nb) in kids.iter().enumerate() {
            let nbn = &b.nodes[nb];
            let Some(na) = a.get(&nbn.id) else {
                if nbn.ty == "flow" {
                    return false;
                }
                let line = dline(nb);
                ops.push(if i == 0 { format!("+{}/ {}", b.nodes[p].id, line) } else { format!("+{} {}", b.nodes[kids[i - 1]].id, line) });
                if nbn.ty == "table" {
                    for r in &nbn.args.rows {
                        ops.push(format!("+{}/ {}", nbn.id, r.join(" | ")));
                    }
                }
                if !sub(b, nb, ops, dline) {
                    return false;
                }
                continue;
            };
            let nan = &a.nodes[na];
            if nan.ty != nbn.ty {
                return false;
            }
            let (mut la, mut lb) = (line_of(nan, None, None), line_of(nbn, None, None));
            if nbn.ty == "tab" {
                // selection is patched on the tabs node, not the tab
                la = la.replacen("tab *", "tab ", 1);
                lb = lb.replacen("tab *", "tab ", 1);
            }
            if nbn.ty == "tabs" {
                let sel = |s: &Surface, n: &Node| n.children.iter().find(|&&c| s.nodes[c].flags.contains("selected")).and_then(|&c| s.nodes[c].label.clone());
                if sel(a, nan) != sel(b, nbn) {
                    ops.push(format!("*{} {}", nbn.id, q(&sel(b, nbn).unwrap_or_default())));
                }
            }
            if nbn.ty == "flow" {
                if la != lb || !flow_ops(a, b, na, nb, ops) {
                    return false;
                }
            }
            if la != lb && nbn.ty == "series" {
                // a buffer changes by `*s.key v`, `-s.key` and `+s key:v` (appended); anything else is a reshape
                let (da, db) = (&nan.args.data, &nbn.args.data);
                for d in da {
                    if !db.iter().any(|x| x.0 == d.0) {
                        ops.push(format!("-{}.{}", nbn.id, d.0));
                    }
                }
                let kept: Vec<&(String, f64, bool)> = db.iter().filter(|x| da.iter().any(|d| d.0 == x.0)).collect();
                let kept_a: Vec<&(String, f64, bool)> = da.iter().filter(|d| db.iter().any(|x| x.0 == d.0)).collect();
                if kept.iter().map(|x| &x.0).ne(kept_a.iter().map(|x| &x.0)) {
                    return false;
                }
                for x in &kept {
                    let d = da.iter().find(|d| d.0 == x.0).unwrap();
                    if d.1 != x.1 {
                        ops.push(format!("*{}.{} {}", nbn.id, x.0, fmt_num(x.1)));
                    }
                }
                let new: Vec<&(String, f64, bool)> = db.iter().filter(|x| !da.iter().any(|d| d.0 == x.0)).collect();
                if !new.is_empty() {
                    // appended means they are the tail of the new buffer
                    if db[db.len() - new.len()..].iter().any(|x| da.iter().any(|d| d.0 == x.0)) {
                        return false;
                    }
                    ops.push(format!("+{} {}", nbn.id, new.iter().map(|x| format!("{}:{}", x.0, fmt_num(x.1))).collect::<Vec<_>>().join(" ")));
                }
            } else if la != lb && is_chart(&nbn.ty) {
                // a datum changes by `*chart.key value`; a new, removed or re-highlighted datum has no patch form
                let (da, db) = (&nan.args.data, &nbn.args.data);
                if da.len() != db.len() || da.iter().zip(db).any(|(x, y)| x.0 != y.0 || x.2 != y.2) {
                    return false;
                }
                for (x, y) in da.iter().zip(db) {
                    if x.1 != y.1 {
                        ops.push(format!("*{}.{} {}", nbn.id, y.0, fmt_num(y.1)));
                    }
                }
            } else if la != lb {
                let mut extra = String::new();
                if nan.flags.contains("disabled") && !nbn.flags.contains("disabled") {
                    extra.push_str(" enabled");
                }
                if nbn.ty == "select" {
                    if nan.args.options.iter().any(|o| !nbn.args.options.contains(o)) {
                        return false; // a removed option has no patch form
                    }
                    for o in nbn.args.options.iter().filter(|o| !nan.args.options.contains(o)) {
                        extra.push_str(&format!(" +{}", q(o)));
                    }
                }
                ops.push(format!("*{} {}{}", nbn.id, primary_value(b, nb), extra));
            }
            if nbn.ty == "table" {
                let (ra, rb) = (&nan.args.rows, &nbn.args.rows);
                let key = |r: &Vec<String>| r.first().cloned().unwrap_or_default();
                for r in ra {
                    if !rb.iter().any(|x| key(x) == key(r)) {
                        ops.push(format!("-{}.{}", nbn.id, key(r)));
                    }
                }
                for (j, r) in rb.iter().enumerate() {
                    match ra.iter().find(|x| key(x) == key(r)) {
                        None => ops.push(if j == 0 { format!("+{}/ {}", nbn.id, r.join(" | ")) } else { format!("+{}.{} {}", nbn.id, key(&rb[j - 1]), r.join(" | ")) }),
                        Some(o) => {
                            for (k, cell) in r.iter().enumerate() {
                                if o.get(k) != Some(cell) {
                                    let col = nbn.args.cols.get(k).map(|c| c.0.clone()).unwrap_or_default();
                                    ops.push(format!("*{}.{}.{} {}", nbn.id, key(r), col, q(cell)));
                                }
                            }
                        }
                    }
                }
            }
            if !walk(a, b, nb, ops, dline) {
                return false;
            }
        }
        true
    }
    if !walk(a, b, b.root(), &mut ops, &dline) {
        return None;
    }
    for (id, n) in &a.ids {
        let n = *n;
        if b.get(id).is_some() {
            continue;
        }
        let still_has_parent = match find_parent(a, n) {
            Some((p, _)) => p == a.root() || (b.get(&a.nodes[p].id).is_some() && !ops.contains(&format!("-{}", a.nodes[p].id))),
            None => false,
        };
        if a.nodes[n].ty != "node" && still_has_parent {
            ops.push(format!("-{}", id));
        }
    }
    let mut out = vec![format!("~{}", b.rev)];
    out.extend(ops);
    Some(out.join("\n"))
}

/// The one-line residue of a dissolved surface.
pub fn residue_line(s: &Surface) -> String {
    let title = s
        .get("title")
        .and_then(|n| s.nodes[n].label.clone())
        .or_else(|| s.ids.iter().map(|(_, n)| &s.nodes[*n]).find(|n| is_textlike(&n.ty) && n.label.is_some()).and_then(|n| n.label.clone()))
        .unwrap_or_else(|| s.id.clone());
    format!("@{} dissolved: {}", s.id, title)
}

/// How an input event prints a value: `="text"`, `on`/`off`, `*choice`, or a number.
pub fn fmt_value(ty: &str, v: &Value) -> String {
    match ty {
        "field" => format!("={}", jstr(match v { Value::Str(s) => s, _ => "" })),
        "check" | "toggle" => if *v == Value::Bool(true) { "on".into() } else { "off".into() },
        "select" => {
            let l: Vec<String> = match v {
                Value::List(l) => l.clone(),
                Value::Str(s) => vec![s.clone()],
                _ => vec![],
            };
            l.iter().map(|x| format!("*{}", if x.contains(' ') { jstr(x) } else { x.clone() })).collect::<Vec<_>>().join(" ")
        }
        _ => match v {
            Value::Num(x) => fmt_num(*x),
            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::List(l) => l.join(","),
            Value::None => "null".into(),
        },
    }
}
