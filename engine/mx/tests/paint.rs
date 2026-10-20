//! Geometry in one tree: sized nodes are coordinate spaces, positioned nodes sit in them, primitives paint,
//! a node with paint children shows them instead of its default look; series buffers; each; anim.
use mx::perceive::PerceiveOptions;
use mx::{parse, perceive};

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
    text @50,21 "Export PNG" fill:white anchor:middle
  button Reset @420,280
row end
  button Clear
"#;

#[test]
fn funnel_parses_with_derived_ids_and_levels() {
    let s = parse(FUNNEL).unwrap();
    let mut ids: Vec<&str> = s.ids.iter().map(|(i, _)| i.as_str()).collect();
    ids.sort();
    assert_eq!(ids, ["77", "bg", "clear", "export", "export.export", "funnel", "h2", "leads", "marker", "path", "pipe", "qualified", "rect", "reset", "root", "row", "won"]);
    let l1 = perceive(&s, 1, PerceiveOptions::default()).unwrap();
    assert_eq!(l1, r#"@chart-editor rev=0
h2 Q3 pipeline
region 640x320 #pipe [9 shapes, 9 labelled]
  region Funnel @40,20
    rect @0,0 560x48 fill:primary radius:6 "Leads 1,240" #leads
    rect @40,64 480x48 fill:primary-70 radius:6 "Qualified 610" #qualified
    rect @120,128 320x48 fill:success radius:6 "Won 95" #won
    text @280,210 anchor:middle size:14 "7.7% conversion"
  circle @300,200 r:4 fill:danger "Target" #marker
  button "Export PNG" @520,280 100x32
    text @50,21 anchor:middle fill:white "Export PNG"
  button Reset @420,280
row end
  button Clear"#);
    let l3 = perceive(&s, 3, PerceiveOptions::default()).unwrap();
    assert!(l3.contains("\n  rect @0,0 640x320 fill:muted-50 #bg\n"), "{}", l3);
    assert!(l3.contains("\n  path \"M 300 180 L 300 200\" stroke:muted dashed\n"), "{}", l3);
    assert!(l3.contains("\n    rect @0,0 100x32 fill:primary radius:16\n"), "{}", l3);
}

#[test]
fn nodes_are_patched_by_geometry_style_and_their_own_words() {
    let s = parse(FUNNEL).unwrap();
    let t = mx::apply(&s, "~1\n*won 340x48 fill:warning\n*marker @300,180\n-bg\n+funnel/ rect @200,192 160x48 fill:warning \"Stalled 40\"\n*leads \"Leads 1,300\"\n*export @500,280 disabled\n*reset Restart primary", None).unwrap();
    let l1 = perceive(&t, 1, PerceiveOptions::default()).unwrap();
    assert!(l1.contains("rect @120,128 340x48 fill:warning radius:6 \"Won 95\" #won"), "{}", l1);
    assert!(l1.contains("circle @300,180 r:4 fill:danger \"Target\" #marker"), "{}", l1);
    assert!(l1.contains("    rect @200,192 160x48 fill:warning \"Stalled 40\"\n"), "{}", l1);
    assert!(l1.contains("button \"Export PNG\" @500,280 100x32 disabled"), "{}", l1);
    assert!(l1.contains("button Restart @420,280 primary #reset"), "{}", l1);
    assert!(t.get("bg").is_none() && t.get("stalled").is_some());
    let d = mx::diff(&s, &t).unwrap();
    let again = mx::apply(&s, &d, None).unwrap();
    assert_eq!(perceive(&again, 3, PerceiveOptions::default()).unwrap(), perceive(&t, 3, PerceiveOptions::default()).unwrap(), "{}", d);
}

#[test]
fn errors_name_the_problem() {
    let e = |src: &str| parse(src).unwrap_err().to_string();
    assert_eq!(e("@x\nrect @0,0 10x10"), "line=2 positioned rect needs a sized parent");
    assert_eq!(e("@x\nbutton Go @0,0"), "line=2 positioned button needs a sized parent");
    assert_eq!(e("@x\nregion 100x100\n  rect 10x10"), "line=3 rect needs a position @x,y");
    assert_eq!(e("@x\nregion 100x100\n  button Go"), "line=3 button inside a sized node needs a position @x,y");
    assert_eq!(e("@x\nregion 100x100\n  button @0,0"), "line=3 button needs a label");
    assert_eq!(e("@x\nregion 100x100\n  rect @0,0 10x10 colour:red"), "line=3 unknown style word \"colour:red\" on rect");
    assert_eq!(e("@x\ncanvas 100x100"), "line=2 canvas is gone: give any node a size (region \"Label\" 640x320) and position its children with @x,y");
    assert_eq!(e("@x\nregion 100x100\n  rect @0,0 10x10\n    text @1,1 \"in a rect\""), "line=4 rect cannot have children");
}

#[test]
fn series_buffers_each_and_anim() {
    let s = parse("@live\nseries sales Apr:38 May:42 Jun:51 anim:300ms\nline \"Revenue\" =sales\neach =sales row\n  metric =key =value\nbadge =sales.max success\nregion 100x100\n  circle @10,10 r:4 fill:danger anim:200ms \"Dot\"").unwrap();
    assert_eq!(perceive(&s, 1, PerceiveOptions::default()).unwrap(), "@live rev=0\nseries sales [3 points] min:38 max:51 last:51 anim:300ms\nline Revenue =sales\neach =sales row [3 instances]\n  metric =key =value\nbadge =sales.max success\nregion 100x100 [1 shapes, 1 labelled]\n  circle @10,10 r:4 anim:200ms fill:danger \"Dot\"");
    let t = mx::apply(&s, "~1\n+sales Jul:47\n*sales.Jun 55\n-sales.Apr", None).unwrap();
    let l1 = perceive(&t, 1, PerceiveOptions::default()).unwrap();
    assert!(l1.contains("series sales [3 points] min:42 max:55 last:47") && l1.contains("[3 instances]"), "{}", l1);
    assert_eq!(parse("@x\neach row").unwrap_err().to_string(), "line=2 each needs =buffer");
}

#[test]
fn v1_lines_keep_their_meaning() {
    // a size word in a paragraph is text, a size on a card makes it a space
    let s = parse("@x\ntext The grid is 3x3 today\ncard \"Map\" 200x100\n  circle @50,50 r:10 fill:primary \"You\"").unwrap();
    let l1 = perceive(&s, 1, PerceiveOptions::default()).unwrap();
    assert_eq!(l1, "@x rev=0\ntext The grid is 3x3 today\ncard Map 200x100 [1 shapes, 1 labelled]\n  circle @50,50 r:10 fill:primary \"You\"");
}
