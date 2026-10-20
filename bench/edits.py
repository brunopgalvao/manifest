"""Three scripted edits per visual, expressed in each format.

html_replace: list of (old, new) pairs, the way a coding agent edits a file
              (an Edit tool call). Counted as tokens(old) + tokens(new).
a2ui:         the JSONL messages A2UI needs. updateComponents re-sends whole
              components because v0.9 upserts components and does not merge props.
mx0 / mx1:    MX patch blocks for spec v0 and v1.
Full HTML regeneration is also reported (the whole file), since that is what
a chat-rendered artifact usually costs.
"""
import json


def j(*msgs):
    return "\n".join(json.dumps(m, separators=(",", ":")) for m in msgs)


def uc(sid, *comps):
    return {"version": "v0.9", "updateComponents": {"surfaceId": sid, "components": list(comps)}}


def ud(sid, path, value):
    return {"version": "v0.9", "updateDataModel": {"surfaceId": sid, "path": path, "value": value}}


EDITS = {
 "dash": [
  {"what": "Sep revenue 73 -> 80 and KPI to $315k",
   "html_replace": [('<div class="v">$308k</div><div class="up">+14%</div>', '<div class="v">$315k</div><div class="up">+16%</div>'),
                    ('47, 57, 73 thousand', '47, 57, 80 thousand')],
   "a2ui": j(uc("dash", {"id": "rev", "component": "Metric", "label": "Revenue", "value": "$315k", "delta": "+16%", "tone": "positive"},
                {"id": "chart", "component": "BarChart", "description": "Monthly revenue (k$)", "labels": ["Apr", "May", "Jun", "Jul", "Aug", "Sep"], "values": [38, 42, 51, 47, 57, 80], "highlight": 5})),
   "mx0": '~dash 1\n* rev value="$315k" delta="+16%"\n* chart y=[38,42,51,47,57,80]',
   "mx1": '~1\n*revenue $315k +16%\n*bar.Sep 80'},
  {"what": "Enable Export",
   "html_replace": [('<button disabled data-mx="export">', '<button data-mx="export">')],
   "a2ui": j(ud("dash", "/exportReady", True)),
   "mx0": '~dash 2\n* export enabled=true',
   "mx1": '~2\n*export enabled'},
  {"what": "Add a note after the chart",
   "html_replace": [('<div class="row" data-mx="controls">', '<p class="up">September is a record month</p>\n<div class="row" data-mx="controls">')],
   "a2ui": j(uc("dash", {"id": "root", "component": "Column", "children": ["title", "kpis", "chart", "note", "controls"]},
                {"id": "note", "component": "Text", "text": "September is a record month"})),
   "mx0": '~dash 3\n+ note text "September is a record month" tone=positive in=root after=chart',
   "mx1": '~3\n+bar text "September is a record month" success'},
 ],
 "form": [
  {"what": "Fix email value",
   "html_replace": [('value="sam@"><span class="hint">Enter a valid email</span>', 'value="sam@lee.dev">')],
   "a2ui": j(ud("form", "/email", "sam@lee.dev")),
   "mx0": '~form 1\n$/email = "sam@lee.dev"',
   "mx1": '~1\n*email ="sam@lee.dev"'},
  {"what": "Add Hatha option",
   "html_replace": [('<option>Ashtanga</option>', '<option>Ashtanga</option><option>Hatha</option>')],
   "a2ui": j(uc("form", {"id": "class", "component": "ChoicePicker", "label": "Class", "options": [{"label": "Vinyasa", "value": "vinyasa"}, {"label": "Yin", "value": "yin"}, {"label": "Ashtanga", "value": "ashtanga"}, {"label": "Hatha", "value": "hatha"}], "value": {"path": "/class"}})),
   "mx0": '~form 2\n* class options=["Vinyasa","Yin","Ashtanga","Hatha"]',
   "mx1": '~2\n*class +Hatha'},
  {"what": "Relabel submit",
   "html_replace": [('data-mx="submit">Book<', 'data-mx="submit">Book Yin class<')],
   "a2ui": j(uc("form", {"id": "submit_t", "component": "Text", "text": "Book Yin class"})),
   "mx0": '~form 3\n* submit "Book Yin class"',
   "mx1": '~3\n*book "Book Yin class"'},
 ],
 "table": [
  {"what": "Mark #1044 paid",
   "html_replace": [('$210.00</td><td><span class="b refunded">Refunded', '$210.00</td><td><span class="b paid">Paid')],
   "a2ui": j(ud("table", "/orders/3/status", "Paid")),
   "mx0": '~table 1\n$/orders/3/3 = "Paid"',
   "mx1": '~1\n*table.#1044.Status Paid'},
  {"what": "Go to page 2",
   "html_replace": [('<button disabled data-mx="prev">Previous</button><span data-mx="page">Page 1 of 4', '<button data-mx="prev">Previous</button><span data-mx="page">Page 2 of 4')],
   "a2ui": j(uc("table", {"id": "page", "component": "Text", "text": "Page 2 of 4"})),
   "mx0": '~table 2\n* prev enabled=true\n* page "Page 2 of 4"',
   "mx1": '~2\n*previous enabled\n*page "Page 2 of 4"'},
  {"what": "Remove row #1047",
   "html_replace": [('<tr><td>#1047</td><td>Marco Rossi</td><td>$19.99</td><td><span class="b paid">Paid</span></td></tr>\n', '')],
   "a2ui": j(ud("table", "/orders", [r for r in []]) ) if False else None,  # filled below
   "mx0": '~table 3\n$- /orders/6',
   "mx1": '~3\n-table.#1047'},
 ],
 "flow": [
  {"what": "Rename MX box",
   "html_replace": [('text-anchor="middle">MX</text>', 'text-anchor="middle">MX text</text>')],
   "a2ui": j(uc("flow", {"id": "mx", "component": "Box", "x": 186, "y": 40, "w": 130, "h": 56, "label": "MX text", "tone": "accent"})),
   "mx0": '~flow 1\n* mx "MX text"',
   "mx1": '~1\n*mx "MX text"'},
  {"what": "Make the loop arrow solid",
   "html_replace": [('stroke="#555" stroke-dasharray="4 4"', 'stroke="#555"')],
   "a2ui": j(uc("flow", {"id": "a4", "component": "Arrow", "from": "perception", "to": "agent", "label": "tree, boxes, diagnostics", "route": "below"})),
   "mx0": '~flow 2\n* a4 dashed=',
   "mx1": '~2\n*perception>agent solid'},
  {"what": "Recolour Perception box",
   "html_replace": [('fill="#e3f6ea" stroke="#0a7a3d"', 'fill="#eef1fd" stroke="#4f6bed"')],
   "a2ui": j(uc("flow", {"id": "perception", "component": "Box", "x": 558, "y": 40, "w": 130, "h": 56, "label": "Perception", "tone": "accent"})),
   "mx0": '~flow 3\n* perception tone=accent',
   "mx1": '~3\n*perception primary'},
 ],
 "confirm": [
  {"what": "Change body to 4 files",
   "html_replace": [('report.pdf, notes.md and draft.docx will', 'report.pdf, notes.md, draft.docx and photo.png will'), ('Delete 3 files?', 'Delete 4 files?')],
   "a2ui": j(uc("confirm", {"id": "title", "component": "Text", "text": "Delete 4 files?", "variant": "h2"},
                {"id": "body", "component": "Text", "text": "report.pdf, notes.md, draft.docx and photo.png will be moved to Trash."})),
   "mx0": '~confirm 1\n* title "Delete 4 files?"\n* body "report.pdf, notes.md, draft.docx and photo.png will be moved to Trash."',
   "mx1": '~1\n*title "Delete 4 files?"\n*body "report.pdf, notes.md, draft.docx and photo.png will be moved to Trash."'},
  {"what": "Relabel Delete",
   "html_replace": [('data-mx="delete">Delete<', 'data-mx="delete">Move to Trash<')],
   "a2ui": j(uc("confirm", {"id": "delete_t", "component": "Text", "text": "Move to Trash"})),
   "mx0": '~confirm 2\n* delete "Move to Trash"',
   "mx1": '~2\n*delete "Move to Trash"'},
  {"what": "Dismiss after the user answers",
   "html_replace": [],  # needs agent-side tracking + a full re-render without the dialog; counted as regeneration
   "html_regen_only": True,
   "a2ui": j({"version": "v0.9", "deleteSurface": {"surfaceId": "confirm"}}),
   "mx0": "",  # life=until:action dissolves it with no message at all
   "mx1": ""},
 ],
 "settings": [
  {"what": "Switch to General tab",
   "html_replace": [('aria-selected="false" data-mx="general"', 'aria-selected="true" data-mx="general"'),
                    ('aria-selected="true" data-mx="notif"', 'aria-selected="false" data-mx="notif"'),
                    ('hidden data-mx="general-panel"', 'data-mx="general-panel"'),
                    ('<div class="panel" role="tabpanel" data-mx="notif-panel">', '<div class="panel" role="tabpanel" hidden data-mx="notif-panel">')],
   "a2ui": None,  # not expressible: A2UI Tabs has no selected-tab prop
   "mx0": '~settings 1\n* tabs selected=general',
   "mx1": '~1\n*tabs General'},
  {"what": "Turn email digest on",
   "html_replace": [('Email digest<input type="checkbox" role="switch">', 'Email digest<input type="checkbox" role="switch" checked>')],
   "a2ui": j(ud("settings", "/email", True)),
   "mx0": '~settings 2\n$/email = true',
   "mx1": '~2\n*email on'},
  {"what": "Volume 70 -> 40",
   "html_replace": [('max="100" value="70"', 'max="100" value="40"')],
   "a2ui": j(ud("settings", "/volume", 40)),
   "mx0": '~settings 3\n$/volume = 40',
   "mx1": '~3\n*volume 40'},
 ],
}

# A2UI updateDataModel removes the key at a path when value is omitted.
EDITS["table"][2]["a2ui"] = j({"version": "v0.9", "updateDataModel": {"surfaceId": "table", "path": "/orders/6"}})
