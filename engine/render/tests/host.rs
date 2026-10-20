//! The host without a window: lifecycle, events from pointer and keys, since= diffs, look.
use mx_render::host::Host;
use mx_render::{Hit, Renderer, Theme};

fn host() -> Host {
    Host::new(Renderer::new())
}

#[test]
fn render_observe_patch_and_since() {
    let mut h = host();
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let dash = std::fs::read_to_string(format!("{}/bench/corpus/dash.mx", root)).unwrap();
    assert_eq!(h.render(&dash).unwrap(), "@dash rev=0 nodes=9");
    let l2 = h.observe("dash", 2, None, None).unwrap();
    assert!(l2.starts_with("@dash rev=0 @0,0,728,"), "{}", l2);
    assert_eq!(h.patch("dash", "~1\n*revenue $315k +16%\n*bar.Sep 80\n*export enabled").unwrap(), "@dash rev=1 nodes=9");
    assert_eq!(h.observe("dash", 1, Some(0), None).unwrap(), "~1\n*revenue $315k +16%\n*bar.Sep 80\n*export \"Export CSV\" enabled");
    assert_eq!(h.observe("dash", 1, Some(1), None).unwrap(), "~1");
    assert_eq!(h.patch("dash", "~1\n*export disabled").unwrap_err().to_string(), "stale rev 1 (surface at 1)");
    assert_eq!(h.render("@x\nh1 Hi\nbutton Go primay").unwrap_err().to_string(), "line=3 unknown word \"primay\" on button line");
    let (png, w, hh) = h.look("dash", Some("bar")).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    assert!(w > 600 && hh > 150, "{}x{}", w, hh);
    assert_eq!(h.list(), "@dash rev=1 nodes=9");
}

#[test]
fn pointer_and_keys_make_events() {
    let mut h = host();
    h.render("@f until:action\nh1 Sign up\nfield Email email\ntoggle Updates off\nslider Volume 40 0..100\nselect Plan Free *Pro Team\ntabs\n  tab *A\n    text a\n  tab B\n    text b\nbutton Send primary").unwrap();
    let boxes = h.entry("f").unwrap().rendered.as_ref().unwrap().boxes.clone();
    let center = |id: &str| { let b = boxes[id]; (b.x as f32 + b.w as f32 / 2.0, b.y as f32 + b.h as f32 / 2.0) };
    let (x, y) = center("updates");
    assert!(matches!(h.hit("f", x, y), Some((Hit::Toggle(_), _))));
    assert_eq!(h.click("f", x, y).unwrap(), "@f input updates on rev=1");
    let (x, y) = center("plan");
    // the select's control sits below its label
    let b = boxes["plan"];
    assert_eq!(h.click("f", x, b.y as f32 + b.h as f32 - 10.0).unwrap(), "@f input plan *Team rev=2");
    let (x, y) = center("email");
    let b = boxes["email"];
    assert!(h.click("f", x, b.y as f32 + b.h as f32 - 10.0).is_none());
    assert_eq!(h.editing("f"), Some("email"));
    for c in "a@b.c".chars() { h.key("f", &c.to_string()); }
    h.key("f", "\u{8}");
    h.key("f", "o");
    assert_eq!(h.key("f", "\n").unwrap(), "@f input email =\"a@b.o\" rev=3");
    let b = boxes["volume"];
    assert_eq!(h.click("f", b.x as f32 + b.w as f32 * 0.75, b.y as f32 + b.h as f32 - 8.0).unwrap(), "@f input volume 75 rev=4");
    let (x, y) = center("b");
    assert_eq!(h.click("f", x, y).unwrap(), "@f input tabs \"B\" rev=5");
    assert!(h.observe("f", 1, None, None).unwrap().contains("tab *B\n"));
    let (x, y) = center("send");
    let ev = h.click("f", x, y).unwrap();
    assert_eq!(ev, "@f action send rev=5 {\"email\":\"a@b.o\",\"updates\":true,\"volume\":75.0,\"plan\":\"Team\"}");
    let evs = h.drain();
    assert_eq!(evs.last().unwrap(), "@f dissolved reason=until:action rev=5");
    assert_eq!(evs.len(), 7);
    assert_eq!(h.observe("f", 1, None, None).unwrap(), "@f dissolved rev=5");
    let _ = y;
}

#[test]
fn lifecycle_ttl_seen_ephemeral_and_residue() {
    let mut h = host();
    h.tick(1_000);
    h.render("@t ttl:2s residue:line\nh1 Saved\ntext Your changes are saved").unwrap();
    h.render("@e ephemeral\ntext Thinking").unwrap();
    h.render("@s seen:1s residue:archive\nh1 Tip\ntext Press ? for help").unwrap();
    h.tick(2_000);
    assert_eq!(h.list(), "@t ttl:2s residue:line rev=0 nodes=2\n@e ephemeral rev=0 nodes=1\n@s seen:1s residue:archive rev=0 nodes=2");
    h.tick(3_100);
    assert_eq!(h.drain(), vec!["@s dissolved reason=seen:1s rev=0", "@t dissolved reason=ttl:2s rev=0"]);
    h.turn();
    assert_eq!(h.drain(), vec!["@e dissolved reason=ephemeral rev=0"]);
    // an archived surface stays listed; a line residue is its one line
    assert_eq!(h.list(), "@t dissolved: Saved\n@s seen:1s residue:archive rev=0 nodes=2");
    assert_eq!(h.observe("t", 1, None, None).unwrap(), "@t dissolved: Saved");
    assert_eq!(h.observe("s", 1, None, None).unwrap(), "@s seen:1s residue:archive rev=0 archived\nh1 Tip\ntext Press ? for help");
    assert!(h.patch("s", "~1\n*tip Hint").unwrap_err().to_string().contains("has dissolved"));
}

#[test]
fn dark_palette_keeps_contrast() {
    let mut r = Renderer::new();
    r.theme = Theme::dark();
    let mut h = Host::new(r);
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    for n in ["dash", "form", "table", "flow", "confirm", "settings"] {
        let mx = std::fs::read_to_string(format!("{}/bench/corpus/{}.mx", root, n)).unwrap();
        h.render(&mx).unwrap();
        let diags = &h.entry(n).unwrap().rendered.as_ref().unwrap().diags;
        assert!(!diags.iter().any(|d| matches!(d, mx::Diag::Contrast { .. })), "{}: {:?}", n, diags);
    }
}

#[test]
fn compose_stacks_surfaces_and_residues_at_device_scale() {
    let mut h = host();
    h.renderer.scale = 2.0;
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    h.render(&std::fs::read_to_string(format!("{}/bench/corpus/dash.mx", root)).unwrap()).unwrap();
    h.render("@toast ttl:1s residue:line\nh1 Saved\ntext Your changes are saved").unwrap();
    h.render(&std::fs::read_to_string(format!("{}/bench/corpus/form.mx", root)).unwrap()).unwrap();
    h.tick(2_000);
    let (pm, placed) = h.compose(1520, 1800, 2.0, 0.0);
    assert_eq!(pm.width(), 1520);
    assert_eq!(placed.iter().map(|p| (p.id.as_str(), p.live)).collect::<Vec<_>>(), vec![("dash", true), ("toast", false), ("form", true)]);
    assert_eq!(placed[0].y, 16.0);
    assert_eq!(placed[0].w, 728.0);
    assert_eq!(placed[1].y, placed[0].y + placed[0].h + 16.0);
    // a click on the form's Book button, in screen coordinates
    let (i, rx, ry) = mx_render::host::Host::under(&placed, 760.0, 100.0, placed[2].y + placed[2].h - 30.0).unwrap();
    assert_eq!(placed[i].id, "form");
    assert!(matches!(h.hit("form", rx, ry), Some((Hit::Button(b), _)) if b == "book"), "{:?}", h.hit("form", rx, ry));
    std::fs::create_dir_all(format!("{}/bench/out/rust", root)).unwrap();
    pm.save_png(format!("{}/bench/out/rust/window.png", root)).unwrap();
}

#[test]
fn each_instances_and_tweens() {
    let mut h = host();
    h.tick(1_000);
    h.render("@live\nseries sales Apr:38 May:42 Jun:51 anim:400ms\neach =sales row\n  metric =key =value\nline Sales =sales\nregion 200x100 #stage\n  circle @20,50 r:10 fill:primary anim:400ms \"Dot\"").unwrap();
    let boxes = h.entry("live").unwrap().rendered.as_ref().unwrap().boxes.clone();
    // one metric per element, named by the element's key, side by side
    assert!(boxes.contains_key("metric.Apr") && boxes.contains_key("metric.Jun"), "{:?}", boxes.keys().collect::<Vec<_>>());
    assert_eq!(boxes["metric.Apr"].y, boxes["metric.Jun"].y);
    assert!(boxes["metric.Jun"].x > boxes["metric.Apr"].x);
    let before = h.pixmap("live").unwrap().clone();
    h.patch("live", "~1\n*sales.Jun 90\n*dot @150,50").unwrap();
    assert!(h.animating());
    // mid-tween the pixels are neither the old nor the final frame; the boxes and perception are final
    h.tick(1_200);
    let mid = h.pixmap("live").unwrap().clone();
    h.tick(1_500);
    assert!(!h.animating());
    let after = h.pixmap("live").unwrap().clone();
    assert_ne!(before.data(), mid.data());
    assert_ne!(mid.data(), after.data());
    let b2 = h.entry("live").unwrap().rendered.as_ref().unwrap().boxes.clone();
    assert_eq!(b2["dot"].x - b2["stage"].x, 140);
    assert!(h.observe("live", 1, None, None).unwrap().contains("min:38 max:90 last:90"));
}
