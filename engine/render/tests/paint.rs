//! The paint layer in the renderer: boxes for primitives, painted groups that take input, offcanvas
//! diagnostics, and the fidelity tiers 2 (raster plus semantic overlay) and 3 (vector reconstruction).
use mx_render::host::Host;
use mx_render::{Hit, Renderer};
use tiny_skia::Pixmap;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

const FUNNEL: &str = r#"@chart-editor
h2 "Q3 pipeline"
region 640x320 #pipe
  rect @0,0 640x320 fill:muted-50 #bg
  region "Funnel" @40,20
    rect @0,0 560x48 fill:primary radius:6 #leads "Leads 1,240"
    rect @40,64 480x48 fill:primary-70 radius:6 #qualified "Qualified 610"
    rect @120,128 320x48 fill:success radius:6 #won "Won 95"
    text @280,210 "7.7% conversion" size:14 anchor:middle
  path "M 300 180 L 300 200" stroke:muted dashed
  circle @300,200 r:4 fill:danger #marker "Target"
  button "Export PNG" @520,280 100x32
    rect @0,0 100x32 fill:primary radius:16
    text @50,21 "Export PNG" fill:white anchor:middle size:13 weight:600
  button Reset @400,280
  toggle Live on @20,280
  rect @700,10 20x20 fill:danger #stray "Off the edge"
row end
  button Clear
"#;

#[test]
fn funnel_lays_out_paints_and_takes_input() {
    let mut h = Host::new(Renderer::new());
    assert_eq!(h.render(FUNNEL).unwrap(), "@chart-editor rev=0 nodes=18 diags=1");
    let l2 = h.observe("chart-editor", 2, None, None).unwrap();
    let boxes = h.entry("chart-editor").unwrap().rendered.as_ref().unwrap().boxes.clone();
    let pipe = boxes["pipe"];
    assert_eq!((pipe.w, pipe.h), (640, 320), "{}", l2);
    let leads = boxes["leads"];
    assert_eq!((leads.x - pipe.x, leads.y - pipe.y, leads.w, leads.h), (40, 20, 560, 48));
    let marker = boxes["marker"];
    assert_eq!((marker.x - pipe.x, marker.y - pipe.y, marker.w, marker.h), (296, 196, 8, 8));
    let export = boxes["export"];
    assert_eq!((export.x - pipe.x, export.y - pipe.y, export.w, export.h), (520, 280, 100, 32));
    assert!(l2.ends_with("!offcanvas stray"), "{}", l2);
    assert!(l2.contains("button \"Export PNG\" @520,280 100x32 @"), "{}", l2);
    // positioned nodes with no paint children get their default look at their natural size
    let reset = boxes["reset"];
    assert_eq!((reset.x - pipe.x, reset.y - pipe.y, reset.h), (400, 280, 36), "{:?}", reset);
    assert!(reset.w > 50 && reset.w < 90, "{:?}", reset);
    assert_eq!(h.click("chart-editor", reset.x as f32 + 20.0, reset.y as f32 + 18.0).unwrap(), "@chart-editor action reset rev=0 {\"live\":true}");
    let live = boxes["live"];
    assert_eq!(h.click("chart-editor", live.x as f32 + 10.0, live.y as f32 + 18.0).unwrap(), "@chart-editor input live off rev=1");
    // the painted button is a target
    let (cx, cy) = (export.x as f32 + 50.0, export.y as f32 + 16.0);
    assert!(matches!(h.hit("chart-editor", cx, cy), Some((Hit::Button(id), _)) if id == "export"));
    assert_eq!(h.click("chart-editor", cx, cy).unwrap(), "@chart-editor action export rev=1 {\"live\":false}");
    // and a patch moves things
    h.patch("chart-editor", "~2\n*marker @300,180\n*won 340x48 fill:warning\n-stray").unwrap();
    let l2 = h.observe("chart-editor", 2, None, None).unwrap();
    assert!(!l2.contains("!offcanvas"), "{}", l2);
    let (png, _, _) = h.look("chart-editor", None).unwrap();
    std::fs::create_dir_all(format!("{}/bench/out/rust", ROOT)).unwrap();
    std::fs::write(format!("{}/bench/out/rust/funnel.png", ROOT), png).unwrap();
}

fn diff_stats(a: &Pixmap, b: &Pixmap) -> (usize, usize, f64, u8) {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()), "size");
    let (mut any, mut big, mut sum, mut max) = (0usize, 0usize, 0u64, 0u8);
    for (pa, pb) in a.pixels().iter().zip(b.pixels()) {
        let d = [pa.red().abs_diff(pb.red()), pa.green().abs_diff(pb.green()), pa.blue().abs_diff(pb.blue())];
        let m = *d.iter().max().unwrap();
        if m > 0 {
            any += 1;
        }
        if m > 32 {
            big += 1;
        }
        sum += m as u64;
        max = max.max(m);
    }
    (any, big, sum as f64 / (a.width() * a.height()) as f64, max)
}

/// Tier 2: a screenshot painted pixel for pixel inside a canvas, with labelled regions on top that read
/// back as semantics and take input.
#[test]
fn tier2_raster_with_semantic_overlay() {
    let photo = format!("{}/bench/fidelity/ref/photo.png", ROOT);
    let mx = format!("@shot\nh2 \"A screenshot with semantics\"\nregion 480x320 #shot\n  image @0,0 480x320 {} \"Photo\"\n  button Open @20,20 100x40\n    decoration @0,0 100x40\n  link Credits @380,290 90x24\n    decoration @0,0 90x24\n  region Sky @0,0 480x120\n", photo);
    let mut h = Host::new(Renderer::new());
    assert_eq!(h.render(&mx).unwrap(), "@shot rev=0 nodes=8");
    let l1 = h.observe("shot", 1, None, None).unwrap();
    assert_eq!(l1, format!("@shot rev=0\nh2 A screenshot with semantics\nregion 480x320 #shot [1 shapes, 4 labelled]\n  image {} @0,0 480x320 \"Photo\"\n  button Open @20,20 100x40\n  link Credits @380,290 90x24\n  region Sky @0,0 480x120", photo));
    let boxes = h.entry("shot").unwrap().rendered.as_ref().unwrap().boxes.clone();
    let cv = boxes["shot"];
    let (png, _, _) = h.look("shot", Some("shot")).unwrap();
    let painted = Pixmap::decode_png(&png).unwrap();
    let original = Pixmap::decode_png(&std::fs::read(&photo).unwrap()).unwrap();
    let (any, big, mean, max) = diff_stats(&painted, &original);
    assert_eq!((any, big, max), (0, 0, 0), "tier 2: {} px differ, mean {:.3}", any, mean);
    assert_eq!(h.click("shot", cv.x as f32 + 70.0, cv.y as f32 + 40.0).unwrap(), "@shot action open rev=0 {}");
    assert!(matches!(h.hit("shot", cv.x as f32 + 400.0, cv.y as f32 + 300.0), Some((Hit::Link(id, _), _)) if id == "credits"));
    assert!(h.hit("shot", cv.x as f32 + 240.0, cv.y as f32 + 60.0).is_none(), "a region is not a target");
    std::fs::write(format!("{}/bench/out/rust/tier2.png", ROOT), png).unwrap();
}

/// Tier 3: a figure drawn by an independent rasterizer (PIL, bench/fidelity/vector.py) reproduced with
/// primitives. Differences are anti-aliasing at the edges.
#[test]
fn tier3_vector_reconstruction_within_tolerance() {
    // PIL's coordinates are inclusive (a 200-wide box covers 200.25 px after 4x supersampling) and its
    // outlines are drawn inside the box; the MX says so with quarter-pixel sizes and an inset stroke.
    let mx = "@vec\nregion 400x300 #vec\n  rect @20,20 200.25x100.25 fill:#2f5bea radius:12 \"Blue card\"\n  rect @241,21 138.25x38.25 fill:#e1f4e8 stroke:#156e3b width:2 \"Green box\"\n  circle @300.125,180.125 r:40.125 fill:#c62a2a \"Ring\"\n  circle @300.125,180.125 r:20.125 fill:#ffffff\n  line @20,280 @380,140 stroke:#1b1d21 width:3\n  path \"M 60 160 L 200.25 160 L 130.125 260.25 Z\" fill:#8a5900 \"Triangle\"\n  rect @40,200 8.25x8.25 fill:#0b7285\n";
    let mut h = Host::new(Renderer::new());
    h.render(mx).unwrap();
    let (png, w, hh) = h.look("vec", Some("vec")).unwrap();
    assert_eq!((w, hh), (400, 300));
    let painted = Pixmap::decode_png(&png).unwrap();
    let reference = Pixmap::decode_png(&std::fs::read(format!("{}/bench/fidelity/ref/vector.png", ROOT)).unwrap()).unwrap();
    let (any, big, mean, max) = diff_stats(&painted, &reference);
    let total = (w * hh) as f64;
    eprintln!("tier 3: {} px differ ({:.2}%), {} by more than 32 ({:.2}%), mean {:.3}, max {}", any, 100.0 * any as f64 / total, big, 100.0 * big as f64 / total, mean, max);
    std::fs::write(format!("{}/bench/out/rust/tier3.png", ROOT), png).unwrap();
    std::fs::write(format!("{}/bench/out/rust/tier3.stats.txt", ROOT), format!("differing={} ({:.2}%) big={} ({:.2}%) mean={:.3} max={}\n", any, 100.0 * any as f64 / total, big, 100.0 * big as f64 / total, mean, max)).unwrap();
    assert!(big as f64 / total < 0.02, "{} px differ by more than 32/255", big);
    assert!(mean < 2.0, "mean difference {:.3}", mean);
}

/// Tier 1: a screenshot or photo in an `image` node is painted at natural size, never upscaled, with no
/// rounding, so it comes back pixel for pixel.
#[test]
fn tier1_raster_passthrough() {
    let photo = format!("{}/bench/fidelity/ref/photo.png", ROOT);
    let mut h = Host::new(Renderer::new());
    h.render(&format!("@pass\nh2 \"A photo\"\nimage {} \"Photo\" #shot\ntext Below it", photo)).unwrap();
    let (png, w, hh) = h.look("pass", Some("shot")).unwrap();
    let painted = Pixmap::decode_png(&png).unwrap();
    let original = Pixmap::decode_png(&std::fs::read(&photo).unwrap()).unwrap();
    assert_eq!((w, hh), (original.width(), original.height()));
    let (any, _, _, _) = diff_stats(&painted, &original);
    assert_eq!(any, 0, "tier 1: {} px differ", any);
}
