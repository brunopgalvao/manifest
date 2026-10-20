# Proposal: perception, lifecycle and revisions for A2UI, and MX as an inference format

**Status:** Draft, not yet submitted
**Target:** A2UI v1.0 (Candidate), also applicable to v0.9.1
**Created:** 2026-10-19
**Reference implementation:** https://github.com/brunopgalvao/manifest (the compiler that shipped with the TypeScript core (now folded into the engine's A2UI emitters in `bench/`), `packages/renderer`, `packages/mcp`)

This document is written in the shape of the proposals under `specification/proposals/` in the A2UI repository, so it can be opened there as a pull request or a discussion with few changes. Section 10 lists what to strip before sending.

## 1. Background and problem statement

A2UI v1.0 is a write-mostly protocol. The agent sends `createSurface`, `updateComponents`, `updateDataModel` and `deleteSurface`. The renderer sends back `action` (a user pressed something), function calls and responses, and errors. With `sendDataModel` the renderer also piggybacks the surface's data model onto outbound messages.

Three things the agent cannot learn through the protocol:

1. **What is on screen now.** After the user has typed, toggled and switched tabs, the agent knows the data model (if it asked for it) but not the component tree it is about to update, which tab is selected, which subtrees are hidden, or whether the surface still exists.
2. **Whether its UI fits.** The agent cannot tell that a column header overflowed, two nodes overlap, a badge is unreadable, or a button is too small to tap. Today's fallback is a screenshot: with a 1280×800 viewport that is about 1,366 input tokens per look, and a model reading pixels answers factual questions about its own UI less accurately than one reading text (27/30 against 30/30 in our benchmark, section 7).
3. **When a surface should go away.** A toast, a confirmation or a progress card lives until the agent remembers to send `deleteSurface`. The agent must keep a timer, come back for a turn it would not otherwise take, and still cannot express "remove this once the user has seen it".

A fourth, smaller gap: there is no revision on a surface. Two updates that cross, or an update built from a stale view of the data model, overwrite each other silently.

A2UI v1.0 already has the hooks to close these gaps without changing the envelope:

- `metadata.extensions` at surface, component, catalog-definition and action scope, which renderers must accept and may ignore;
- catalog functions with `allowedCallers: agentOnly`, invoked with `callRendererFunction` and answered with `rendererFunctionResponse`;
- `inlineCatalogs` in `a2uiRendererCapabilities`, with which a renderer can advertise the functions it implements.

This proposal uses only those hooks. A renderer that implements none of it keeps working; an agent that sees the capability can use it.

Separately, three inference formats (Express, Atom, Elemental) are proposed as compact syntaxes that models write and a host compiles to A2UI JSON. None of them publishes a measurement. Section 6 proposes MX as a fourth, with a benchmark that can be re-run, and a compiler to both v0.9 and v1.0.

## 2. Summary of the proposal

| Part | Mechanism in v1.0 | Spec change needed |
|---|---|---|
| 3. Perception: `observe` | Agent-callable renderer function in an inline catalog | None |
| 4. Lifecycle: `ttl`, `until`, `seen`, `residue` | Surface metadata extension `manifest_lifecycle`; a renderer-originated `action` named `surfaceDissolved` | None |
| 5. Revisions | Reported by `observe` and on every `action`; optional stale-update rejection | Optional: `metadata` on `updateComponents` / `updateDataModel` |
| 6. MX inference format | A compiler, like Express/Atom/Elemental | None; a directory under `inference_formats/` |

Each part is independently adoptable. The extension name `manifest_*` follows the reversed-domain rule for third-party extensions; if the maintainers prefer, parts 3 to 5 can become official `a2ui_perception`, `a2ui_lifecycle` and `a2ui_rev`.

## 3. Perception: the `observe` function

### 3.1 Definition

A renderer that supports perception advertises an inline catalog containing one function:

```json
{
  "catalogId": "https://brunopgalvao.github.io/manifest/a2ui/perception/v1",
  "protocolVersion": "1.0",
  "functions": {
    "observe": {
      "type": "object",
      "description": "Return the current state of a surface as the agent should see it: live component tree, data model, hidden subtrees, optional layout boxes and diagnostics.",
      "returnType": "object",
      "allowedCallers": "agentOnly",
      "properties": {
        "@call": { "const": "observe" },
        "args": {
          "type": "object",
          "properties": {
            "surfaceId": { "type": "string" },
            "level": { "type": "integer", "enum": [0, 1, 2], "default": 1 },
            "since": { "type": "integer", "description": "Return only what changed after this revision." },
            "focus": { "type": "string", "description": "Component id; return only this subtree." },
            "format": { "type": "string", "enum": ["json", "mx"], "default": "json" }
          },
          "required": ["surfaceId"],
          "unevaluatedProperties": false
        }
      },
      "required": ["@call", "args"],
      "unevaluatedProperties": false
    }
  }
}
```

The agent calls it with the existing message:

```json
{"version":"v1.0","callRendererFunction":{"functionCallId":"c17","surfaceId":"dash","callFunction":{"@call":"observe","catalogId":"https://brunopgalvao.github.io/manifest/a2ui/perception/v1","args":{"surfaceId":"dash","level":2}}}}
```

and gets a `rendererFunctionResponse` whose `value` is described below. If `observe` is absent from `inlineCatalogs`, the agent knows the renderer cannot be observed and falls back to screenshots or the data model.

### 3.2 Levels

| Level | Returns | Use |
|---|---|---|
| 0 | `surfaceId`, `rev`, `lifecycle`, `nodes`, `diagnostics` count | Is it alive, did anything change since I last looked? |
| 1 | Level 0 plus the component tree with live values, the data model, and hidden-subtree summaries | What is on screen, what did the user enter, which tab is open? |
| 2 | Level 1 plus a layout box per component and the diagnostics list | Does it fit, is anything unreadable or too small? |

Level 1 is the default and is what an agent needs before most updates. Level 2 costs about 50% more tokens in our corpus (1,246 against 828 for six surfaces) and is for checking a fresh render.

### 3.3 JSON form (`format: "json"`)

```json
{
  "surfaceId": "dash",
  "rev": 3,
  "lifecycle": "persistent",
  "components": [
    {"id": "root", "component": "Column", "children": ["title", "row", "bar", "row2"], "box": [0, 0, 728, 472]},
    {"id": "title", "component": "Text", "text": "Revenue overview", "variant": "h1", "box": [17, 17, 694, 25]},
    {"id": "revenue", "component": "Metric", "label": "Revenue", "value": "$315k", "delta": "+16%", "tone": "success", "box": [17, 54, 223, 83]}
  ],
  "dataModel": {"range": ["YTD"]},
  "hidden": {"general": ["Dark mode", "Language"]},
  "diagnostics": [
    {"kind": "overflow", "id": "table", "row": "#1042", "column": "Customer", "shown": 174, "needed": 330}
  ]
}
```

Rules:

- `components` is the renderer's **current** flat list in document order, after every `updateComponents` and every user input has been applied. Values bound with `@path` are reported resolved, so the agent reads `"value": "sam@lee.dev"` and not a pointer. The agent can copy any component from this list into an `updateComponents` message unchanged, which is the point: whatever can be read can be written back.
- Components inside a hidden subtree (an unselected tab, a collapsed section) are omitted. `hidden` maps the hiding component's id to the labels of up to three direct children, so "is the Language setting on the General tab?" is answerable without opening the tab.
- `box` is `[x, y, w, h]` in CSS pixels relative to the surface's top-left, present only at level 2.
- `diagnostics` (level 1 and 2, counted at level 0) is the renderer's own report of layout problems. The four kinds below are the ones we found worth having; renderers may add more.

| kind | fields | meaning |
|---|---|---|
| `overflow` | `id`, optional `row`/`column`, `shown`, `needed` | Text was clipped or wrapped past its box: `shown` px available, `needed` px required. |
| `overlap` | `id`, `other` | Two boxes intersect that are not ancestor and descendant. |
| `contrast` | `id`, `ratio` | Foreground/background contrast under WCAG AA (4.5:1 text, 3:1 large text). |
| `tiny` | `id`, `w`, `h` | An interactive component smaller than 24×24 px. |

- `since: N` returns only components whose rendered form changed after revision N, plus `removed: [ids]` and the data-model paths that changed. If a change cannot be expressed that way (a component changed type), the renderer returns the full tree and sets `full: true`.
- `focus: id` restricts the tree to that component and its descendants.

### 3.4 Text form (`format: "mx"`)

The same information in MX, the syntax of section 6, which is 2 to 4 times cheaper in tokens than the JSON form and what our reference renderer produces natively. Level 2 of the dashboard above, exactly as the renderer returns it:

```
@dash rev=3 @0,0,728,472
h1 Revenue overview @17,17,694,25
row @17,54,694,83
  metric Revenue $315k +16% @17,54,223,83
  metric Orders 1,284 +6% @252,54,223,83
  metric Churn 2.1% +0.4pt danger @488,54,223,83
bar "Monthly revenue (k$)" Apr:38 May:42 Jun:51 Jul:47 Aug:57 *Sep:80 @17,149,694,238
row @17,399,694,55
  select Range Q3 *YTD 12m @17,399,66,55
  button "Export CSV" disabled @95,409,107,36
```

and a table with a diagnostic:

```
table Order Customer Total Status:badge @17,124,694,317
  #1041 | Ana Souza | $84.00 | Paid
  #1042 | Oluwaseun Adebayo-Whitfield Consulting Partners | $1,920.00 | Pending
  ...
!overflow table.#1042.Customer 174/330px
```

An agent that writes MX reads this and answers "which row overflows and by how much" directly. An agent that writes A2UI JSON asks for `format: "json"` and gets section 3.3. Renderers may support only `json`; `mx` is optional and is listed in the function's description when available.

### 3.5 Why a function and not a message

A2UI's renderer-to-agent envelope has four keys. Adding a fifth (`surfaceState`) would be cleaner but needs a schema change in `renderer_to_agent.json` and every transport binding. Agent-callable functions already exist for exactly this shape of request (agent asks, renderer answers with a value, correlated by `functionCallId`), and `inlineCatalogs` already carries the capability negotiation. If the maintainers would rather have a message, the payload of 3.3 is the body of that message unchanged.

### 3.6 Pushing perception without a call

Two cases do not need a round trip and are covered by extension fields the renderer already sends:

- Every `action` carries `metadata.extensions["manifest_rev"]`, the surface revision at the time of the press, so the agent knows whether its view is current before deciding what to do (section 5).
- When `sendDataModel` is on, the renderer may add `hidden` and `diagnostics` next to the data model in `a2uiRendererDataModel.surfaces[id]`, under the same extension name. Agents that read the data model get the two cheapest perception facts for free.

## 4. Lifecycle: surfaces that end on their own

### 4.1 Declaration

`createSurface.metadata.extensions["manifest_lifecycle"]`:

```json
{"version":"v1.0","createSurface":{"surfaceId":"confirm","catalogId":"…","components":[…],
  "metadata":{"extensions":{"manifest_lifecycle":{"until":"action","ttl":"60s","residue":"line"}}}}}
```

| Field | Type | The surface dissolves when |
|---|---|---|
| `ttl` | duration | that long after it first became visible, wall clock. Toasts. |
| `seen` | duration | it has been on screen for that long in total; time scrolled away or in a background tab does not count. Things the user must have seen. |
| `until` | `"action"` or a component id | any action fires, or that button is pressed. Confirmations. |
| `ephemeral` | boolean | the next agent turn begins (the transport defines a turn; in A2A it is the next message from the agent). Progress cards. |
| `residue` | `"none"`, `"line"`, `"archive"` | what remains afterwards (below). Default `none`. |

Durations are a number with a unit: `500ms`, `10s`, `2m`, `1h`. Several conditions mean whichever comes first.

`residue`:

- `none`: the surface is removed exactly as if the agent had sent `deleteSurface`.
- `line`: the renderer removes the surface and keeps a one-line trace in whatever place it shows past surfaces (a chat transcript, a log, a collapsed row): the surface's first heading, or else its first `Text`. The line is also what `observe` returns for the surface afterwards.
- `archive`: the surface is removed from the screen but kept in memory, and `observe` still returns it in full. For things the agent may want to re-read (a report the user dismissed).

### 4.2 Notification

When a surface dissolves for any reason other than the agent's own `deleteSurface`, the renderer sends an `action`:

```json
{"action":{"name":"surfaceDissolved","surfaceId":"confirm","sourceComponentId":"root",
  "timestamp":"2026-10-19T09:41:07Z",
  "context":{"reason":"until:delete","rev":3,"residue":"Delete 3 files?"},
  "metadata":{"extensions":{"manifest_lifecycle":{"reason":"until:delete"}}}}}
```

`reason` is one of `ttl`, `seen`, `until:<componentId>`, `until:action`, `ephemeral`, or `user` (the user closed it through renderer chrome). `rev` is the surface's last revision; `residue` is the line kept, when `residue: "line"`.

When `until` ends a surface because of a button, the renderer sends that button's own `action` first and `surfaceDissolved` second, so the agent sees the user's choice before the surface's end.

### 4.3 Why renderer-side

The agent could run the timers itself and send `deleteSurface`. That costs a turn per toast, fails when the agent is not resident, and cannot express `seen` at all, because only the renderer knows whether the surface was on screen. Lifecycle is an attribute of the surface the way `catalogId` is, and belongs on `createSurface`.

## 5. Revisions

Every surface has an integer revision owned by the renderer: 0 at `createSurface`, incremented by every `updateComponents`, every `updateDataModel`, and every user input that changes the data model. The revision is:

- returned by `observe` at every level;
- attached to every `action` as `metadata.extensions["manifest_rev"]`.

This is enough for an agent to notice that the user acted on a view it has since replaced, and to re-observe instead of acting on stale data.

**Optional, needs a small spec change:** an agent may state the revision an update is built on. `updateComponents` and `updateDataModel` currently have no `metadata`; we ask for `metadata` to be allowed on both, with the same `extensions` shape as `createSurface`. Then:

```json
{"version":"v1.0","updateComponents":{"surfaceId":"dash","components":[…],"metadata":{"extensions":{"manifest_rev":{"base":3}}}}}
```

A renderer that supports revisions rejects the update with an `error` (`code: "STALE_REVISION"`, `message: "surface is at rev 5"`) when `base` is not the current revision. A renderer that does not ignores the extension, as the spec already requires.

In MX this is the `~3` head of every patch; the rule "every patch says what it applies to" removed a whole class of lost updates in our dogfooding and costs one token.

## 6. MX as an inference format

### 6.1 What it is

MX is a one-node-per-line text format with two-space indentation, derived component ids and the same syntax for writing, patching and reading back. The dashboard of section 3 as the model writes it:

```
@dash
h1 "Revenue overview"
row
  metric Revenue $308k +14%
  metric Orders 1,284 +6%
  metric Churn 2.1% +0.4pt danger
bar "Monthly revenue (k$)" Apr:38 May:42 Jun:51 Jul:47 Aug:57 *Sep:73
row
  select Range Q3 *YTD 12m
  button "Export CSV" disabled
```

Ids are derived (`title`, `revenue`, `orders`, `churn`, `bar`, `range`, `export`, `row`, `row2`) so the model never writes them for creation and uses them for patches:

```
~3
*revenue $315k +16%
*bar.Sep 80
```

The full syntax is `spec/mx.md`. The compiler in the compiler that shipped with the TypeScript core (now folded into the engine's A2UI emitters in `bench/`) turns a surface into `createSurface` (v1.0) or `createSurface` + `updateComponents` + `updateDataModel` (v0.9), and a patch into `updateComponents` and `updateDataModel` messages, with the standard `@path` and `@call` forms; `node bench/a2ui_emit.mjs dash v1.0` prints them.

### 6.2 Measurements

Counted with Anthropic's `count_tokens` endpoint (the public `cl100k` tokenizer gives the same ratios within 10%). Six surfaces: dashboard, form, table, diagram, confirmation, tabbed settings, each written with identical content in every format; A2UI is minified JSON, HTML is lean hand-written markup. Method and corpus in `bench/`.

| | HTML | A2UI v0.9 JSON | A2UI v1.0 JSON | Express | Atom | Elemental | MX |
|---|---|---|---|---|---|---|---|
| Output tokens to create 6 surfaces | 4,521 | 3,658 | 3,363 | 1,874 | 1,762 | 2,476 | **788** |
| Output tokens for 16 edits | 622 (find/replace) | 1,207 (`updateComponents`/`updateDataModel`) | – | – | – | – | **238** |
| Input tokens to read 6 surfaces back | 1,544 (cropped screenshots) | not possible | not possible | – | – | – | **828** (L1) / 1,246 (L2) |
| Reading accuracy, Claude Haiku, 30 questions | 27/30 (screenshots) | – | – | – | – | – | **30/30** |

The Express, Atom and Elemental columns are the same six surfaces written in each format with exactly the components and data of the JSON, compiled with the A2UI agent SDK's own compilers (`a2ui-agent-sdk` 0.9.0) to confirm they are valid; all 18 compile. Files in `bench/corpus/formats/`, method in `bench/results.md` section 9.

Writability: three models (Claude Haiku, Sonnet, Opus) given only the spec wrote 12 of 12 requested surfaces and patches validly, and produced byte-identical patches for the same edit.

On this corpus Express is 44% below v1.0 JSON (its document claims 55–70%), Atom is 6% below Express (its document claims 25–50%), and Elemental is 26% below JSON. MX is 2.2× smaller than Atom, the smallest of the three. The difference is design rather than syntax: the three formats keep A2UI's model (an id or variable per node, a separately written data model, two nodes per button, options as objects), and MX does not (section 6.3).

### 6.3 What MX adds that the other formats do not

- **Patches with derived ids.** Express/Atom/Elemental re-send components by id; the model must have chosen and remembered the ids. MX derives them from content, so a patch is `*revenue $315k` and the model needs no memory of the creation.
- **Read-back in the same syntax.** `observe` returns MX; the model can copy a line, change a word and send it as a patch.
- **Lifecycle words on the surface line**: `@confirm until:action ttl:60s residue:line`, compiled to section 4.
- **Revision on every patch**: `~3`, compiled to section 5.

What it lacks: templates over lists (`_template` in Express), client functions beyond the fixed validation set, and per-component catalog ids. These are deliberate omissions for the first version, not conflicts.

## 7. Reference implementation

In this repository:

- `packages/core`: MX parser, patcher, perception (levels, boxes, diagnostics, `since` diffs) and the A2UI compiler for v0.9 and v1.0, with lifecycle metadata and the `surfaceDissolved` action. 23 tests, parity-tested against the Python reference used for the benchmark.
- `packages/renderer`: a DOM renderer that implements lifecycle (`ttl`, `seen`, `until`, `ephemeral`, residues), events with revisions, layout boxes from `getBoundingClientRect`, and the four diagnostics. It renders MX directly, not A2UI JSON; its `observe` is the function of section 3 with `format: "mx"`.
- `packages/mcp`: an MCP server exposing render / patch / observe / look / dissolve, which is how we dogfood it from a coding agent.

To make this an A2UI renderer feature rather than a parallel renderer, the perception module needs to be bound to an A2UI renderer's component tree. For `@a2ui/lit` or `@a2ui/react` that is: walk the surface's rendered components, read each element's `getBoundingClientRect`, compute the four diagnostics from the DOM, serialise with the current data model. The core's `perceive()` already does the serialisation and the diff from a tree plus a box map; the renderer-specific part is about 150 lines. Lifecycle is a timer and an `IntersectionObserver` per surface, about 80 lines in our renderer (`packages/renderer/src/host.ts`).

## 8. Compatibility

- Nothing here changes the agent-to-renderer or renderer-to-agent schema except the optional `metadata` on update messages in section 5.
- Renderers that do not implement the extensions ignore them, as v1.0 requires for unknown `extensions` keys.
- Agents detect support from `inlineCatalogs` (perception) and may send lifecycle metadata blind; a surface without lifecycle support simply persists, which is today's behaviour.
- All of it works on v0.9.1 too, except that v0.9.1 has no `metadata.extensions` on `createSurface`; there the lifecycle object would go in `theme` or be dropped.

## 9. Open questions for the maintainers

1. Function or message for perception (3.5)? We prefer the function for zero schema change; a message is cleaner long-term.
2. Should the level 2 `box` be in CSS pixels or density-independent units? We use CSS pixels because the diagnostics thresholds (24 px targets, WCAG ratios) are defined in them.
3. Is the `a2ui_` prefix available for lifecycle and revision if the design is accepted, so agents can rely on one name?
4. Is there interest in a shared corpus for inference-format token measurements? We offer `bench/corpus` and the counting scripts.

## 10. Before sending

- The benchmark's custom catalog id is still `https://manifest.dev/bench/catalog.json`, a placeholder kept so the recorded token counts stay reproducible. The catalog itself is `bench/catalog.json`; hosting it at the project site and changing the id moves the A2UI counts by a few tokens per surface.
