//! A small MCP server over stdio (JSON-RPC 2.0, one message per line): the same five tools and the spec
//! resource as packages/mcp, served from the Rust host.
use mx_render::host::Host;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const CHEATSHEET: &str = include_str!("../../../spec/cheatsheet.md");
pub const SPEC: &str = include_str!("../../../spec/v1.md");

/// Something to tell the window: repaint, or the client went away.
#[derive(Debug, Clone, Copy)]
pub enum Wake {
    Redraw,
    Quit,
}

pub trait Waker: Send {
    fn wake(&self, w: Wake);
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn tools() -> Value {
    let s = |d: &str| json!({ "type": "string", "description": d });
    json!([
        { "name": "manifest.render", "title": "Render a surface",
          "description": "Show a UI. `mx` is a full MX document starting with its @surface line (see the server instructions or manifest://spec). Rendering an existing surface id replaces it. Returns the surface's L0 line, or `error line=N ...`.",
          "inputSchema": { "type": "object", "properties": { "mx": s("The MX document") }, "required": ["mx"] },
          "annotations": { "readOnlyHint": false, "idempotentHint": true, "openWorldHint": false } },
        { "name": "manifest.patch", "title": "Patch a surface",
          "description": "Change part of a surface. `patch` starts with `~<rev>` (the current rev + 1), then `*addr args` / `+addr node` / `-addr` lines. Rejected as a whole if the rev is stale or an address does not resolve. Returns the L0 line.",
          "inputSchema": { "type": "object", "properties": { "surface": s("Surface id"), "patch": s("The patch block") }, "required": ["surface", "patch"] },
          "annotations": { "readOnlyHint": false, "idempotentHint": false, "openWorldHint": false } },
        { "name": "manifest.observe", "title": "Read a surface back",
          "description": "Read the current state as MX text. level 0: one line; 1 (default): the tree with live values and !diagnostics, painted shapes only when labelled; 2: plus @x,y,w,h layout boxes; 3: every painted shape as written, with boxes. `since` returns only what changed after that rev, as a patch. `focus` limits to one subtree. `wait` blocks up to that many seconds for a user event, so you can wait for an answer to a dialog. Without `surface`, lists every surface's L0 line and residue lines.",
          "inputSchema": { "type": "object", "properties": {
              "surface": s("Surface id; omit to list all"),
              "level": { "type": "integer", "minimum": 0, "maximum": 3, "description": "0, 1, 2 or 3" },
              "since": { "type": "integer", "description": "Return a patch relative to this rev" },
              "focus": s("Node id to narrow to"),
              "wait": { "type": "number", "minimum": 0, "maximum": 600, "description": "Seconds to wait for a user event (action, input, dissolve) before answering; returns as soon as one arrives" } } },
          "annotations": { "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false } },
        { "name": "manifest.look", "title": "Look at pixels",
          "description": "A PNG of one node (or the whole surface when `id` is omitted). The only pixel path: use observe for everything else, and look only to judge appearance.",
          "inputSchema": { "type": "object", "properties": { "surface": s("Surface id"), "id": s("Node id") }, "required": ["surface"] },
          "annotations": { "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false } },
        { "name": "manifest.dissolve", "title": "Dissolve a surface",
          "description": "Remove a surface now, honouring its residue setting. Persistent surfaces only go away this way or when the user closes them.",
          "inputSchema": { "type": "object", "properties": { "surface": s("Surface id") }, "required": ["surface"] },
          "annotations": { "readOnlyHint": false, "idempotentHint": true, "openWorldHint": false } },
    ])
}

fn text(t: String) -> Value {
    json!({ "type": "text", "text": t })
}

fn str_arg<'a>(args: &'a Value, k: &str) -> Option<&'a str> {
    args.get(k).and_then(Value::as_str)
}

/// Run a tool. Returns (content, is_error); events since the last call are prepended by the caller.
fn call(host: &mut Host, name: &str, args: &Value) -> (Vec<Value>, bool) {
    let need = |k: &str| -> Result<&str, String> { str_arg(args, k).ok_or_else(|| format!("{} needs `{}`", name, k)) };
    let r: Result<Vec<Value>, String> = (|| match name {
        "manifest.render" => {
            host.turn();
            host.render(need("mx")?).map(|l0| vec![text(l0)]).map_err(|e| e.to_string())
        }
        "manifest.patch" => host.patch(need("surface")?, need("patch")?).map(|l0| vec![text(l0)]).map_err(|e| e.to_string()),
        "manifest.observe" => {
            let level = args.get("level").and_then(Value::as_u64).unwrap_or(1) as u8;
            let since = args.get("since").and_then(Value::as_u64).map(|v| v as u32);
            let focus = str_arg(args, "focus");
            match str_arg(args, "surface") {
                Some(s) => host.observe(s, level, since, focus).map(|t| vec![text(t)]).map_err(|e| e.to_string()),
                None => Ok(vec![text(host.list())]),
            }
        }
        "manifest.look" => {
            let s = need("surface")?;
            let id = str_arg(args, "id");
            let (png, w, h) = host.look(s, id).map_err(|e| e.to_string())?;
            Ok(vec![text(format!("look {}{} {}x{}px", s, id.map(|i| format!(" {}", i)).unwrap_or_default(), w, h)), json!({ "type": "image", "data": base64(&png), "mimeType": "image/png" })])
        }
        "manifest.dissolve" => {
            let s = need("surface")?;
            host.dissolve(s, "model");
            Ok(vec![text(format!("@{} dissolving", s))])
        }
        _ => Err(format!("unknown tool {}", name)),
    })();
    match r {
        Ok(c) => (c, false),
        Err(e) => (vec![text(format!("error {}", e))], true),
    }
}

fn with_events(host: &mut Host, mut content: Vec<Value>) -> Vec<Value> {
    let events = host.drain();
    if events.is_empty() {
        return content;
    }
    let ev = events.join("\n");
    match content.first_mut() {
        Some(first) if first["type"] == "text" => {
            let t = format!("{}\n{}", ev, first["text"].as_str().unwrap_or(""));
            first["text"] = Value::String(t);
        }
        _ => content.insert(0, text(ev)),
    }
    content
}

pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn handle(host: &Arc<Mutex<Host>>, waker: &Option<Box<dyn Waker>>, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let reply = |result: Value| id.clone().map(|id| json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    let error = |code: i64, m: &str| id.clone().map(|id| json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": m } }));
    match method {
        "initialize" => {
            let v = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("2025-06-18");
            reply(json!({ "protocolVersion": v, "capabilities": { "tools": {}, "resources": {} }, "serverInfo": { "name": "manifest", "version": env!("CARGO_PKG_VERSION") }, "instructions": CHEATSHEET.trim() }))
        }
        "notifications/initialized" | "notifications/cancelled" => None,
        "ping" => reply(json!({})),
        "tools/list" => reply(json!({ "tools": tools() })),
        "resources/list" => reply(json!({ "resources": [{ "uri": "manifest://spec", "name": "spec", "title": "Manifest MX specification", "mimeType": "text/markdown" }] })),
        "resources/templates/list" => reply(json!({ "resourceTemplates": [] })),
        "prompts/list" => reply(json!({ "prompts": [] })),
        "resources/read" => match params.get("uri").and_then(Value::as_str) {
            Some("manifest://spec") => reply(json!({ "contents": [{ "uri": "manifest://spec", "mimeType": "text/markdown", "text": SPEC }] })),
            _ => error(-32002, "resource not found"),
        },
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            if name == "manifest.observe" {
                if let Some(w) = args.get("wait").and_then(Value::as_f64) {
                    let until = now_ms() + (w * 1000.0) as u64;
                    loop {
                        {
                            let mut h = host.lock().unwrap();
                            h.tick(now_ms());
                            if !h.events.is_empty() {
                                break;
                            }
                        }
                        if now_ms() >= until {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
            let (content, is_error) = {
                let mut h = host.lock().unwrap();
                h.tick(now_ms());
                let (c, e) = call(&mut h, &name, &args);
                (with_events(&mut h, c), e)
            };
            if let Some(w) = waker {
                w.wake(Wake::Redraw);
            }
            let mut r = json!({ "content": content });
            if is_error {
                r["isError"] = Value::Bool(true);
            }
            reply(r)
        }
        _ => error(-32601, "method not found"),
    }
}

/// Serve until stdin closes. `waker` is told when the screen should repaint and when to quit.
pub fn serve(host: Arc<Mutex<Host>>, waker: Option<Box<dyn Waker>>) {
    let stdin = std::io::stdin();
    let out = Mutex::new(std::io::stdout());
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                let mut o = out.lock().unwrap();
                let _ = writeln!(o, "{}", json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "parse error" } }));
                let _ = o.flush();
                continue;
            }
        };
        let replies: Vec<Value> = match msg.as_array() {
            Some(batch) => batch.iter().filter_map(|m| handle(&host, &waker, m)).collect(),
            None => handle(&host, &waker, &msg).into_iter().collect(),
        };
        let mut o = out.lock().unwrap();
        for r in replies {
            let _ = writeln!(o, "{}", r);
        }
        let _ = o.flush();
    }
    if let Some(w) = waker {
        w.wake(Wake::Quit);
    }
}
