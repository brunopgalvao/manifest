//! The server end to end: JSON-RPC over the binary's stdin and stdout, headless.
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn rpc(stdin: &mut impl Write, out: &mut impl BufRead, id: u64, method: &str, params: serde_json::Value) -> serde_json::Value {
    writeln!(stdin, "{}", serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{}: {}", e, line));
    assert_eq!(v["id"], id);
    v
}

fn text(r: &serde_json::Value) -> String {
    r["result"]["content"].as_array().unwrap().iter().filter(|c| c["type"] == "text").map(|c| c["text"].as_str().unwrap()).collect::<Vec<_>>().join("\n")
}

#[test]
fn render_observe_patch_look_dissolve_over_stdio() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut child = Command::new(env!("CARGO_BIN_EXE_manifest-mcp")).arg("--headless").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let init = rpc(&mut stdin, &mut out, 1, "initialize", serde_json::json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }));
    assert_eq!(init["result"]["serverInfo"]["name"], "manifest");
    assert!(init["result"]["instructions"].as_str().unwrap().starts_with("Manifest renders UI"));
    writeln!(stdin, "{}", serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).unwrap();
    let tools = rpc(&mut stdin, &mut out, 2, "tools/list", serde_json::json!({}));
    let mut names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["manifest.dissolve", "manifest.look", "manifest.observe", "manifest.patch", "manifest.render"]);
    let spec = rpc(&mut stdin, &mut out, 3, "resources/read", serde_json::json!({ "uri": "manifest://spec" }));
    assert!(spec["result"]["contents"][0]["text"].as_str().unwrap().starts_with("# Manifest MX v1.2"));
    let dash = std::fs::read_to_string(format!("{}/bench/corpus/dash.mx", root)).unwrap();
    let r = rpc(&mut stdin, &mut out, 4, "tools/call", serde_json::json!({ "name": "manifest.render", "arguments": { "mx": dash } }));
    assert_eq!(text(&r), "@dash rev=0 nodes=9");
    let l2 = text(&rpc(&mut stdin, &mut out, 5, "tools/call", serde_json::json!({ "name": "manifest.observe", "arguments": { "surface": "dash", "level": 2 } })));
    assert!(l2.starts_with("@dash rev=0 @0,0,728,"), "{}", l2);
    assert!(l2.contains("bar \"Monthly revenue (k$)\" Apr:38 May:42 Jun:51 Jul:47 Aug:57 *Sep:73 @"), "{}", l2);
    let p = rpc(&mut stdin, &mut out, 6, "tools/call", serde_json::json!({ "name": "manifest.patch", "arguments": { "surface": "dash", "patch": "~1\n*revenue $315k +16%\n*bar.Sep 80\n*export enabled" } }));
    assert_eq!(text(&p), "@dash rev=1 nodes=9");
    let since = rpc(&mut stdin, &mut out, 7, "tools/call", serde_json::json!({ "name": "manifest.observe", "arguments": { "surface": "dash", "since": 0 } }));
    assert_eq!(text(&since), "~1\n*revenue $315k +16%\n*bar.Sep 80\n*export \"Export CSV\" enabled");
    let stale = rpc(&mut stdin, &mut out, 8, "tools/call", serde_json::json!({ "name": "manifest.patch", "arguments": { "surface": "dash", "patch": "~1\n*export disabled" } }));
    assert_eq!(stale["result"]["isError"], true);
    assert_eq!(text(&stale), "error stale rev 1 (surface at 1)");
    let bad = rpc(&mut stdin, &mut out, 9, "tools/call", serde_json::json!({ "name": "manifest.render", "arguments": { "mx": "@x\nh1 Hi\nbutton Go primay" } }));
    assert_eq!(text(&bad), "error line=3 unknown word \"primay\" on button line");
    let look = rpc(&mut stdin, &mut out, 10, "tools/call", serde_json::json!({ "name": "manifest.look", "arguments": { "surface": "dash", "id": "bar" } }));
    assert!(text(&look).starts_with("look dash bar "), "{}", text(&look));
    let img = look["result"]["content"].as_array().unwrap().iter().find(|c| c["type"] == "image").unwrap();
    assert_eq!(img["mimeType"], "image/png");
    assert!(img["data"].as_str().unwrap().starts_with("iVBORw0KGgo"));
    let toast = rpc(&mut stdin, &mut out, 11, "tools/call", serde_json::json!({ "name": "manifest.render", "arguments": { "mx": "@toast ttl:100ms residue:line\nh1 Saved" } }));
    assert_eq!(text(&toast), "@toast ttl:100ms residue:line rev=0 nodes=1");
    // wait returns as soon as the ttl event arrives, and the event heads the result
    let waited = rpc(&mut stdin, &mut out, 12, "tools/call", serde_json::json!({ "name": "manifest.observe", "arguments": { "wait": 5 } }));
    assert_eq!(text(&waited), "@toast dissolved reason=ttl:100ms rev=0\n@dash rev=1 nodes=9\n@toast dissolved: Saved");
    let d = rpc(&mut stdin, &mut out, 13, "tools/call", serde_json::json!({ "name": "manifest.dissolve", "arguments": { "surface": "dash" } }));
    assert_eq!(text(&d), "@dash dissolved reason=model rev=1\n@dash dissolving");
    let list = rpc(&mut stdin, &mut out, 14, "tools/call", serde_json::json!({ "name": "manifest.observe", "arguments": {} }));
    assert_eq!(text(&list), "@toast dissolved: Saved");
    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}
