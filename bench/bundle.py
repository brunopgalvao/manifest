"""Build the perception bundle a fresh reader agent gets for the accuracy test: the legend, then every visual's
perception at the level (from the engine) followed by its questions.
Usage: bundle.py <tag> <level> [questions file]   e.g. bundle.py paint 1 questions_v2.json -> out/acc/bundle_paint_L1.txt"""
import json, pathlib, subprocess, sys
B = pathlib.Path(__file__).parent
MXR = B.parent / "engine" / "target" / "release" / "mxr"
tag, level = sys.argv[1], sys.argv[2]
qfile = sys.argv[3] if len(sys.argv) > 3 else "questions.json"
Q = json.loads((B / qfile).read_text())
out = [(B / "legend.txt").read_text().strip(), ""]
for n, qs in Q.items():
    out.append(f'=== UI "{n}" ===')
    out.append(subprocess.run([str(MXR), "observe", str(B / "corpus" / f"{n}.mx"), level], capture_output=True, text=True).stdout.rstrip())
    out.append("--- questions ---")
    for i, (q, _) in enumerate(qs, 1):
        out.append(f"{n}.{i} {q}")
    out.append("")
p = B / "out" / "acc" / f"bundle_{tag}_L{level}.txt"
p.write_text("\n".join(out))
print("wrote", p)
