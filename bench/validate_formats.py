"""Compile the Express, Atom and Elemental versions of the corpus (corpus/formats/) with the A2UI
agent SDK's own compilers, against a v1.0 catalog that is the basic catalog plus the benchmark's custom
components (Metric, BarChart, Table, Canvas, Box, Arrow). Prints what compiled, what failed and why, and
writes the compiled JSON to out/formats/<name>.<format>.json for inspection.

Run with the SDK installed:  <venv>/bin/python -I validate_formats.py <path to a2ui repo checkout>
"""
import copy, json, pathlib, sys, traceback

B = pathlib.Path(__file__).resolve().parent
A2UI = pathlib.Path(sys.argv[1]).resolve()
OUT = B / "out" / "formats"
OUT.mkdir(parents=True, exist_ok=True)
NAMES = ["dash", "form", "table", "flow", "confirm", "settings"]
CUSTOM_ID = "https://manifest.dev/bench/catalog.json"

from a2ui.core import Catalog
from a2ui.inference_formats.express.compiler import ExpressCompiler
from a2ui.inference_formats.experimental.atom.compiler import AtomCompiler
from a2ui.inference_formats.experimental.elemental.compiler import ElementalCompiler
from a2ui.inference_formats import to_message_dicts

S = lambda d="": {"type": "string", "description": d}
N = lambda d="": {"type": "number", "description": d}
DS = {"$ref": "common_types.json#/$defs/DynamicString"}
CUSTOM = {
    "Metric": {"label": DS, "value": DS, "delta": DS, "tone": {"type": "string", "enum": ["positive", "negative", "neutral"]}},
    "BarChart": {"description": DS, "labels": {"type": "array", "items": {"type": "string"}}, "values": {"type": "array", "items": {"type": "number"}}, "highlight": N()},
    "Table": {"columns": {"type": "array", "items": {"type": "object", "properties": {"key": S(), "label": S(), "as": S()}, "required": ["key", "label"]}},
              "rows": {"$ref": "common_types.json#/$defs/DynamicList"}},
    "Canvas": {"width": N(), "height": N(), "description": DS, "children": {"$ref": "common_types.json#/$defs/ChildList"}},
    "Box": {"x": N(), "y": N(), "w": N(), "h": N(), "label": DS, "tone": {"type": "string", "enum": ["accent", "positive", "negative", "neutral"]}},
    "Arrow": {"from": S(), "to": S(), "label": DS, "route": {"type": "string", "enum": ["above", "below"]}, "dashed": {"type": "boolean"}},
}
REQUIRED = {"Metric": ["label", "value"], "BarChart": ["labels", "values"], "Table": ["columns", "rows"], "Canvas": ["children"],
            "Box": ["x", "y", "w", "h", "label"], "Arrow": ["from", "to"]}


def catalog():
    base = json.loads((A2UI / "catalogs/basic/v1/catalog.json").read_text())
    cat = copy.deepcopy(base)
    cat["$id"] = cat["catalogId"] = CUSTOM_ID
    cat["title"] = "Manifest benchmark catalog"
    # The hand-written corpus uses heading variants on Text, as the v0.9 basic catalog allowed; v1's basic
    # catalog folds headings into markdown. The benchmark catalog keeps the variants so every format writes
    # the same components as the JSON it is compared with.
    text = cat["components"]["Text"]
    props = text.get("properties") or next(a for a in text["allOf"] if "properties" in a)["properties"]
    props["variant"]["enum"] = ["h1", "h2", "h3", "caption", "body"]
    for name, props in CUSTOM.items():
        cat["components"][name] = {"type": "object", "properties": {"component": {"const": name}, **props},
                                   "required": ["component", *REQUIRED[name]], "additionalProperties": False}
    cat["$defs"]["anyComponent"] = {"oneOf": [{"$ref": f"#/components/{n}"} for n in cat["components"]]}
    (B / "catalog.json").write_text(json.dumps(cat, indent=1))
    return Catalog.from_json(cat, protocol_version="1.0", catalog_id=CUSTOM_ID)


def main():
    cat = catalog()
    compilers = {"express": ExpressCompiler([cat]), "atom": AtomCompiler([cat]), "elemental": ElementalCompiler([cat])}
    ok = {}
    for fmt, comp in compilers.items():
        for n in NAMES:
            src = (B / "corpus/formats" / f"{n}.{fmt}").read_text()
            try:
                msgs = comp.compile(src, n) if fmt != "express" else comp.compile(src, surface_id=n)
                d = to_message_dicts(msgs)
                (OUT / f"{n}.{fmt}.json").write_text(json.dumps(d, indent=1))
                comps = sum(len(m.get("createSurface", {}).get("components", []) or m.get("updateComponents", {}).get("components", [])) for m in d)
                ok[(fmt, n)] = f"ok {len(d)} msg, {comps} components"
            except Exception as e:  # report and continue
                ok[(fmt, n)] = "FAIL " + str(e).splitlines()[0][:300]
            print(f"{fmt:9} {n:9} {ok[(fmt, n)]}")
    (OUT / "summary.json").write_text(json.dumps({f"{k[0]}/{k[1]}": v for k, v in ok.items()}, indent=1))


if __name__ == "__main__":
    main()
