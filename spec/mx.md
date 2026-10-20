# MX: the Manifest format

> One tree that the model writes, the screen renders and the model reads back.

MX is a text format for anything on a screen. One node per line, two-space indent to nest. A node is a **role** (`button`, `metric`, `region`, `rect`…), a label, some state, and optionally a position, a size and paint. The renderer owns layout and look by default; a node that declares geometry or paint children takes them over. Perception returns the same syntax, with the renderer's boxes and diagnostics, so whatever the model reads it can write back as a patch.

---

## 1. Shape of a document

```
@<surface-id> [lifecycle...]
<node line>
  <child node line>
```

- Each line is one node. Indent by two spaces per level to nest.
- Top-level lines are children of an implicit vertical column (`root`).
- Words are separated by spaces. Quote anything with spaces: `button "Export CSV"`.
- **Headings, `text` and `badge`:** everything after the type is the text, no quotes needed: `h1 Class feedback`. Trailing tone words and `#id` are read as modifiers, so quote text that really ends with one.
- **Everything else:** the label is quoted when it has spaces, and bare words after it must be valid for that node. An unknown word is an error that names the line (`primay` is caught, not ignored).
- A line that fails to parse fails the whole document: `line=N <message>`.

## 2. IDs: derived, stable, overridable

Every node gets an ID without you writing one:

| Node | Derived ID |
|---|---|
| The first `h1` | `title` |
| A labelled node (text, metric, button, field, select, check, toggle, slider, tab, link, image, badge, a titled card, region, item, figure, rect, circle, line, path) | The first word of the label, lowercased, letters and digits only. `"Export CSV"` → `export`, `card "Pro"` → `pro`. |
| Anything else (other headings, row, col, an untitled card, table, tabs, charts, an unlabelled shape) | Its type name. A second heading is `h2`; a second `row` is `row2`. |
| A flow node | Its name, slugged: `Agent` → `agent` |
| A `series` | Its name |

**Duplicates are scoped by their nearest labelled container**: a `card "Pro"`, a `tab Billing`, a named `flow`, a labelled `region`, `item` or `figure`, or any labelled node that shows paint children (`button "Export PNG"` over a rect and a text). The ID becomes `<container-id>.<base>`: `pro.monthly`, `export.text`. Only when that still collides, or there is no labelled container, do duplicates get `2`, `3`, … in document order.

- Add `#name` anywhere on the line to choose the ID yourself.
- **IDs are fixed at creation.** Relabelling a button does not change its ID, and a node inserted later never renames an existing one. Perception prints `#id` whenever the ID is not what these rules would derive, so you never have to remember it.

## 3. Nodes

Arguments are positional, in the order shown. `[...]` means optional. Flags are bare words.

### Layout
| Line | Meaning |
|---|---|
| `row [end\|center\|between]` | Horizontal group. Metrics and cards in a row share the width. |
| `col` | Vertical group. |
| `card ["Title"]` | Bordered container. A title gives it an ID and makes it a labelled container. |
| `grid <cols>` | Grid, e.g. `grid 3`. |
| `tabs` | Children are `tab` lines. |
| `tab [*]Label` | A tab; `*` marks the selected one. Children are its content. |

### Content
| Line | Meaning |
|---|---|
| `h1\|h2\|h3 Text` | Heading. |
| `text Text [tone]` | Paragraph. |
| `badge Text [tone]` | Pill label. |
| `metric Label Value [Delta] [tone]` | A number with a label. `+` deltas are green and `-` deltas red unless a tone says otherwise. |
| `image <src> "alt"` | A picture at natural size, never upscaled, scaled down only to fit the column, with no rounding, so a screenshot comes back pixel for pixel. |
| `link Text <url>` | A link; opening it is confirmed and reported as an event. |

### Data
| Line | Meaning |
|---|---|
| `bar\|line\|area\|pie ["Caption"] Key:Num ...` | Chart. `*Key:Num` highlights a datum. |
| `table Col Col:badge ...` then one row per child line: `#1041 \| Ana \| $84 \| Paid` | The first cell is the row key. A `:badge` column paints each cell as a pill, toned by a fixed word list (`paid`, `done`, `active`… success; `pending`, `review`… warning; `failed`, `overdue`… danger; `new`, `queued`… info; else muted) or by an explicit `Text:tone`. |
| `flow ["Caption"]` then chain lines: `Agent >writes MX >draws Renderer:success` | Diagram, laid out left to right. `Name:tone` colours a node, `>label` names an edge, a trailing `below`, `above`, `dashed` routes or styles the chain's edges. Back edges are routed below. |
| `series <name> Key:Num ...` | A data buffer (section 5). |

### Inputs
| Line | Meaning |
|---|---|
| `button Label [tone] [disabled]` | Fires `action`. |
| `field Label [email\|date\|number\|search\|password] [="value"] [required]` | Text input. |
| `select Label Opt *Opt Opt [multi]` | `*` marks the chosen option(s). |
| `check Label on\|off`, `toggle Label on\|off` | Boolean inputs. |
| `slider Label <value> [min..max]` | Range, default `0..100`. |

**Tones** are the CSS-framework words: `primary success danger warning info muted`.

### Generic roles
| Line | Meaning |
|---|---|
| `region ["Label"]`, `item ["Label"]`, `figure ["Label"]` | A node that means something but has no look of its own: a map area, a diagram box, a chart. With a size it is a coordinate space; with children it shows them. Without either it is just its label. |
| `decoration` | A node that means nothing to a reader: omitted from perception below level 3. The only role that may go unlabelled when interactive roles may not. |

## 4. Geometry: positions, sizes, spaces, paint

Any node may carry geometry. The renderer lays out and paints by default; geometry takes that over, piece by piece.

- `WxH` on a node makes it a **space**: a coordinate system in CSS pixels, with the node's top-left as the origin. A space's children are all positioned. A space is laid out like any block (at most the column's width, scaled down to fit, never up).
- `@x,y` on a node **positions** it in its parent's space. A positioned node without children gets its **default look** at its natural size (or the size it declares): `button Reset @400,280` is a real button, laid out and painted by the renderer, sitting at that point. A positioned node with children shows them instead, in its own space: `button "Export PNG" @520,280 100x32` over a `rect` and a `text` is a button that looks like those two shapes and still fires `action`.
- **Primitives** are the shapes: `rect @x,y WxH`, `circle @cx,cy r:N`, `line @x1,y1 @x2,y2`, `path "M 0 0 L 10 10 Z"`, `text @x,y "String"`, `image @x,y WxH <src> "alt"`. They need a position and a space to sit in; they cannot have children. `text`, `image` and `line` are also node types with a default look; a position tells the two readings apart.
- A quoted string after a shape's geometry is its **label**, which gives it an ID and shows it in perception: `rect @0,0 560x48 fill:primary "Leads 1,240"`.
- **Style words:** `fill:`, `stroke:`, `width:` (stroke width), `radius:` (corners), `opacity:`, `size:` and `weight:` (text), `anchor:start|middle|end` (a text's position is the start of its baseline), `rotate:deg`, `scale:k`, `anim:` (section 5), and the flag `dashed`. A node with a default look takes `fill:`, `stroke:`, `radius:`, `opacity:`, `rotate:` and `scale:` too, once it carries a position or a size. Unknown style words are errors.
- **Colours** are the six tones, a tone with a lightness step (`primary-70`: that fraction of the tone over the background, so a surface stays consistent across the light and dark palettes), a palette key (`fg`, `fg2`, `bg`, `card`, `line`, `white`), a hex (`#2f5bea`, `#abc`) or `none`.
- Positions and sizes may have decimals. A position outside a space, or a child without a position inside one, is an error.

```
region 640x320 #pipe
  rect @0,0 640x320 fill:muted-50 #bg
  region "Funnel" @40,20
    rect @0,0 560x48 fill:primary radius:6 "Leads 1,240"
    text @280,30 "Leads 1,240" fill:white anchor:middle size:14 weight:600
  circle @300,200 r:4 fill:danger "Target"
  button "Export PNG" @520,280 100x32
    rect @0,0 100x32 fill:primary radius:16
    text @50,21 "Export PNG" fill:white anchor:middle
  toggle Live on @20,280
```

The rule for every writer, model or importer: **use the highest level that is faithful.** Coordinates cost tokens and models place things badly; a `button` is nine tokens and a painted one is forty. Paint when nothing with a default look says what you mean.

## 5. Buffers: structure once, data many times

```
series sales Apr:38 May:42 Jun:51 anim:300ms
line "Revenue" =sales
metric Latest =sales.last
each =sales row
  metric =key =value
```

- `series <name> Key:Num ...` declares a named buffer. `=name` on a chart binds the chart to it. `=name.last|first|min|max|sum|count|<Key>` on a `metric`, `text`, heading or `badge` binds one value.
- Patches touch the buffer, not the nodes: `+sales Jul:47` appends, `*sales.Jun 55` sets, `-sales.Apr` removes. Every bound node follows on the next paint.
- `each =buffer [row|col|N]` paints its children once per element; inside the template `=key`, `=value` and `=index` bind to the element. Every instance of a template node takes its element's key as an ID suffix (`metric.Apr`), so boxes and patches address instances.
- `anim:300ms` on any node, or on a series, makes the renderer interpolate the next change to its numbers (buffer values, positions, sizes, radii, slider values) over that time with an ease-in-out curve. Perception and boxes always show the target; only pixels move.
- Perception shows a buffer as a summary (`series sales [6 points] min:38 max:73 last:73`), bindings as written, and `each =sales row [6 instances]` with its template beneath.

## 6. Lifecycle (on the `@` line)

| Word | Dissolves when |
|---|---|
| *(nothing)* | Persistent: dissolves only when the model or the user closes it. |
| `ephemeral` | The next model turn starts. |
| `ttl:10s` | 10 seconds after it first became visible. Units `ms`, `s`, `m`, `h`; bare numbers are seconds. |
| `until:action` / `until:<id>` | Any action fires, or that button is pressed. |
| `seen:10s` | It has been on screen, at least half visible, for a total of 10 seconds. |
| `residue:none\|line\|archive` | What remains afterwards. `none`: everything is gone. `line`: a one-line summary stays on screen and in `observe` listings (`@saved dissolved: Settings saved`). `archive`: gone from the screen, but the model can still `observe` it. |

Several conditions mean whichever comes first: `@confirm until:action ttl:60s residue:line`.

## 7. Patches

```
~<rev>
*<addr> <args>
+<addr> <node line>
-<addr>
```

- The first line names the revision the patch creates: the surface's current revision plus one. A stale revision rejects the whole patch (`stale rev 1 (surface at 2)`). `~<surface> <rev>` binds the patch to a surface explicitly.
- `*<addr> <args>` sets. Words mean what they mean on the node's own line, and only what is given changes: `*revenue $315k +16%`, `*export enabled`, `*export disabled primary`, `*tabs General`, `*email ="a@b.c"`, `*class +Hatha` (adds an option), `*note "New text" success`. Geometry and style words apply to any node: `*marker @300,180`, `*won 340x48 fill:warning`, `*export @500,280`.
- Addresses go inside nodes: `*table.#1044.Status Paid` sets a cell (row key, column), `*bar.Sep 80` sets a datum, `*sales.Jun 55` a buffer value, `*perception>agent solid` an edge.
- `+<addr> <node line>` inserts after `addr`; `+<addr>/ ...` appends inside it. `+table/ a | b | c` appends a row, `+sales Jul:47` a datum. Inside a space, the inserted node is positioned (or a primitive).
- `-<addr>` removes a node, a row or a datum.
- Every address must resolve or the whole patch is rejected, with the patch line in the error. Addresses resolve by longest ID prefix, so dotted IDs and dotted cell addresses coexist.

## 8. Events (host → model)

Every tool result starts with the events since the last call, oldest first:

```
@form action book rev=5 {"email":"sam@lee.dev","news":true}
@form input email ="x" rev=6
@settings input tabs "General" rev=2
@help link https://example.com rev=0
@toast dissolved reason=ttl:10s rev=0
```

An `action` carries the data model: every input's value by ID. Each user input adds one to the revision, as a patch does.

## 9. Perception (model → host → model)

`observe [level] [since=<rev>] [focus=<id>]` returns MX in the same syntax as above.

| Level | Returns |
|---|---|
| **0** | `@id lifecycle rev=N nodes=N diags=N`. Is it alive, and did anything change? |
| **1** | The tree with live values and diagnostics. Hidden subtrees (an unselected tab) collapse to their children's labels: `tab General [hidden: Dark mode, Language, +2]`. Flows show `[N nodes]` and their chains. A space shows `[N shapes, M labelled]` and only its **labelled** nodes beneath it, nested, decorations omitted: a model reading a screen full of custom paint gets a short list of things that mean something. |
| **2** | Level 1 plus `@x,y,w,h` boxes (pixels, surface-relative, after any scaling of a space). |
| **3** | Every node as written, shapes included, with boxes: what a model needs to patch anything it painted. |

**Diagnostics** come from the renderer's own layout and end the listing: `!overflow <addr> shown/needed px` (clipped text), `!overlap a b` (diagram parts), `!contrast <addr> ratio` (text below WCAG AA), `!tiny <addr> WxHpx` (a target under 24 px), `!offcanvas <id>` (a shape entirely outside its space).

`since=<rev>` returns a patch block instead of the tree: `*` lines for changed values, `+` lines for inserted nodes and rows, `-` lines for removals, buffer changes as `*s.key v`, `-s.key` and `+s key:v`. It falls back to the full tree only when a change has no patch form (a node changed type, a diagram changed shape). A new surface is rev 0; every patch and every user input adds one.

`look <id>` returns a PNG of one node (or the surface), the only pixel path. Use it to judge appearance, and `observe` for everything else.

**A rule for writers that follows from level 1:** an arrow or line that carries meaning gets a label, or the diagram is written as a `flow`. Unlabelled shapes are invisible at level 1 by design.

## 10. Transport (MCP)

The engine is an MCP server over stdio (`manifest-mcp`), with a native window or `--headless`:

| Tool | Does |
|---|---|
| `manifest.render {mx}` | Shows a surface. Rendering an existing ID replaces it. Returns the level 0 line. |
| `manifest.patch {surface, patch}` | Applies a patch. Returns the level 0 line. |
| `manifest.observe {surface?, level?, since?, focus?, wait?}` | Reads back. Without a surface, lists every surface's level 0 line and residue lines. `wait` blocks up to that many seconds for a user event. |
| `manifest.look {surface, id?}` | A PNG of one node. |
| `manifest.dissolve {surface}` | Removes a surface, honouring its residue. |

The server's instructions carry the cheat sheet (`spec/cheatsheet.md`) and the resource `manifest://spec` serves this document. The same engine runs in the browser as WebAssembly with the same calls.

## 11. Fidelity

What "paint anything" means, and how it is tested (`engine/render/tests/paint.rs`, `bench/fidelity/`):

1. A screenshot in an `image` comes back pixel for pixel.
2. A screenshot in a sized region with labelled positioned nodes on top comes back with 0 differing pixels; the nodes read back with their roles and take input.
3. A figure drawn by another rasterizer, rebuilt from primitives, differs only at anti-aliased edges (0.62% of pixels by more than 32/255, mean 0.86/255 on the reference figure).

Out of scope: 3D (a reader cannot cheaply read a projected scene back), keyframe animation, and a "define a widget" mechanism (the closed role vocabulary is what keeps perception reliable; recurring patterns become node types).
