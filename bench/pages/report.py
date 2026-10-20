"""Summarise a pages run: how many imported and parsed, text kept, semantic share, tokens of the HTML
against the MX (Claude's tokenizer, cached), and the round trip of heights. Writes out/report.md and
out/report.json next to index.jsonl.

    .venv/bin/python bench/pages/report.py bench/pages/out
"""
import json, pathlib, statistics, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent.parent))
import count_claude
from count_claude import tok, strip_harness

MODEL = "claude-opus-5-5"
out = pathlib.Path(sys.argv[1])
recs = [json.loads(l) for l in (out / "index.jsonl").read_text().splitlines() if l.strip()]
ok = [r for r in recs if r.get("parse") == "ok"]
errors = [r for r in recs if "error" in r]
bad_parse = [r for r in recs if "parse" in r and r["parse"] != "ok"]
rows, ratios, text_pct, sem_share, heights = [], [], [], [], []
tot_html = tot_mx = 0
for r in ok:
    html = (out / f"{r['slug']}.html").read_text(errors="replace")
    mx = (out / f"{r['slug']}.mx").read_text()
    th, tm = tok(strip_harness(html), MODEL), tok(mx, MODEL)
    r["tokens"] = {"html": th, "mx": tm}
    tot_html += th; tot_mx += tm
    ratios.append(th / max(tm, 1))
    t = r["text"]; text_pct.append(100 * t["hit"] / max(t["total"], 1))
    s = r["stats"]; sem_share.append(100 * s["semantic"] / max(s["semantic"] + s["painted"], 1))
    rt = r.get("roundtrip")
    if isinstance(rt, dict) and rt["total"]:
        heights.append(100 * rt["heights"] / rt["total"])
    rows.append((r["source"], r["slug"], th, tm, text_pct[-1], s["semantic"], s["painted"], s["demoted"], r["nodes"]))
count_claude.CACHE.write_text(json.dumps(count_claude._cache))
med = lambda xs: statistics.median(xs) if xs else 0
summary = {
    "pages": len(recs), "imported": len(recs) - len(errors), "parsed": len(ok), "errors": [r["url"] + ": " + r["error"] for r in errors],
    "bad_parse": [r["url"] + ": " + r["parse"] for r in bad_parse],
    "tokens_html": tot_html, "tokens_mx": tot_mx, "ratio_total": round(tot_html / max(tot_mx, 1), 2), "ratio_median": round(med(ratios), 2),
    "text_kept_median_pct": round(med(text_pct), 1), "text_kept_min_pct": round(min(text_pct), 1) if text_pct else 0,
    "semantic_share_median_pct": round(med(sem_share), 1), "heights_within_tolerance_median_pct": round(med(heights), 1),
}
(out / "report.json").write_text(json.dumps(summary, indent=1))
md = ["| source | page | HTML tokens | MX tokens | text kept | semantic | painted | demoted | nodes |", "|---|---|---|---|---|---|---|---|---|"]
for s, slug, th, tm, tp, se, pa, de, n in sorted(rows):
    md.append(f"| {s} | {slug} | {th:,} | {tm:,} | {tp:.0f}% | {se} | {pa} | {de} | {n} |")
md.append(f"| **{len(ok)} pages** | | **{tot_html:,}** | **{tot_mx:,}** | median {summary['text_kept_median_pct']}% | | | | |")
(out / "report.md").write_text("\n".join(md) + "\n")
print(json.dumps(summary, indent=1))
