"""Claude token counts for the A2UI inference formats (Express, Atom, Elemental, corpus/formats/) and for
the hand-written A2UI corpus in v1.0 form (corpus/*.a2ui1.jsonl), next to A2UI v0.9 and MX. Same
tokenizer and overhead handling as count_claude.py. Writes out/results_formats.json and out/tables_formats.md."""
import json, pathlib, sys
from count_claude import tok, NAMES, C, O, _cache, CACHE

MODEL = sys.argv[1] if len(sys.argv) > 1 else "claude-haiku-5-5"
FORMATS = [("a2ui", lambda n: C / f"{n}.a2ui.jsonl"), ("a2ui1", lambda n: C / f"{n}.a2ui1.jsonl"),
           ("express", lambda n: C / "formats" / f"{n}.express"), ("atom", lambda n: C / "formats" / f"{n}.atom"),
           ("elemental", lambda n: C / "formats" / f"{n}.elemental"), ("mx1", lambda n: C / f"{n}.mx")]


def main():
    base = tok("a", MODEL) - 1
    t = lambda s: tok(s, MODEL) - base
    res = {n: {f: t(p(n).read_text()) for f, p in FORMATS} for n in NAMES}
    CACHE.write_text(json.dumps({"\u0000".join(k): v for k, v in _cache.items()}))
    (O / "results_formats.json").write_text(json.dumps({"model": MODEL, "overhead": base, "create": res}, indent=1))
    fm = [f for f, _ in FORMATS]
    out = [f"## Create, inference formats ({MODEL}; {base} tokens of per-message overhead subtracted)\n",
           "| visual | " + " | ".join(fm) + " |", "|---" * (len(fm) + 1) + "|"]
    tot = {f: sum(res[n][f] for n in NAMES) for f in fm}
    for n in NAMES:
        out.append(f"| {n} | " + " | ".join(str(res[n][f]) for f in fm) + " |")
    out.append("| **total** | " + " | ".join(f"**{tot[f]:,}**" for f in fm) + " |")
    out.append("| vs A2UI v1.0 | " + " | ".join(f"{tot[f] / tot['a2ui1']:.2f}×" for f in fm) + " |")
    out.append("| vs MX | " + " | ".join(f"{tot[f] / tot['mx1']:.2f}×" for f in fm) + " |")
    (O / "tables_formats.md").write_text("\n".join(out) + "\n")
    print("\n".join(out))


if __name__ == "__main__":
    main()
