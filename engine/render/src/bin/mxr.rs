//! mxr: the Manifest renderer on the command line.
//!   mxr render <file.mx> <out.png>      paint a surface
//!   mxr observe <file.mx> [0|1|2|3]       perception from the engine's own layout
//!   mxr boxes <file.mx>                 boxes and diagnostics as JSON as JSON
//!   mxr parity <engine/render/tests/dom-boxes> <bench/corpus>   compare boxes with Chromium's recorded layout
//!   --dark before the command paints with the dark palette
use mx_render::{Renderer, Theme};
use std::collections::BTreeMap;
use std::fs;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let mut r = Renderer::new();
    if args.get(1).map(String::as_str) == Some("--dark") {
        r.theme = Theme::dark();
        args.remove(1);
    }
    match args.get(1).map(String::as_str) {
        Some("render") => {
            let s = mx::parse(&fs::read_to_string(&args[2]).unwrap()).unwrap_or_else(|e| die(&e.to_string()));
            r.load_images(&s);
            fs::write(&args[3], r.render_png(&s)).unwrap();
            println!("wrote {}", args[3]);
        }
        Some("observe") => {
            let s = mx::parse(&fs::read_to_string(&args[2]).unwrap()).unwrap_or_else(|e| die(&e.to_string()));
            let level: u8 = args.get(3).and_then(|l| l.parse().ok()).unwrap_or(1);
            println!("{}", r.observe(&s, level, None).unwrap_or_else(|e| die(&e.to_string())));
        }
        Some("boxes") => {
            let s = mx::parse(&fs::read_to_string(&args[2]).unwrap()).unwrap_or_else(|e| die(&e.to_string()));
            let out = r.layout(&s);
            println!("{}", boxes_json(&out.boxes, &out.diags));
        }
        Some("parity") => parity(&r, &args[2], &args[3]),
        Some("flow") => {
            let s = mx::parse(&fs::read_to_string(&args[2]).unwrap()).unwrap_or_else(|e| die(&e.to_string()));
            for (i, n) in s.nodes.iter().enumerate() {
                if n.ty == "flow" {
                    let fl = mx_render::flow::layout_flow(&s, n, &r.fonts);
                    println!("{}x{} nodes={:?} edges={:?}", fl.width, fl.height, fl.nodes.iter().map(|p| (p.id.clone(), p.x, p.y, p.w)).collect::<Vec<_>>(), fl.edges.iter().map(|e| (e.id.clone(), format!("{:?}", e.route))).collect::<Vec<_>>());
                    let _ = i;
                }
            }
        }
        _ => die("usage: mxr render|observe|boxes|parity|flow ..."),
    }
}

fn die(msg: &str) -> ! {
    eprintln!("{}", msg);
    std::process::exit(1)
}

fn boxes_json(boxes: &BTreeMap<String, mx::Box>, diags: &[mx::Diag]) -> String {
    let b: Vec<String> = boxes.iter().map(|(k, v)| format!("{}:{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}", serde_json::to_string(k).unwrap(), v.x, v.y, v.w, v.h)).collect();
    let d: Vec<String> = diags.iter().map(|d| serde_json::to_string(&mx::format_diag(d)).unwrap()).collect();
    format!("{{\"boxes\":{{{}}},\"diags\":[{}]}}", b.join(","), d.join(","))
}

/// Compare the engine's boxes with the DOM renderer's (bench/out/native/<name>.json) for every corpus file.
fn parity(r: &Renderer, native_dir: &str, corpus_dir: &str) {
    let mut names: Vec<String> = fs::read_dir(corpus_dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|f| f.ends_with(".mx")).map(|f| f.trim_end_matches(".mx").to_string()).collect();
    names.sort();
    println!("| surface | node | dom (x,y,w,h) | rust (x,y,w,h) | max dev |");
    println!("|---|---|---|---|---|");
    let mut worst = 0;
    let mut over = 0;
    let mut total = 0;
    for n in &names {
        let s = match mx::parse(&fs::read_to_string(format!("{}/{}.mx1", corpus_dir, n)).unwrap()) {
            Ok(s) => s,
            Err(e) => {
                println!("| {} | parse error {} | | | |", n, e);
                continue;
            }
        };
        let out = r.layout(&s);
        let native: serde_json::Value = match fs::read_to_string(format!("{}/{}.json", native_dir, n)) {
            Ok(t) => serde_json::from_str(&t).unwrap(),
            Err(_) => continue,
        };
        let dom = native["boxes"].as_object().unwrap();
        for (id, bv) in dom {
            let d = [bv["x"].as_i64().unwrap() as i32, bv["y"].as_i64().unwrap() as i32, bv["w"].as_i64().unwrap() as i32, bv["h"].as_i64().unwrap() as i32];
            total += 1;
            match out.boxes.get(id) {
                Some(b) => {
                    let rr = [b.x, b.y, b.w, b.h];
                    let dev = (0..4).map(|i| (d[i] - rr[i]).abs()).max().unwrap();
                    worst = worst.max(dev);
                    if dev > 2 {
                        over += 1;
                    }
                    println!("| {} | {} | {},{},{},{} | {},{},{},{} | {}{} |", n, id, d[0], d[1], d[2], d[3], rr[0], rr[1], rr[2], rr[3], dev, if dev > 2 { " ⚠" } else { "" });
                }
                None => {
                    over += 1;
                    println!("| {} | {} | {},{},{},{} | missing | ⚠ |", n, id, d[0], d[1], d[2], d[3]);
                }
            }
        }
        let dd: Vec<String> = out.diags.iter().map(mx::format_diag).collect();
        let nd: Vec<String> = native["diags"].as_array().map(|a| a.iter().map(|d| format!("!{} {}", d["kind"].as_str().unwrap_or(""), d["addr"].as_str().unwrap_or(""))).collect()).unwrap_or_default();
        println!("| {} | diagnostics | {} | {} | |", n, nd.join("; "), dd.join("; "));
    }
    println!("\n{} boxes, {} beyond 2px, worst deviation {}px", total, over, worst);
}
