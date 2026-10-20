"""Reader bundles for the pages experiment: the same questions over the imported MX (level 1, from the
engine) and over the pages' screenshots. Writes out/acc/bundle_mx.txt and out/acc/bundle_shots.txt."""
import json, pathlib, subprocess
B = pathlib.Path(__file__).parent
MXR = B.parent.parent / "engine" / "target" / "release" / "mxr"
KEY = {"getbootstrap-com-docs-5-3-examples-checkout": "checkout", "getbootstrap-com-docs-5-3-examples-pricing": "pricing", "preview-tabler-io-invoice-html": "invoice", "ui-shadcn-com-examples-dashboard": "shadcn", "getbootstrap-com-docs-5-3-examples-dashboard": "dashboard", "getbootstrap-com-docs-5-3-examples-sign-in": "signin", "getbootstrap-com-docs-5-3-examples-album": "album", "preview-tabler-io-pricing-html": "tabler"}
Q = json.loads((B / "questions.json").read_text())
(B / "out/acc").mkdir(parents=True, exist_ok=True)
legend = (B.parent / "legend.txt").read_text().strip()
mx, shots = [legend, ""], ["Each page below is a screenshot file. Open it with the Read tool and answer the questions from what you see.", ""]
for slug, qs in Q.items():
    k = KEY[slug]
    l1 = subprocess.run([str(MXR), "observe", str(B / "out" / f"{slug}.mx"), "1"], capture_output=True, text=True).stdout.rstrip()
    mx += [f'=== page "{k}" ===', l1, "--- questions ---"] + [f"{k}.{i} {q}" for i, (q, _) in enumerate(qs, 1)] + [""]
    shots += [f'=== page "{k}" ===', f"screenshot: {B / 'out' / (slug + '.png')}", "--- questions ---"] + [f"{k}.{i} {q}" for i, (q, _) in enumerate(qs, 1)] + [""]
(B / "out/acc/bundle_mx.txt").write_text("\n".join(mx))
(B / "out/acc/bundle_shots.txt").write_text("\n".join(shots))
print("wrote bundles;", len(Q), "pages,", sum(len(q) for q in Q.values()), "questions")
