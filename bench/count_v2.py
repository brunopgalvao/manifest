"""Claude-tokenizer counts for the six paint-heavy visuals: MX v2, the HTML/SVG twin (with ARIA roles and
labels), and A2UI v1.0 over a custom paint catalog. Writes out/tables_v2.md and out/results_v2.json."""
import json, pathlib, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import count_claude
from count_claude import tok
B = pathlib.Path(__file__).parent
MODEL = "claude-opus-5-5"
NAMES = ["funnel", "gantt", "map", "kpi", "live", "diagram"]
rows, tot = [], [0, 0, 0]
for n in NAMES:
    mx = (B / "corpus" / f"{n}.mx").read_text()
    html = (B / "corpus/v2html" / f"{n}.html").read_text()
    a2 = (B / "corpus/v2a2ui" / f"{n}.jsonl").read_text()
    t = [tok(mx, MODEL), tok(html, MODEL), tok(a2, MODEL)]
    for i in range(3): tot[i] += t[i]
    rows.append(f"| {n} | {t[0]:,} | {t[1]:,} | {t[2]:,} |")
out = ["| visual | MX v2 | HTML + SVG + ARIA | A2UI v1.0, paint catalog |", "|---|---|---|---|", *rows, f"| **six visuals** | **{tot[0]:,}** | **{tot[1]:,} ({tot[1]/tot[0]:.1f}×)** | **{tot[2]:,} ({tot[2]/tot[0]:.1f}×)** |"]
(B / "out/tables_v2.md").write_text("\n".join(out) + "\n")
json.dump({"model": MODEL, "mx": tot[0], "html": tot[1], "a2ui": tot[2]}, (B / "out/results_v2.json").open("w"))
count_claude.CACHE.write_text(json.dumps({"\u0000".join(k): v for k, v in count_claude._cache.items()}))
print("\n".join(out))
