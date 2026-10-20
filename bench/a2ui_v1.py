"""Rewrite the hand-written A2UI v0.9 corpus (gen_a2ui.py) as v1.0: one createSurface carrying the
components and the data model, `{"@path"}` / `{"@call"}` bindings. Same author, same components, so the
two columns differ only by protocol version. Writes corpus/<name>.a2ui1.jsonl."""
import json, pathlib

C = pathlib.Path(__file__).parent / "corpus"


def rekey(x):
    if isinstance(x, dict):
        return {("@" + k if k in ("path", "call") else k): rekey(v) for k, v in x.items()}
    if isinstance(x, list):
        return [rekey(v) for v in x]
    return x


def to_v1(msgs):
    cs = next(m["createSurface"] for m in msgs if "createSurface" in m)
    out = {"surfaceId": cs["surfaceId"], "catalogId": cs["catalogId"]}
    for m in msgs:
        if "updateComponents" in m:
            out["components"] = rekey(m["updateComponents"]["components"])
        if "updateDataModel" in m:
            out["dataModel"] = m["updateDataModel"]["value"]
    return [{"version": "v1.0", "createSurface": out}]


if __name__ == "__main__":
    for f in sorted(C.glob("*.a2ui.jsonl")):
        msgs = [json.loads(l) for l in f.read_text().splitlines() if l.strip()]
        v1 = to_v1(msgs)
        (C / f.name.replace(".a2ui.jsonl", ".a2ui1.jsonl")).write_text(
            "\n".join(json.dumps(m, separators=(",", ":"), ensure_ascii=False) for m in v1) + "\n")
        print("wrote", f.name, "->", len(json.dumps(v1, separators=(",", ":"))), "chars")
