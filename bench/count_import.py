"""Token cost of a page before and after import: the HTML source (harness attributes stripped), the
accessibility snapshot a browser agent gets, and the imported MX. Counted with Claude's tokenizer
(count_tokens). Reads bench/out/import/<name>.mx written by packages/import. Writes out/tables_import.md."""
import json, pathlib, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import count_claude
from count_claude import tok, strip_harness, NAMES
B = pathlib.Path(__file__).parent
MODEL = "claude-opus-5-5"
rows = []
tot = [0, 0, 0]
for n in NAMES:
    html = strip_harness((B / "corpus" / f"{n}.html").read_text())
    aria = (B / "out" / "aria" / f"{n}.txt").read_text() if (B / "out" / "aria" / f"{n}.txt").exists() else ""
    mx = (B / "out" / "import" / f"{n}.mx").read_text()
    t = [tok(html, MODEL), tok(aria, MODEL), tok(mx, MODEL)]
    for i in range(3): tot[i] += t[i]
    rows.append(f"| {n} | {t[0]:,} | {t[1]:,} | {t[2]:,} | {t[0] / t[2]:.1f}× |")
out = ["| page | HTML source | accessibility snapshot | imported MX | HTML / MX |", "|---|---|---|---|---|", *rows, f"| **total** | **{tot[0]:,}** | **{tot[1]:,}** | **{tot[2]:,}** | **{tot[0] / tot[2]:.1f}×** |"]
(B / "out" / "tables_import.md").write_text("\n".join(out) + "\n")
json.dump({"model": MODEL, "html": tot[0], "aria": tot[1], "mx": tot[2]}, (B / "out" / "results_import.json").open("w"))
count_claude.CACHE.write_text(json.dumps(count_claude._cache))
print("\n".join(out))
