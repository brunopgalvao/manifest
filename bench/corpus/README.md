# Corpus

Six visuals, each expressed with the **same semantics** in every format. Only the encoding differs.

| id | class | what it is |
|---|---|---|
| `dash` | dashboard | 3 KPI metrics, a 6-month bar chart, a range select, and a disabled Export button |
| `form` | form | Yoga class booking: name, email, date, class select, newsletter checkbox, submit |
| `table` | table/list | 8 orders × 4 columns with status badges, a search field and pagination. One customer name is long enough to truncate (planted defect). |
| `flow` | diagram | 4-box pipeline with arrows: Agent → MX → Renderer → Perception, with a loop back to Agent |
| `confirm` | transient | Ephemeral confirm dialog ("Delete 3 files?"), Cancel and Delete buttons |
| `settings` | settings | Tabs (General / Notifications). Visible tab has 3 toggles and a volume slider. |

Formats:
- `*.html`: lean, hand-written HTML/CSS as a careful model would emit it, with one shared style block per file. `data-mx="id"` attributes exist only so the harness can map DOM boxes to node IDs. **They are stripped before token counting.**
- `*.a2ui.jsonl`: A2UI v0.9 messages. The basic catalog lacks chart, table, metric, badge and vector shapes, so those use a **custom catalog** (`bench/custom`) with the same props MX uses. That keeps the comparison about encoding overhead, not catalog coverage.
- `*.mx`: Manifest MX v0, per `spec/v0.md`.
- `*.mx1`: Manifest MX v1 (designed after the baseline run).
