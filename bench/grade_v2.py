"""Grade answers to questions_v2.json. Usage: grade_v2.py  (reads out/acc/answers_paint_*.json)"""
import json, pathlib, re
ACCEPT = {
 "funnel.1": r"1,?240", "funnel.2": r"7\.7", "funnel.3": r"\bwon\b", "funnel.4": r"export png", "funnel.5": r"\b480\b",
 "gantt.1": r"\bbuild\b", "gantt.2": r"\blaunch\b", "gantt.3": r"\bhiring\b", "gantt.4": r"warning|amber|orange|yellow", "gantt.5": r"\b(5|five)\b",
 "map.1": r"dock c", "map.2": r"aisle 6", "map.3": r"(2|two).*loading.*waiting|loading.*waiting", "map.4": r"\b(6|six)\b", "map.5": r"locate",
 "kpi.1": r"48,?210", "kpi.2": r"\+?12\s?%", "kpi.3": r"\b12\b", "kpi.4": r"view report", "kpi.5": r"\bup\b|rising|increas|upward",
 "live.1": r"\b73\b", "live.2": r"\b(6|six)\b", "live.3": r"\b38\b", "live.4": r"latest.*peak.*(line|revenue)|(line|revenue).*latest.*peak", "live.5": r"\b73\b",
 "diagram.1": r"\bapi\b", "diagram.2": r"https", "diagram.3": r"\bapi\b", "diagram.4": r"fill", "diagram.5": r"\b(5|five)\b",
}
REJECT = {"funnel.3": r"leads|qualified", "gantt.2": r"design|build|beta|hiring", "diagram.1": r"cache(?!.*miss)", "map.1": r"dock a|dock b"}
def grade(ans):
    return {k: (bool(re.search(p, str(ans.get(k, "")), re.I)) and not (k in REJECT and re.search(REJECT[k], str(ans.get(k, "")), re.I)), str(ans.get(k, ""))) for k, p in ACCEPT.items()}
if __name__ == "__main__":
    O = pathlib.Path(__file__).parent / "out" / "acc"
    for f in sorted(O.glob("answers_paint_*.json")):
        g = grade(json.loads(f.read_text()))
        wrong = [f"{k}: {a!r}" for k, (ok, a) in g.items() if not ok]
        print(f.stem, f"{sum(ok for ok, _ in g.values())}/{len(g)}", "wrong:", wrong)
