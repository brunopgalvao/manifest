"""Real Claude token counts for every benchmark artifact, via the Messages count_tokens endpoint.
Proxy tokenizers (count.py) undercount Claude by 15-20% on prose and more on code, so this is the
authoritative table. Needs ANTHROPIC_API_KEY. Writes out/results_claude.json and out/tables_claude.md."""
import json, pathlib, re, sys, time
from anthropic import Anthropic
from edits import EDITS

B = pathlib.Path(__file__).parent
C, O = B / "corpus", B / "out"
MODELS = sys.argv[1:] or ["claude-opus-5-5", "claude-haiku-5-5"]
NAMES = ["dash", "form", "table", "flow", "confirm", "settings"]
client = Anthropic()
CACHE = O / "claude_tokens_cache.json"
_cache = {tuple(k.split("\u0000", 1)): v for k, v in json.loads(CACHE.read_text()).items()} if CACHE.exists() else {}


def tok(text, model):
    if not text.strip():
        return 0
    key = (model, text)
    if key not in _cache:
        for attempt in range(5):
            try:
                _cache[key] = client.messages.count_tokens(model=model, messages=[{"role": "user", "content": text}]).input_tokens
                break
            except Exception as e:  # rate limits
                if attempt == 4: raise
                time.sleep(2 * (attempt + 1))
    return _cache[key]


def strip_harness(html):
    return re.sub(r' data-mx="[^"]*"', '', html)


def main():
    res = {}
    for model in MODELS:
        base = tok("a", model) - 1  # the fixed overhead of a message, measured on a one-token message
        r = {"overhead": base, "create": {}, "update": {}, "perceive": {}}
        t = lambda s: (tok(s, model) - base) if s.strip() else 0
        for n in NAMES:
            r["create"][n] = {"html": t(strip_harness((C / f"{n}.html").read_text())),
                              "a2ui": t((C / f"{n}.a2ui.jsonl").read_text()),
                              "mx0": t((C / f"{n}.mx").read_text()), "mx1": t((C / f"{n}.mx").read_text())}
            ups = []
            for e in EDITS[n]:
                u = {"what": e["what"], "html_regen": r["create"][n]["html"],
                     "html_replace": None if e.get("html_regen_only") else sum(t(a) + t(b) for a, b in e["html_replace"]),
                     "a2ui": None if e["a2ui"] is None else t(e["a2ui"]), "mx0": t(e["mx0"]), "mx1": t(e["mx1"])}
                ups.append(u)
            r["update"][n] = ups
            p = {"aria": t((O / "aria" / f"{n}.txt").read_text())}
            for lvl in (0, 1, 2):
                p[f"mx1_L{lvl}"] = t((O / "perceive" / f"{n}.mx.L{lvl}.txt").read_text().rstrip())
            for lvl in (1, 2):
                p[f"native_L{lvl}"] = t((O / "native" / f"{n}.native.L{lvl}.txt").read_text().rstrip())
            r["perceive"][n] = p
            print(model, n, r["create"][n], file=sys.stderr)
        res[model] = r
    CACHE.write_text(json.dumps({"\u0000".join(k): v for k, v in _cache.items()}))
    (O / "results_claude.json").write_text(json.dumps(res, indent=1))
    (O / "tables_claude.md").write_text(tables(res))
    print(tables(res))


def tables(res):
    out = []
    for model, r in res.items():
        out.append(f"## {model} (count_tokens; {r['overhead']} tokens of per-message overhead subtracted)\n")
        fm = ["html", "a2ui", "mx0", "mx1"]
        out.append("### Create\n\n| visual | " + " | ".join(fm) + " |\n" + "|---" * (len(fm) + 1) + "|")
        tot = {f: 0 for f in fm}
        for n, c in r["create"].items():
            out.append(f"| {n} | " + " | ".join(str(c[f]) for f in fm) + " |")
            for f in fm: tot[f] += c[f]
        out.append("| **total** | " + " | ".join(f"**{tot[f]}**" for f in fm) + " |")
        out.append("| vs A2UI | " + " | ".join(f"{tot[f] / tot['a2ui']:.2f}×" for f in fm) + " |\n")
        uf = ["html_regen", "html_replace", "a2ui", "mx0", "mx1"]
        both = {f: 0 for f in uf}
        for ups in r["update"].values():
            for u in ups:
                if all(u[f] is not None for f in uf):
                    for f in uf: both[f] += u[f]
        out.append("### Update (16 edits every format can express)\n\n| " + " | ".join(uf) + " |\n" + "|---" * len(uf) + "|")
        out.append("| " + " | ".join(str(both[f]) for f in uf) + " |\n")
        pf = ["aria", "mx1_L0", "mx1_L1", "mx1_L2", "native_L1", "native_L2"]
        out.append("### Perceive\n\n| visual | " + " | ".join(pf) + " |\n" + "|---" * (len(pf) + 1) + "|")
        pt = {f: 0 for f in pf}
        for n, p in r["perceive"].items():
            out.append(f"| {n} | " + " | ".join(str(p[f]) for f in pf) + " |")
            for f in pf: pt[f] += p[f]
        out.append("| **total** | " + " | ".join(f"**{pt[f]}**" for f in pf) + " |\n")
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    main()
