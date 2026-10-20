# Benchmark results

Every number here was measured, with the method next to it. Token counts use Claude's tokenizer (the `count_tokens` endpoint, `bench/count_claude.py`); earlier runs with a proxy tokenizer counted about 45% lower for every text format and are not reported. Reader accuracy uses fresh Haiku readers that see only a bundle of perception text, a short legend (`bench/legend.txt`) and the questions, and write answers that a regex grader scores (`bench/grade.py`, `bench/grade_v2.py`). One accuracy run per form, so a difference of one or two questions is noise.

The corpus is twelve visuals in `bench/corpus/`: six everyday ones (a dashboard, a form, a table, a flow diagram, a confirm dialog, a settings page) that exist in four formats (MX, lean HTML, A2UI v0.9 and v1.0, and A2UI's own inference formats), and six paint-heavy ones (a funnel, a gantt, a floor plan, a metric card, a live chart, a request-path diagram) that exist as MX, as HTML+SVG with ARIA roles, and as A2UI v1.0 over a custom paint catalog.

## 1. Writing: tokens to create

Six everyday visuals, the same content in each format:

| format | tokens | vs MX |
|---|---|---|
| **MX** | **788** | |
| A2UI Atom (S-expressions, compiled by the A2UI SDK) | 1,762 | 2.2× |
| A2UI Express (positional calls) | 1,874 | 2.4× |
| A2UI Elemental (HTML-like) | 2,476 | 3.1× |
| A2UI v1.0 JSON | 3,363 | 4.3× |
| A2UI v0.9 JSON | 3,658 | 4.6× |
| lean HTML (hand-written, harness attributes stripped) | 4,521 | 5.7× |

Part of the gap is design A2UI could adopt: derived IDs, positional arguments, tones as words. The `flow` primitive gives up explicit coordinates, which is where most of the diagram's saving comes from. All eighteen A2UI inference-format files compile with Google's SDK (`bench/validate_formats.py`).

Six paint-heavy visuals (`bench/count_v2.py`):

| visual | MX | HTML + SVG + ARIA | A2UI v1.0, paint catalog |
|---|---|---|---|
| funnel | 448 | 912 | 1,131 |
| gantt | 485 | 1,002 | 1,217 |
| map | 626 | 1,230 | 1,552 |
| kpi | 336 | 608 | 842 |
| live | 143 | 1,224 | 450 |
| diagram | 623 | 1,128 | 1,493 |
| **six visuals** | **2,661** | **6,104 (2.3×)** | **6,685 (2.5×)** |

The gap is smaller than for everyday visuals because coordinates cost the same in every format. What MX saves is the markup around them and the `series` buffer: the live chart is 143 tokens against 1,224 for an SVG that spells out every point. The expectation to hold: paint lands at a third to a half of the HTML tokens, not a tenth.

## 2. Editing: tokens for sixteen edits

Sixteen scripted edits across the six everyday visuals (`bench/edits.py`): change a value, toggle a flag, add a row, remove a node, switch a tab.

| format | tokens |
|---|---|
| **MX patches** | **238** |
| HTML, find and replace | 622 |
| A2UI update messages | 1,207 |
| HTML, regenerate the page | 12,506 |

Find and replace on HTML is the fair baseline; MX beats it by 2.6×. A patch names the revision it applies to, so a stale edit is rejected rather than applied to the wrong state.

## 3. Reading back: tokens and accuracy

Thirty questions over the six everyday visuals (`bench/questions.json`): values, states, what is selected, what sits next to what, what is truncated.

| what the reader got | tokens for six visuals | correct, out of 30 |
|---|---|---|
| **MX perception, level 1** | **876** | **30** |
| **MX perception, level 2** (with boxes) | **1,294** | **30** |
| cropped screenshots of the HTML twins | 1,544 | 27 |
| full-viewport screenshots | 2,017 | |
| Playwright accessibility tree | 1,415 | 22 |

Screenshots cannot show exact chart values, slider values or lifecycle, and the accessibility tree could not describe the SVG diagram at all. Perception at level 1 is about half the cost of the cheapest pixel path and more accurate.

Thirty questions over the six paint-heavy visuals (`bench/questions_v2.json`):

| perception | tokens for six visuals | reader 1 | reader 2 |
|---|---|---|---|
| level 1 (labelled nodes only) | 2,092 | 28 of 30 | 27 of 30 |
| level 3 (every shape, with boxes) | 3,604 | 29 of 30 | |

Both level 1 readers missed the two questions about the diagram's arrows (where a cache miss goes, what the dashed arrow means), and one of them the question about which gantt task crosses the today line. All three depend on unlabelled lines and paths, which level 1 omits by design. The level 3 reader, who had every shape with its box, answered the arrow questions and missed the today line (a comparison of x coordinates it got wrong). The rule that follows, now in the spec: an arrow that carries meaning gets a label, or the diagram is written as a `flow`; the request-path diagram as a `flow` would be eight lines and read at level 1.

## 4. Writability by models

Haiku, Sonnet and Opus each wrote three new surfaces and one patch from the spec alone, with no examples. Twelve of twelve files parsed, and all three patches produced the asked-for state. Their notes about ambiguities (duration units, quoting of tab labels, numbers with commas, how `*tabs` is addressed) became spec text.

## 5. Diagnostics instead of screenshots

Fourteen defects planted in the corpus (`bench/defects/`): truncated text, overlapping diagram parts, and tiny targets where valid MX could produce them. The renderer's diagnostics (`!overflow`, `!overlap`, `!tiny`, `!contrast`) found 14 of 14 with no false positives, matching a Haiku screenshot reviewer at 8% of its tokens. Only truncation can be planted from valid MX: contrast, target size and overlap cannot occur from the renderer's own defaults, which the diagnostics caught twice during development.

## 6. Fidelity: paint anything

Tier 1, raster passthrough (`bench/fidelity/`): five screenshots and photos rendered through an `image` node came back with 0 differing pixels each, after two renderer rules were removed (corner rounding and column stretching).

Tier 2, raster with semantics (`engine/render/tests/paint.rs`): a photo inside a sized region with a `button`, a `link` and a `region` positioned on top. 0 differing pixels against the source; level 1 lists the three nodes with their roles; a click on the button fires `action`, a click on the region fires nothing.

Tier 3, vector reconstruction: a figure drawn by PIL at 4× supersampling (rounded rect, outlined rect, ring, line, triangle, square) rebuilt from primitives. 0.62% of pixels differ by more than 32/255, mean difference 0.86/255, all at anti-aliased edges; PIL's inclusive coordinates and inside-drawn outlines are stated in the MX as quarter-pixel sizes.

## 7. The engine against a browser's layout

The engine lays out with taffy and paints with tiny-skia. Its gate: reproduce the boxes Chromium produced for the same stylesheet and the same font (Inter, recorded in `engine/render/tests/dom-boxes/`) within 2 px, with the same diagnostics.

| | Chromium | engine |
|---|---|---|
| boxes on the six everyday visuals | 49 | 49, all within 2 px (worst 2 px) |
| diagnostics | `!overflow table.#1042.Customer 174/330px` | `!overflow table.#1042.Customer 173/353px` |
| perception | from DOM measurement | from the engine's own layout, byte-identical on the fixtures |
| runtime dependencies | Chromium, 195 to 359 MB | none; one binary, or 3.5 MB of WebAssembly with the fonts |

Before the font was shared, 45 of 49 boxes matched and the rest were off by 4 to 5 px from glyph widths alone. In the browser the wasm build loads in under 20 ms and parses, lays out and paints a corpus visual in about 13 ms.

## 8. Importing HTML

`import/` reads a rendered page from Chromium and writes MX: semantic nodes where a role fits, SVG box-and-arrow diagrams as `flow`, inline SVG as primitives, and `role=img` subtrees as a sized region of rects and text at the page's coordinates. Scored on the six everyday HTML twins (`import/src/score.ts`):

| page | reference nodes recovered | visible text runs kept | heights within 20% or 8px | widths within 20% or 8px |
|---|---|---|---|---|
| dash | 6 of 7 (the chart is paint, not `bar`) | 18 of 18 | 19 of 21 | 15 of 21 |
| form | 7 of 7 | 8 of 8 | 3 of 8 | 0 of 8 |
| flow | 5 of 6 (the diagram's caption differs) | 9 of 9 | 1 of 2 | 2 of 2 |
| table | 6 of 6 | 40 of 40 | 6 of 7 | 3 of 7 |
| confirm | 5 of 5 | 4 of 4 | 5 of 6 | 2 of 6 |
| settings | 8 of 9 (a hidden tab's content is not rendered) | 7 of 7 | 3 of 8 | 2 of 8 |
| **all** | **37 of 40** | **86 of 86** | **37 of 52** | **24 of 52** |

Widths follow Manifest's column rather than the page's CSS, which is the point of importing, so they are scored apart. What a page costs to read before and after (`bench/count_import.py`):

| page | HTML source | accessibility snapshot | imported MX |
|---|---|---|---|
| dash | 987 | 187 | 498 |
| form | 516 | 194 | 110 |
| table | 1,068 | 743 | 277 |
| flow | 925 | 94 | 119 |
| confirm | 480 | 96 | 73 |
| settings | 593 | 149 | 84 |
| **total** | **4,569** | **1,463** | **1,158 (3.9× smaller than the HTML)** |

The dash page shows the cost of demotion: twelve rects and labels at explicit coordinates are 498 tokens where `bar "Monthly revenue" Apr:38 ...` would be 30. Reading a chart's values from the page's `aria-label` sentence is the obvious next step.

## 9. Real pages, by the hundred

`bench/pages/` runs the importer over UI-heavy pages under permissive licences (Bootstrap, shadcn, Tabler, AdminLTE, TodoMVC, Pico and Bulma examples; Wikipedia, MDN, the Python and Rust docs, US government pages, the HTML standard, W3C's ARIA practices, this site) at a desktop width, follows up to ten same-site links per source, and scores every import without a hand-written reference (`import/src/batch.ts`, `bench/pages/report.py`). Each page record holds the MX, the DOM snapshot, the screenshot, the captured tree with boxes, and the trainable pair: the engine's own painting of the MX and its level 2 read-back. The MX files, the index and the report are committed; snapshots and screenshots are not. A nightly workflow reruns it.

| | |
|---|---|
| pages listed and crawled | 246 |
| fetched, and parsed in the engine | 246 of 246 |
| visible text runs kept, median | 99.1% (lowest 61.9%) |
| nodes with a default look, median share | 100.0% |
| heights within 20% or 8 px of the page's, median | 43.8% |
| HTML tokens, all pages | 35,567,239 |
| MX tokens, all pages | 8,261,392 |
| HTML to MX, median page and total | 8.67× and 4.31× |

The first run of the first hundred parsed 42; every failure was an importer bug a real page exposed (computed colours in `lab()` and `oklab()`, SVG names taken from embedded stylesheets, hrefs with line breaks, icon-only buttons, image sources with query strings read as `key=value`, text that looked like geometry, tab panels above their tab list, chart labels doubled by SVG `<title>`s) and all were fixed, so the corpus did its job. The total ratio is lower than the median because a few chart-heavy pages become thousands of lines of paint; the median page is a tenth of its HTML.

**Reading real pages: MX against the screenshot.** Twenty-four questions over eight of the pages (`bench/pages/questions.json`: a cart total, a plan's price, an invoice number, a dashboard figure, what a button says, whether a box is checked, which plan carries a star icon), answered by fresh Haiku readers given either the imported MX at level 1 or the page's screenshot, two readers each.

| what the reader got | tokens for the eight pages | reader 1 | reader 2 |
|---|---|---|---|
| imported MX, level 1 | 12,102 | 24 of 24 | 24 of 24 |
| screenshot, 1280 by 900 | 12,288 | 23 of 24 | 24 of 24 |

The first pass of this experiment went the other way: both MX readers scored 22 of 24 and both missed the same two questions, a check mark in a comparison table and a star icon on a plan. Both were meaning carried by icons the importer had dropped as decoration. The importer now names icons from their accessible name, their `<use>` reference, their icon class, or, for the common nameless ones, the shape of their path (check, cross, star, chevrons), and writes the name into table cells; the second pass is the table above. The screenshot reader that missed a question read "Share and Export" where the page also has "This week".

What this says, plainly: on real pages at a desktop viewport, perception as MX costs about the same as the screenshot and reads at least as well. The difference is what else it carries: the whole page rather than the viewport, exact values rather than pixels, every node addressable for a patch, and no vision model in the loop. The drastic token savings are on the writing side and on UI the agent produced itself; on reading arbitrary pages the saving is parity at level 1 with those properties on top, and a tenth of the HTML if the alternative was reading the source.

## Reproduce

```bash
cd engine && cargo build --release && cargo test --release          # engine, parity, fidelity
cd bench && python3 -m venv .venv && .venv/bin/pip install anthropic pillow
node render.js                                                       # screenshots, accessibility trees, boxes from the HTML twins
.venv/bin/python count_claude.py                                     # §1 to §3 token tables (ANTHROPIC_API_KEY)
.venv/bin/python count_v2.py && .venv/bin/python count_import.py     # paint and import tables
.venv/bin/python bundle.py paint 1 questions_v2.json                 # a reader bundle; give it to a fresh model, grade with grade_v2.py
node ../import/src/batch.ts pages/urls.txt pages/out --crawl 10 && .venv/bin/python pages/report.py pages/out   # §9
.venv/bin/python pages/bundle.py                                     # the two reader bundles for §9; grade with pages/grade.py
```
