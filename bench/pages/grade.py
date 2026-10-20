"""Grade answers to pages/questions.json. Usage: grade.py  (reads pages/out/acc/answers_*.json)"""
import json, pathlib, re
A = {
 "checkout.1": r"\b(3|three)\b", "checkout.2": r"\$?20\b", "checkout.3": r"examplecode",
 "pricing.1": r"\$?15\b", "pricing.2": r"\b30\b", "pricing.3": r"pro.*enterprise|enterprise.*pro",
 "invoice.1": r"inv/001/15", "invoice.2": r"3[.,]200", "invoice.3": r"\b20\s?%",
 "shadcn.1": r"1,?250(\.00)?", "shadcn.2": r"-\s?20\s?%|down 20", "shadcn.3": r"4\.5\s?%",
 "dashboard.1": r"1,?001", "dashboard.2": r"share.*export.*this week", "dashboard.3": r"\b(4|four)\b",
 "signin.1": r"\b(no|off|unchecked|not)\b", "signin.2": r"email", "signin.3": r"sign in",
 "album.1": r"main call to action.*secondary action", "album.2": r"view.*edit", "album.3": r"9 min",
 "tabler.1": r"\$?49\b", "tabler.2": r"\b(3|three)\b", "tabler.3": r"premium",
}
R = {"pricing.3": r"\bfree\b", "tabler.3": r"free|enterprise", "signin.1": r"\byes\b"}
def grade(ans):
    return {k: (bool(re.search(p, str(ans.get(k, "")), re.I)) and not (k in R and re.search(R[k], str(ans.get(k, "")), re.I)), str(ans.get(k, ""))) for k, p in A.items()}
if __name__ == "__main__":
    O = pathlib.Path(__file__).parent / "out" / "acc"
    for f in sorted(O.glob("answers_*.json")):
        g = grade(json.loads(f.read_text()))
        print(f.stem, f"{sum(ok for ok, _ in g.values())}/{len(g)}", "wrong:", [f"{k}: {a!r}" for k, (ok, a) in g.items() if not ok])
