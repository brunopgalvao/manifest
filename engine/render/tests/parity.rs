//! The parity gate: every box Chromium laid out for the same stylesheet and font (tests/dom-boxes, recorded
//! from the earlier DOM renderer with Inter) is reproduced by the engine within 2px, with the same diagnostics.
use mx_render::Renderer;
use std::fs;

const NAMES: &[&str] = &["dash", "form", "table", "flow", "confirm", "settings"];

#[test]
fn boxes_match_the_dom_renderer_within_2px() {
    let r = Renderer::new();
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut checked = 0;
    for n in NAMES {
        let s = mx::parse(&fs::read_to_string(format!("{}/bench/corpus/{}.mx", root, n)).unwrap()).unwrap();
        let out = r.layout(&s);
        let native: serde_json::Value = serde_json::from_str(&fs::read_to_string(format!("{}/engine/render/tests/dom-boxes/{}.json", root, n)).unwrap()).unwrap();
        for (id, b) in native["boxes"].as_object().unwrap() {
            let d = [b["x"].as_i64().unwrap() as i32, b["y"].as_i64().unwrap() as i32, b["w"].as_i64().unwrap() as i32, b["h"].as_i64().unwrap() as i32];
            let got = out.boxes.get(id).unwrap_or_else(|| panic!("{}: no box for {}", n, id));
            let g = [got.x, got.y, got.w, got.h];
            for i in 0..4 {
                assert!((d[i] - g[i]).abs() <= 2, "{}.{}: dom {:?} rust {:?}", n, id, d, g);
            }
            checked += 1;
        }
        let want: Vec<String> = native["diags"].as_array().unwrap().iter().map(|d| format!("{} {}", d["kind"].as_str().unwrap(), d["addr"].as_str().unwrap())).collect();
        let got: Vec<String> = out.diags.iter().map(|d| mx::format_diag(d).trim_start_matches('!').split(' ').take(2).collect::<Vec<_>>().join(" ")).collect();
        assert_eq!(got, want, "{}: diagnostics", n);
    }
    assert!(checked >= 49, "checked {} boxes", checked);
}

#[test]
fn perception_from_the_engine_reads_like_the_fixtures() {
    let r = Renderer::new();
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let s = mx::parse(&fs::read_to_string(format!("{}/bench/corpus/dash.mx", root)).unwrap()).unwrap();
    let l2 = r.observe(&s, 2, None).unwrap();
    assert!(l2.starts_with("@dash rev=0 @0,0,728,"), "{}", l2);
    assert!(l2.contains("metric Revenue $308k +14% @17,54,223,83"), "{}", l2);
    let png = r.render_png(&s);
    assert_eq!(&png[1..4], b"PNG");
}
