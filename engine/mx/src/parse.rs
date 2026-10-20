use crate::model::*;
use std::collections::BTreeSet;

pub const LABELLED: &[&str] = &["text", "metric", "button", "field", "select", "check", "toggle", "slider", "tab", "link", "image", "badge", "node", "card", "region", "item", "figure", "decoration", "rect", "circle", "line", "path"];
/// Paint primitives: shapes with a position inside a sized node.
pub const PRIMITIVES: &[&str] = &["rect", "circle", "line", "path", "text", "image"];
/// Generic roles: nodes that mean something without a default look of their own.
pub const GENERIC: &[&str] = &["region", "item", "figure", "decoration"];
pub const INTERACTIVE_ROLES: &[&str] = &["button", "link", "field", "select", "check", "toggle", "slider", "tab"];
const STYLE_WORDS: &[&str] = &["fill", "stroke", "width", "radius", "opacity", "size", "weight", "anchor", "rotate", "scale", "font", "anim"];
pub const CHARTS: &[&str] = &["bar", "line", "area", "pie"];
pub const TONES: &[&str] = &["primary", "success", "danger", "warning", "info", "muted"];
pub const TEXTLIKE: &[&str] = &["h1", "h2", "h3", "text", "badge"];
const FIELD_KINDS: &[&str] = &["text", "email", "date", "number", "search", "password"];
const CONTAINERS: &[&str] = &["card", "tab", "flow", "region", "item", "figure"];
pub const INPUTS: &[&str] = &["field", "select", "toggle", "check", "slider"];
const BADGE_WORDS: &[(&str, &str)] = &[
    ("success", "paid done active ok operational passed up open live healthy"),
    ("warning", "pending degraded warning slow partial review draft"),
    ("danger", "failed down error refunded blocked overdue critical cancelled"),
    ("info", "new info scheduled queued"),
];

pub fn is_tone(w: &str) -> bool {
    TONES.contains(&w)
}
pub fn is_chart(t: &str) -> bool {
    CHARTS.contains(&t)
}
pub fn is_labelled(t: &str) -> bool {
    LABELLED.contains(&t)
}
pub fn is_textlike(t: &str) -> bool {
    TEXTLIKE.contains(&t)
}
pub fn is_input(t: &str) -> bool {
    INPUTS.contains(&t)
}
/// A node whose label scopes its children's ids: titled cards, tabs, flows, labelled generic roles, and any
/// labelled node that shows paint children (`button "Export" ...` over a rect and a text).
pub fn is_container(n: &Node) -> bool {
    (CONTAINERS.contains(&n.ty.as_str()) && (n.label.is_some() || n.ty == "flow")) || (n.label.is_some() && !n.children.is_empty() && !is_painted(n))
}
pub fn is_primitive(t: &str) -> bool {
    PRIMITIVES.contains(&t)
}
pub fn is_generic(t: &str) -> bool {
    GENERIC.contains(&t)
}
/// A paint primitive (`text`, `image` and `line` are also node types with a default look; a position tells them apart).
pub fn is_painted(n: &Node) -> bool {
    is_primitive(&n.ty) && (n.args.pos.is_some() || !["text", "image", "line"].contains(&n.ty.as_str()))
}
/// A node with a position: it sits in its parent's coordinate space.
pub fn is_positioned(n: &Node) -> bool {
    n.args.pos.is_some() && !is_painted(n)
}
/// A coordinate space: a node with a declared size. Its children are positioned.
pub fn is_space(n: &Node) -> bool {
    n.args.size.is_some() && !is_painted(n)
}
/// Carries geometry or style of its own (so perception prints them and patches take them).
pub fn has_geometry(n: &Node) -> bool {
    n.args.pos.is_some() || n.args.size.is_some() || n.args.style.keys().any(|k| k != "anim")
}
pub fn is_interactive_role(w: &str) -> bool {
    INTERACTIVE_ROLES.contains(&w)
}

/// Geometry and style words on a node with a default look: `@x,y`, `WxH`, `fill:` and the other style words.
/// Returns the words it did not take. Taken only when the line carries a position, or a size on a
/// non-text node, so v1 lines keep their meaning.
pub fn geometry_words(nd: &mut Node, ws: &[String]) -> Result<Vec<String>> {
    let has_pos = ws.iter().any(|w| parse_pos(w).is_some());
    let has_size = ws.iter().any(|w| parse_size(w).is_some());
    let textlike = is_textlike(&nd.ty) || nd.ty == "series";
    if !(has_pos || (has_size && !textlike) || is_generic(&nd.ty)) {
        return Ok(ws.to_vec());
    }
    let mut rest = vec![];
    for w in ws {
        if let Some(p) = parse_pos(w) {
            nd.args.pos = Some(p);
        } else if let Some(sz) = parse_size(w) {
            nd.args.size = Some(sz);
        } else if let Some(i) = w.find(':') {
            let (k, v) = (&w[..i], &w[i + 1..]);
            if STYLE_WORDS.contains(&k) && !w.starts_with('"') {
                nd.args.style.insert(k.to_string(), unq(v));
            } else {
                rest.push(w.clone());
            }
        } else {
            rest.push(w.clone());
        }
    }
    Ok(rest)
}

/// Parse a line as a child of `parent_is_space`: primitives and positioned nodes inside a space, flowed nodes
/// outside. Positions outside a space, and missing positions inside one, are errors.
pub fn parse_child(line: &str, in_space: bool) -> Result<Node> {
    let first = words(line).into_iter().next().unwrap_or_default();
    // rect, circle and path are always primitives; text, image and line are primitives only inside a space
    let primitive = is_primitive(&first) && (in_space || !["text", "image", "line"].contains(&first.as_str()));
    let nd = if primitive { parse_primitive(line)? } else { parse_node(line)? };
    if in_space && !is_painted(&nd) && nd.args.pos.is_none() {
        return Err(MXError::new(format!("{} inside a sized node needs a position @x,y", nd.ty)));
    }
    if !in_space && nd.args.pos.is_some() {
        return Err(MXError::new(format!("positioned {} needs a sized parent", nd.ty)));
    }
    Ok(nd)
}

/// `@x,y` -> (x, y)
pub fn parse_pos(w: &str) -> Option<(f32, f32)> {
    let body = w.strip_prefix('@')?;
    let (a, b) = body.split_once(',')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// `WxH` -> (w, h)
pub fn parse_size(w: &str) -> Option<(f32, f32)> {
    let (a, b) = w.split_once('x')?;
    if a.is_empty() || b.is_empty() || !a.chars().next().unwrap().is_ascii_digit() {
        return None;
    }
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// Geometry, style and label words shared by every primitive. Returns the positional words it did not take.
pub fn paint_words(nd: &mut Node, ws: &[String]) -> Result<Vec<String>> {
    let mut rest = vec![];
    for w in ws {
        if let Some(p) = parse_pos(w) {
            if nd.args.pos.is_none() {
                nd.args.pos = Some(p);
            } else if nd.ty == "line" && nd.args.pos2.is_none() {
                nd.args.pos2 = Some(p);
            } else {
                return Err(MXError::new(format!("{} has too many positions", nd.ty)));
            }
        } else if let Some(s) = parse_size(w) {
            nd.args.size = Some(s);
        } else if let Some(r) = w.strip_prefix("r:") {
            nd.args.radius = Some(num(r)? as f32);
        } else if w == "dashed" {
            nd.flags.insert("dashed".into());
        } else if w.starts_with('"') {
            if nd.label.is_none() {
                nd.label = Some(unq(w));
            } else {
                return Err(MXError::new(format!("{} has two labels", nd.ty)));
            }
        } else if let Some(i) = w.find(':') {
            let (k, v) = (&w[..i], &w[i + 1..]);
            if STYLE_WORDS.contains(&k) {
                nd.args.style.insert(k.to_string(), unq(v));
            } else if nd.ty == "image" && (w.starts_with("http") || w.starts_with("data:") || w.starts_with("file:")) {
                rest.push(w.clone()); // a URL source
            } else {
                return Err(MXError::new(format!("unknown style word {} on {}", jstr(w), nd.ty)));
            }
        } else if let Some(b) = w.strip_prefix('=') {
            nd.args.bind = Some(b.to_string());
        } else {
            rest.push(w.clone());
        }
    }
    Ok(rest)
}

/// One primitive line inside a canvas.
pub fn parse_primitive(line: &str) -> Result<Node> {
    let ws = words(line);
    let t = ws.first().cloned().unwrap_or_default();
    let mut nd = Node::new(&t);
    let mut rest: Vec<String> = vec![];
    for w in ws.iter().skip(1) {
        if w.starts_with('#') && w.len() > 1 && !is_digits(&w[1..]) {
            nd.explicit = Some(w[1..].to_string());
        } else {
            rest.push(w.clone());
        }
    }
    match t.as_str() {
        "rect" | "circle" | "line" => {
            let left = paint_words(&mut nd, &rest)?;
            if let Some(w) = left.first() {
                return Err(MXError::new(format!("unknown word {} on {} line", jstr(w), t)));
            }
            if nd.args.pos.is_none() {
                return Err(MXError::new(format!("{} needs a position @x,y", t)));
            }
            if t == "rect" && nd.args.size.is_none() {
                return Err(MXError::new("rect needs a size WxH"));
            }
            if t == "circle" && nd.args.radius.is_none() {
                return Err(MXError::new("circle needs r:N"));
            }
            if t == "line" && nd.args.pos2.is_none() {
                return Err(MXError::new("line needs two positions"));
            }
        }
        "path" => {
            // the first quoted word is the path data, a second one the label
            let mut others = rest.clone();
            if let Some(i) = others.iter().position(|w| w.starts_with('"')) {
                nd.args.path = Some(unq(&others.remove(i)));
            }
            let left = paint_words(&mut nd, &others)?;
            if let Some(w) = left.first() {
                return Err(MXError::new(format!("unknown word {} on path line", jstr(w))));
            }
            if nd.args.path.is_none() {
                return Err(MXError::new("path needs quoted path data"));
            }
        }
        "text" => {
            let left = paint_words(&mut nd, &rest)?;
            if let Some(w) = left.first() {
                return Err(MXError::new(format!("unknown word {} on text line", jstr(w))));
            }
            if nd.args.pos.is_none() {
                return Err(MXError::new("text in a canvas needs a position @x,y"));
            }
            if nd.label.is_none() && nd.args.bind.is_none() {
                return Err(MXError::new("text needs its string"));
            }
        }
        "image" => {
            let left = paint_words(&mut nd, &rest)?;
            // src is the first bare word; alt is the quoted label
            match left.first() {
                Some(src) => nd.args.href = Some(src.clone()),
                None => return Err(MXError::new("image needs a source")),
            }
            if left.len() > 1 {
                return Err(MXError::new(format!("unknown word {} on image line", jstr(&left[1]))));
            }
            if nd.label.is_none() {
                return Err(MXError::new("image needs alt text"));
            }
            nd.args.alt = nd.label.clone();
            if nd.args.pos.is_none() {
                return Err(MXError::new("image in a canvas needs a position @x,y"));
            }
        }
        _ => return Err(MXError::new(format!("{} is not a paint primitive", jstr(&t)))),
    }
    Ok(nd)
}

fn allowed_flags(t: &str) -> Option<&'static [&'static str]> {
    match t {
        "button" => Some(&["disabled"]),
        "field" => Some(&["required"]),
        "select" => Some(&["multi"]),
        "row" => Some(&["end", "center", "between"]),
        "toggle" | "check" => Some(&[]),
        _ => None,
    }
}

/// The tokenizer: quoted strings (with escapes), `="quoted"`, and bare words that may end in a quoted part
/// (`Key:"two words"`). Same language as the TypeScript `TOKEN` regex.
pub fn words(s: &str) -> Vec<String> {
    let b: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    let quoted = |i: &mut usize| -> String {
        // b[*i] == '"'
        let start = *i;
        *i += 1;
        while *i < b.len() {
            if b[*i] == '\\' {
                *i += 2;
                continue;
            }
            if b[*i] == '"' {
                *i += 1;
                return b[start..(*i).min(b.len())].iter().collect();
            }
            *i += 1;
        }
        b[start..].iter().collect()
    };
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '"' {
            out.push(quoted(&mut i));
            continue;
        }
        if c == '=' && i + 1 < b.len() && b[i + 1] == '"' {
            i += 1;
            let q = quoted(&mut i);
            out.push(format!("={}", q));
            continue;
        }
        let start = i;
        while i < b.len() && !b[i].is_whitespace() && b[i] != '"' {
            i += 1;
        }
        let mut w: String = b[start..i].iter().collect();
        if i < b.len() && b[i] == '"' {
            w.push_str(&quoted(&mut i));
        }
        out.push(w);
    }
    out
}

fn json_str(s: &str) -> String {
    serde_json::from_str::<String>(s).unwrap_or_else(|_| s.trim_matches('"').to_string())
}

pub fn unq(w: &str) -> String {
    if w.starts_with('"') {
        return json_str(w);
    }
    if w.starts_with("=\"") {
        return json_str(&w[1..]);
    }
    if let Some(rest) = w.strip_prefix('=') {
        return rest.to_string();
    }
    if w.contains(":\"") {
        let i = w.find(':').unwrap();
        return format!("{}:{}", &w[..i], json_str(&w[i + 1..]));
    }
    w.to_string()
}

pub fn slug(s: Option<&str>) -> Option<String> {
    let first: String = s.unwrap_or("").split(' ').next().unwrap_or("").to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if first.is_empty() {
        None
    } else {
        Some(first)
    }
}

/// Tone of a badge cell: explicit `:tone` suffix, else the fixed word list.
pub fn badge_tone(text: &str) -> (String, String) {
    if let Some(i) = text.rfind(':') {
        if i > 0 {
            let t = &text[i + 1..];
            if is_tone(t) {
                return (text[..i].to_string(), t.to_string());
            }
        }
    }
    let lower = text.to_lowercase();
    for (tone, ws) in BADGE_WORDS {
        if ws.split(' ').any(|w| w == lower) {
            return (text.to_string(), tone.to_string());
        }
    }
    (text.to_string(), "muted".into())
}

pub fn metric_default_tone(delta: Option<&str>) -> Option<&'static str> {
    match delta {
        Some(d) if d.starts_with('+') => Some("success"),
        Some(d) if d.starts_with('-') => Some("danger"),
        _ => None,
    }
}

pub(crate) fn num(v: &str) -> Result<f64> {
    if v.trim().is_empty() {
        return Ok(0.0); // JavaScript's Number("") is 0
    }
    v.trim().parse::<f64>().map_err(|_| MXError::new(format!("not a number: {}", v)))
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

fn jstr(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// Parse one node line into a node (no children, no id). Flow chains and table rows are parsed by `parse`.
pub fn parse_node(line: &str) -> Result<Node> {
    let ws = words(line);
    let t = ws.first().cloned().unwrap_or_default();
    let mut nd = Node::new(&t);
    let mut rest: Vec<String> = vec![];
    for w in ws.iter().skip(1) {
        if w.starts_with('#') && t != "table" && w.len() > 1 && !is_digits(&w[1..]) {
            nd.explicit = Some(w[1..].to_string());
        } else {
            rest.push(w.clone());
        }
    }
    // `anim:300ms` on any node: the renderer tweens the next change
    if let Some(i) = rest.iter().position(|w| w.starts_with("anim:")) {
        let w = rest.remove(i);
        nd.args.style.insert("anim".into(), w[5..].to_string());
    }
    rest = geometry_words(&mut nd, &rest)?;
    let ts = t.as_str();
    if is_generic(ts) {
        // region "Label" | item "Label" | figure "Label" | decoration: a role with no default look
        for w in &rest {
            if nd.label.is_none() {
                nd.label = Some(unq(w));
            } else {
                return Err(MXError::new(format!("unknown word {} on {} line", jstr(w), t)));
            }
        }
    } else if ts == "each" {
        for w in &rest {
            if let Some(b) = w.strip_prefix('=') {
                nd.args.bind = Some(b.to_string());
            } else if w == "row" || w == "col" {
                nd.flags.insert(w.clone());
            } else if is_digits(w) {
                nd.args.grid_cols = Some(w.parse().unwrap());
            } else {
                return Err(MXError::new(format!("unknown word {} on each line", jstr(w))));
            }
        }
        if nd.args.bind.is_none() {
            return Err(MXError::new("each needs =buffer"));
        }
    } else if ["row", "col", "card", "stack", "grid", "tabs", "flow"].contains(&ts) {
        for w in &rest {
            if w.starts_with('"') {
                nd.label = Some(unq(w));
            } else if ts == "grid" && is_digits(w) {
                nd.args.grid_cols = Some(w.parse().unwrap());
            } else {
                nd.flags.insert(w.clone());
            }
        }
    } else if ts == "table" {
        // a quoted first word is the caption; later quoted words are column names with spaces
        for (i, w) in rest.iter().enumerate() {
            if w.starts_with('"') && i == 0 {
                nd.label = Some(unq(w));
            } else if w.starts_with('"') {
                let full = unq(w);
                match full.rfind(':') {
                    Some(j) if is_tone(&full[j + 1..]) || &full[j + 1..] == "badge" => nd.args.cols.push((full[..j].to_string(), Some(full[j + 1..].to_string()))),
                    _ => nd.args.cols.push((full, None)),
                }
            } else {
                match w.find(':') {
                    None => nd.args.cols.push((w.clone(), None)),
                    Some(i) => {
                        let k = &w[i + 1..];
                        nd.args.cols.push((w[..i].to_string(), if k.is_empty() { None } else { Some(k.to_string()) }));
                    }
                }
            }
        }
    } else if is_chart(ts) {
        for w in &rest {
            if w.starts_with('"') && nd.label.is_none() {
                nd.label = Some(unq(w));
            } else if let Some(b) = w.strip_prefix('=') {
                nd.args.bind = Some(b.to_string());
            } else if w.contains(':') {
                let hi = w.starts_with('*');
                let body = w.trim_start_matches('*');
                let i = body.rfind(':').unwrap();
                nd.args.data.push((body[..i].to_string(), num(&body[i + 1..])?, hi));
            } else {
                nd.flags.insert(w.clone());
            }
        }
    } else if ts == "metric" {
        let pos: Vec<&String> = rest.iter().filter(|w| !is_tone(w) && !(w.starts_with('=') && !w.starts_with("=\""))).collect();
        let binds: Vec<String> = rest.iter().filter(|w| w.starts_with('=') && !w.starts_with("=\"")).map(|w| w[1..].to_string()).collect();
        if pos.is_empty() && binds.is_empty() {
            return Err(MXError::new("metric needs a label"));
        }
        if pos.is_empty() {
            // metric =key =value: both bound
            nd.args.bind_label = binds.first().cloned();
            nd.args.bind = binds.get(1).cloned();
            nd.label = Some(String::new());
        } else {
            nd.label = Some(unq(pos[0]));
            nd.args.bind = binds.first().cloned();
        }
        nd.args.value = if pos.len() > 1 { Value::Str(unq(pos[1])) } else { Value::None };
        nd.args.delta = if pos.len() > 2 { Some(unq(pos[2])) } else { None };
        let tones: Vec<&String> = rest.iter().filter(|w| is_tone(w)).collect();
        nd.args.tone = if let Some(t) = tones.first() { Some(t.to_string()) } else { metric_default_tone(nd.args.delta.as_deref()).map(String::from) };
    } else if ts == "select" {
        if rest.is_empty() {
            return Err(MXError::new("select needs a label"));
        }
        nd.label = Some(unq(&rest[0]));
        let mut sel: Vec<String> = vec![];
        for w in &rest[1..] {
            if w == "multi" {
                nd.flags.insert(w.clone());
                continue;
            }
            let w2 = w.strip_prefix('*');
            if let Some(w2) = w2 {
                sel.push(unq(w2));
                nd.args.options.push(unq(w2));
            } else {
                nd.args.options.push(unq(w));
            }
        }
        nd.args.value = if sel.len() == 1 { Value::Str(sel[0].clone()) } else { Value::List(sel) };
    } else if ts == "toggle" || ts == "check" {
        if rest.is_empty() {
            return Err(MXError::new(format!("{} needs a label", t)));
        }
        nd.label = Some(unq(&rest[0]));
        nd.args.value = Value::Bool(rest[1..].iter().any(|w| w == "on"));
        for w in &rest[1..] {
            if w != "on" && w != "off" {
                nd.flags.insert(w.clone());
            }
        }
    } else if ts == "slider" {
        if rest.is_empty() {
            return Err(MXError::new("slider needs a label"));
        }
        nd.label = Some(unq(&rest[0]));
        for w in &rest[1..] {
            if let Some(i) = w.find("..") {
                nd.args.min = Some(num(&w[..i])?);
                nd.args.max = Some(num(&w[i + 2..])?);
            } else {
                nd.args.value = Value::Num(num(w)?);
            }
        }
        if nd.args.min.is_none() {
            nd.args.min = Some(0.0);
        }
        if nd.args.max.is_none() {
            nd.args.max = Some(100.0);
        }
        if nd.args.value == Value::None {
            nd.args.value = Value::Num(nd.args.min.unwrap());
        }
    } else if ts == "field" {
        if rest.is_empty() {
            return Err(MXError::new("field needs a label"));
        }
        nd.label = Some(unq(&rest[0]));
        nd.args.kind = Some("text".into());
        nd.args.value = Value::Str(String::new());
        for w in &rest[1..] {
            if w.starts_with('=') {
                nd.args.value = Value::Str(unq(w));
            } else if FIELD_KINDS.contains(&w.as_str()) {
                nd.args.kind = Some(w.clone());
            } else {
                nd.flags.insert(w.clone());
            }
        }
    } else if ts == "tab" {
        if rest.is_empty() {
            return Err(MXError::new("tab needs a label"));
        }
        let mut lab = rest[0].as_str();
        if let Some(l) = lab.strip_prefix('*') {
            nd.flags.insert("selected".into());
            lab = l;
        }
        nd.label = Some(unq(lab));
    } else if ts == "series" {
        let name = rest.first().filter(|w| !w.contains(':')).ok_or_else(|| MXError::new("series needs a name"))?;
        nd.label = Some(unq(name));
        nd.explicit = Some(unq(name));
        for w in &rest[1..] {
            let i = w.rfind(':').ok_or_else(|| MXError::new(format!("series expects key:value, got {}", jstr(w))))?;
            nd.args.data.push((w[..i].to_string(), num(&w[i + 1..])?, false));
        }
    } else if ["h1", "h2", "h3", "text", "badge", "button", "link", "image"].contains(&ts) {
        let allowed = |w: &str| is_tone(w) || w == "disabled" || w == "enabled";
        // key=value extras; a URL with a query string is not one
        let kv: Vec<&String> = rest.iter().filter(|w| w.contains('=') && !w.starts_with('=') && !w.starts_with('"') && !w.contains("://") && !w.starts_with("data:")).collect();
        for w in &kv {
            let i = w.find('=').unwrap();
            nd.args.kv.insert(w[..i].to_string(), unq(&w[i + 1..]));
        }
        let mut r: Vec<String> = rest.iter().filter(|w| !kv.contains(w)).cloned().collect();
        while let Some(last) = r.last() {
            if !allowed(last) {
                break;
            }
            let w = r.pop().unwrap();
            if is_tone(&w) {
                nd.args.tone = Some(w);
            } else {
                nd.flags.insert(w);
            }
        }
        if is_textlike(ts) {
            if r.len() == 1 && r[0].starts_with('=') && !r[0].starts_with("=\"") {
                nd.args.bind = Some(r[0][1..].to_string());
                r.clear();
            }
            nd.label = if r.is_empty() { None } else { Some(r.iter().map(|w| unq(w)).collect::<Vec<_>>().join(" ")) };
        } else {
            if !r.is_empty() {
                nd.label = Some(unq(&r[0]));
            }
            for w in r.iter().skip(1) {
                if ts == "link" && nd.args.href.is_none() {
                    nd.args.href = Some(w.clone());
                } else if ts == "image" && w.starts_with('"') {
                    nd.args.alt = Some(unq(w));
                } else if ts == "button" && is_tone(w) {
                    nd.args.tone = Some(w.clone());
                } else if ts == "button" && w == "disabled" {
                    nd.flags.insert(w.clone());
                } else {
                    return Err(MXError::new(format!("unknown word {} on {} line", jstr(w), t)));
                }
            }
        }
    } else if is_primitive(ts) {
        return Err(MXError::new(format!("{} needs a position inside a sized node", t)));
    } else if ts == "canvas" || ts == "g" {
        return Err(MXError::new(format!("{} is gone: give any node a size (region \"Label\" 640x320) and position its children with @x,y", t)));
    } else {
        return Err(MXError::new(format!("unknown node type {}", jstr(&t))));
    }
    validate(&nd)?;
    Ok(nd)
}

fn validate(nd: &Node) -> Result<()> {
    let t = nd.ty.as_str();
    if ["button", "field", "select", "check", "toggle", "slider", "tab"].contains(&t) && nd.label.as_deref().unwrap_or("").is_empty() {
        return Err(MXError::new(format!("{} needs a label", t)));
    }
    if t == "image" && nd.args.alt.as_deref().unwrap_or("").is_empty() {
        return Err(MXError::new("image needs alt text"));
    }
    if let Some(ok) = allowed_flags(t) {
        let mut bad: Vec<&String> = nd.flags.iter().filter(|f| !ok.contains(&f.as_str())).collect();
        bad.sort();
        if !bad.is_empty() {
            let list = serde_json::to_string(&bad).unwrap();
            return Err(MXError::new(format!("unknown flag(s) {} on {}", list, t)));
        }
    }
    Ok(())
}

fn parse_flow_chain(s: &mut Surface, flow: usize, line: &str) -> Result<()> {
    let mut ws = words(line);
    let mut flags: BTreeSet<String> = BTreeSet::new();
    while let Some(last) = ws.last() {
        if !last.starts_with('>') && ["dashed", "solid", "below", "above"].contains(&last.as_str()) {
            flags.insert(ws.pop().unwrap());
        } else {
            break;
        }
    }
    let node = |s: &mut Surface, tokword: &str| -> Result<usize> {
        let full = unq(tokword);
        let (name, tone) = match full.find(':') {
            None => (full.clone(), String::new()),
            Some(i) => (full[..i].to_string(), full[i + 1..].to_string()),
        };
        if !tone.is_empty() && !is_tone(&tone) {
            return Err(MXError::new(format!("unknown tone {} on flow node {}", jstr(&tone), jstr(&name))));
        }
        let existing = s.nodes[flow].args.nodes.clone();
        for idx in existing {
            if s.nodes[idx].label.as_deref() == Some(name.as_str()) {
                if !tone.is_empty() {
                    s.nodes[idx].args.tone = Some(tone.clone());
                }
                return Ok(idx);
            }
        }
        let mut n = Node::new("node");
        n.label = Some(name);
        n.args.tone = if tone.is_empty() { None } else { Some(tone) };
        let idx = s.push(n);
        s.nodes[flow].args.nodes.push(idx);
        Ok(idx)
    };
    if ws.is_empty() {
        return Ok(());
    }
    let mut prev = node(s, &ws[0])?;
    let mut i = 1;
    while i < ws.len() {
        let lab = if ws[i].starts_with('>') && ws[i] != ">" { Some(unq(&ws[i][1..])) } else { None };
        if i + 1 >= ws.len() {
            return Err(MXError::new("flow chain ends with an arrow"));
        }
        let nxt = node(s, &ws[i + 1])?;
        s.nodes[flow].args.edges.push(Edge { from: prev, to: nxt, label: lab, flags: flags.clone(), id: String::new() });
        prev = nxt;
        i += 2;
    }
    Ok(())
}

pub fn parse_head(line: &str) -> Result<(String, Vec<String>)> {
    let ws = words(&line[1..]);
    if ws.is_empty() || ws[0].is_empty() {
        return Err(MXError::new("surface needs an id after @"));
    }
    Ok((ws[0].clone(), ws[1..].to_vec()))
}

/// Parse a whole MX document. Ids are derived for the whole tree at once, so duplicates scope symmetrically.
pub fn parse(text: &str) -> Result<Surface> {
    let mut s = Surface::new();
    // (indent, node, the node is a coordinate space for its children)
    let mut stack: Vec<(i64, usize, bool)> = vec![(-1, s.root(), false)];
    for (i, raw) in text.split('\n').enumerate() {
        let ln = i + 1;
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if raw.trim().is_empty() {
            continue;
        }
        let ind = raw.len() - raw.trim_start_matches(' ').len();
        let line = raw.trim();
        let r: Result<()> = (|| {
            if line.starts_with('@') {
                let (id, life) = parse_head(line)?;
                s.id = id;
                s.life = life;
                return Ok(());
            }
            while stack.last().map(|x| x.0 >= ind as i64).unwrap_or(false) {
                stack.pop();
            }
            let (parent, in_space) = {
                let top = stack.last().unwrap();
                (top.1, top.2)
            };
            if is_painted(&s.nodes[parent]) {
                return Err(MXError::new(format!("{} cannot have children", s.nodes[parent].ty)));
            }
            if in_space {
                let nd = parse_child(line, true)?;
                let space = is_space(&nd) || is_positioned(&nd);
                let idx = s.push(nd);
                s.nodes[parent].children.push(idx);
                stack.push((ind as i64, idx, space));
                return Ok(());
            }
            if s.nodes[parent].ty == "table" {
                s.nodes[parent].args.rows.push(line.split('|').map(|c| c.trim().to_string()).collect());
                return Ok(());
            }
            if s.nodes[parent].ty == "flow" {
                return parse_flow_chain(&mut s, parent, line);
            }
            let nd = parse_child(line, false)?;
            let space = is_space(&nd);
            let idx = s.push(nd);
            s.nodes[parent].children.push(idx);
            stack.push((ind as i64, idx, space));
            Ok(())
        })();
        if let Err(e) = r {
            return Err(MXError::new(e.message).at(ln));
        }
    }
    if s.id.is_empty() {
        return Err(MXError::new("document needs an @surface line").at(1));
    }
    let ids = crate::perceive::derive_all(&s);
    fn fix(s: &mut Surface, n: usize, ids: &std::collections::BTreeMap<usize, String>) {
        let children = s.nodes[n].children.clone();
        for c in children {
            let id = ids[&c].clone();
            s.nodes[c].id = id.clone();
            s.register(&id, c);
            if s.nodes[c].ty == "flow" {
                let fnodes = s.nodes[c].args.nodes.clone();
                for x in fnodes {
                    let xid = ids[&x].clone();
                    s.nodes[x].id = xid.clone();
                    s.register(&xid, x);
                }
                let edges: Vec<Edge> = s.nodes[c].args.edges.iter().map(|e| Edge { id: format!("{}>{}", s.nodes[e.from].id, s.nodes[e.to].id), ..e.clone() }).collect();
                s.nodes[c].args.edges = edges;
            }
            fix(s, c, ids);
        }
    }
    let root = s.root();
    fix(&mut s, root, &ids);
    Ok(s)
}
