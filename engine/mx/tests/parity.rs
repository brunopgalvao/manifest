//! Parity with the recorded fixtures (tests/fixtures.json): perception at every level for the corpus and the
//! tree after each scripted edit, written by the original Python reference and kept as ground truth.
use mx::{parse, perceive, Box, Diag};
use mx::perceive::PerceiveOptions;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Fixture {
    mx: String,
    boxes: BTreeMap<String, Box>,
    diags: Vec<Diag>,
    ids: Vec<String>,
    #[serde(rename = "L0")]
    l0: String,
    #[serde(rename = "L1")]
    l1: String,
    #[serde(rename = "L2")]
    l2: String,
}

#[derive(Deserialize)]
struct Fixtures {
    corpus: BTreeMap<String, Fixture>,
}

fn fixtures() -> Fixtures {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures.json");
    serde_json::from_str(&std::fs::read_to_string(p).expect("fixtures.json")).expect("parse fixtures")
}

#[test]
fn corpus_ids_and_perception_match_the_fixtures() {
    let fx = fixtures();
    for (name, f) in &fx.corpus {
        let s = parse(&f.mx).unwrap_or_else(|e| panic!("{}: {}", name, e));
        let mut ids: Vec<String> = s.ids.iter().map(|(id, _)| id.clone()).collect();
        ids.sort();
        assert_eq!(ids, f.ids, "{}: ids", name);
        let l0 = perceive(&s, 0, PerceiveOptions { diags: &f.diags, ..Default::default() }).unwrap();
        assert_eq!(l0, f.l0, "{}: L0", name);
        let l1 = perceive(&s, 1, PerceiveOptions { diags: &f.diags, ..Default::default() }).unwrap();
        assert_eq!(l1, f.l1, "{}: L1", name);
        let l2 = perceive(&s, 2, PerceiveOptions { boxes: Some(&f.boxes), diags: &f.diags, ..Default::default() }).unwrap();
        assert_eq!(l2, f.l2, "{}: L2", name);
    }
}

#[test]
fn errors_name_the_line() {
    let e = parse("@x\nh1 Hi\n  button").unwrap_err();
    assert_eq!(e.line, Some(3));
    assert_eq!(e.message, "button needs a label");
    let e = parse("@x\nimage a.png").unwrap_err();
    assert_eq!(e.to_string(), "line=2 image needs alt text");
    let e = parse("h1 Hi").unwrap_err();
    assert_eq!(e.to_string(), "line=1 document needs an @surface line");
    let e = parse("@x\nrow left").unwrap_err();
    assert_eq!(e.message, "unknown flag(s) [\"left\"] on row");
}

#[test]
fn focus_and_hidden_tabs() {
    let s = parse("@s\nh1 Settings\ntabs\n  tab General\n    toggle \"Dark mode\" off\n    select Language English *French\n  tab *Notifications\n    toggle Push on").unwrap();
    let l1 = perceive(&s, 1, PerceiveOptions::default()).unwrap();
    assert!(l1.contains("tab General [hidden: Dark mode, Language]"), "{}", l1);
    let f = perceive(&s, 1, PerceiveOptions { focus: Some("push"), ..Default::default() }).unwrap();
    assert_eq!(f, "@s rev=0\ntabs\n  tab *Notifications\n    toggle Push on");
    assert!(perceive(&s, 1, PerceiveOptions { focus: Some("nope"), ..Default::default() }).is_err());
}

#[derive(Deserialize)]
struct Step {
    patch: String,
    #[serde(rename = "L1")]
    l1: String,
}

#[derive(Deserialize)]
struct Pricing {
    mx: String,
    ids: Vec<String>,
    #[serde(rename = "L1")]
    l1: String,
    patch: String,
    after: String,
}

#[derive(Deserialize)]
struct EditFixtures {
    edits: BTreeMap<String, Vec<Step>>,
    pricing: Pricing,
}

#[test]
fn patches_match_the_fixtures_and_diffs_round_trip() {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures.json");
    let text = std::fs::read_to_string(p).unwrap();
    let fx: EditFixtures = serde_json::from_str(&text).unwrap();
    let corpus = fixtures();
    let mut applied = 0;
    for (name, steps) in &fx.edits {
        let mut s = parse(&corpus.corpus[name].mx).unwrap();
        for st in steps {
            let next = mx::apply(&s, &st.patch, None).unwrap_or_else(|e| panic!("{}: {}\n{}", name, e, st.patch));
            let l1 = perceive(&next, 1, PerceiveOptions::default()).unwrap();
            assert_eq!(l1, st.l1, "{}: after {:?}", name, st.patch);
            // the diff from the previous state re-applies to the same tree
            if let Some(d) = mx::diff(&s, &next) {
                let again = mx::apply(&s, &d, None).unwrap_or_else(|e| panic!("{}: diff {}\n{}", name, e, d));
                assert_eq!(perceive(&again, 1, PerceiveOptions::default()).unwrap(), st.l1, "{}: diff round trip\n{}", name, d);
            }
            s = next;
            applied += 1;
        }
    }
    assert!(applied >= 17, "applied {}", applied);
    let s = parse(&fx.pricing.mx).unwrap();
    let mut ids: Vec<String> = s.ids.iter().map(|(id, _)| id.clone()).collect();
    ids.sort();
    assert_eq!(ids, fx.pricing.ids);
    assert_eq!(perceive(&s, 1, PerceiveOptions::default()).unwrap(), fx.pricing.l1);
    let after = mx::apply(&s, &fx.pricing.patch, None).unwrap();
    assert_eq!(perceive(&after, 1, PerceiveOptions::default()).unwrap(), fx.pricing.after);
    assert_eq!(after.rev, 1);
}

#[test]
fn patch_errors_and_data_model() {
    let s = parse("@f\nh1 Sign up\nfield Email email required\ntoggle Updates on\nslider Volume 40 0..100\nbutton Send primary").unwrap();
    assert_eq!(mx::data_model(&s).to_string(), r#"{"email":"","updates":true,"volume":40.0}"#);
    assert_eq!(mx::apply(&s, "~2\n*send disabled", None).unwrap_err().to_string(), "stale rev 2 (surface at 0)");
    assert_eq!(mx::apply(&s, "~1\n*nope x", None).unwrap_err().to_string(), "line=2 no node \"nope\"");
    assert_eq!(mx::apply(&s, "~g 1\n*send disabled", Some("f")).unwrap_err().message, "patch names surface \"g\", call names \"f\"");
    let t = mx::apply(&s, "~1\n*send disabled\n*email =\"a@b.c\"\n-updates\n+volume text After", None).unwrap();
    assert_eq!(perceive(&t, 1, PerceiveOptions::default()).unwrap(), "@f rev=1\nh1 Sign up\nfield Email email =\"a@b.c\" required\nslider Volume 40 0..100\ntext After\nbutton Send primary disabled");
    assert_eq!(mx::diff(&s, &t).unwrap(), "~1\n*email =\"a@b.c\"\n+volume text After\n*send Send primary disabled\n-updates");
    assert_eq!(mx::residue_line(&s), "@f dissolved: Sign up");
}
