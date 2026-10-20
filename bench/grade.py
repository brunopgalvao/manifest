"""Grade perception answers. Each answer is matched against an acceptance
pattern (case-insensitive). Usage: grade.py  (reads out/acc/answers_<form>.json)
"""
import json, pathlib, re

ACCEPT = {
 "dash.1": r"\b51\b", "dash.2": r"\b(no|disabled|not)\b", "dash.3": r"churn", "dash.4": r"ytd",
 "dash.5": r"export",
 "form.1": r"^\W*sam@\W*$|\bsam@(?![\w.])", "form.2": r"\byin\b", "form.3": r"\b(yes|checked|on|true)\b",
 "form.4": r"2026-10-21|oct\w* 21|10/21/2026|21/10/2026", "form.5": r"^\W*book\W*$|\"book\"|\bbook\b(?! a class)",
 "table.1": r"refunded", "table.2": r"(yes|truncat).*(1042|oluwaseun|adebayo)|(1042|oluwaseun|adebayo).*truncat",
 "table.3": r"142\.75", "table.4": r"\b(no|disabled|not)\b", "table.5": r"\b1\b.*\b4\b",
 "flow.1": r"\b(4|four)\b", "flow.2": r"\breads\b", "flow.3": r"perception", "flow.4": r"dash",
 "flow.5": r"agent",
 "confirm.1": r"\b(3|three)\b", "confirm.2": r"delete", "confirm.3": r"delete",
 "confirm.4": r"(yes).*(60|answer|action|respond)", "confirm.5": r"report\.pdf|notes\.md|draft\.docx",
 "settings.1": r"notification", "settings.2": r"\b(no|off|disabled|false)\b", "settings.3": r"\b70\b",
 "settings.4": r"\bno\b|not visible|hidden", "settings.5": r"\b(3|three)\b",
}
# Answers that name a wrong thing as well as the right one are wrong.
REJECT = {"dash.3": r"revenue|orders", "flow.3": r"\b(agent|mx|renderer)\b", "confirm.2": r"cancel",
          "confirm.3": r"cancel", "settings.4": r"\byes\b"}


def grade(ans):
    res = {}
    for k, pat in ACCEPT.items():
        a = str(ans.get(k, "")).strip()
        ok = bool(re.search(pat, a, re.I)) and not (k in REJECT and re.search(REJECT[k], a, re.I))
        res[k] = (ok, a)
    return res


if __name__ == "__main__":
    O = pathlib.Path(__file__).parent / "out" / "acc"
    summary = {}
    for f in sorted(O.glob("answers_*.json")):
        form = f.stem.replace("answers_", "")
        g = grade(json.loads(f.read_text()))
        summary[form] = {"correct": sum(ok for ok, _ in g.values()), "of": len(g),
                         "wrong": {k: a for k, (ok, a) in g.items() if not ok}}
    (O / "grades.json").write_text(json.dumps(summary, indent=1))
    for form, s in summary.items():
        print(f"{form:8} {s['correct']}/{s['of']}  wrong: {s['wrong']}")
