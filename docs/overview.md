# Manifest: an overview

## What it is

Manifest is a way for an AI agent to put things on a screen and to see what is there, through one text tree. The agent writes MX, a compact one-node-per-line format; the engine lays it out, paints it, takes the user's input; and the agent reads the screen back as the same text, with the engine's own layout boxes and a short list of diagnostics, instead of a screenshot.

Three decisions make it work:

1. **One vocabulary of roles with defaults.** `button "Export CSV" disabled` is nine tokens and the renderer owns the look. Any node may take a position, a size and paint children, so the same tree can draw a funnel, a floor plan or a gauge down to the pixel, and every painted thing still carries a label and a role. Use the highest level that is faithful.
2. **Perception is the format.** `observe` returns MX, so whatever the agent reads it can patch. Level 1 is the semantic tree; level 2 adds boxes; level 3 adds every shape. A `since=` query returns only what changed, as a patch.
3. **One engine.** The core, the renderer, the host with lifecycle and input, and the MCP server are one Rust codebase that runs as a native window, headless, or as WebAssembly in the browser, with no browser dependency of its own.

## How it works

```
agent ──MX──▶ engine ──pixels──▶ screen
  ▲             │
  └──MX+boxes───┘  observe, since=, look, events
```

- **Parse:** one line per node, IDs derived from labels, errors that name the line.
- **Layout:** flexbox and grid through taffy, text shaped from the bundled Inter font, sized nodes as coordinate spaces for positioned children, nested default looks for positioned nodes without paint.
- **Paint:** tiny-skia, at the screen's pixel density, light and dark palettes.
- **Host:** surfaces, revisions, lifecycle (`ttl`, `seen`, `until`, `ephemeral`, residues), pointer and key input that produce `action` and `input` events, tweens for nodes marked `anim:`.
- **Perception:** from the engine's own boxes, with `!overflow`, `!overlap`, `!contrast`, `!tiny` and `!offcanvas`.
- **Transport:** MCP over stdio, or the same calls from JavaScript against the wasm build.

## What it costs, measured

All counts with Claude's tokenizer; reader accuracy with fresh Haiku readers that saw only the text. Full tables in `bench/results.md`.

| | MX | best alternative |
|---|---|---|
| Six everyday visuals, tokens to create | 788 | 1,762 (A2UI Atom), 4,521 (HTML) |
| Sixteen edits | 238 | 622 (HTML find and replace) |
| Read back, level 1 | 828 tokens, 30 of 30 questions | 1,544 tokens, 27 of 30 (cropped screenshots) |
| Six paint-heavy visuals | 2,685 | 6,104 (HTML + SVG + ARIA), 6,645 (A2UI paint catalog) |
| Read back, paint-heavy, level 1 and 3 | 27 and 30 of 30 | |
| A page imported from HTML | 1,161 | 4,569 (the HTML), 1,463 (accessibility snapshot) |

## Where it sits

- **A2UI** (Google) covers the write side: declarative components, no code, streaming. Manifest adds perception and lifecycle, and `docs/a2ui-extension.md` proposes both as A2UI v1.0 extensions with MX as an inference format; the compiler exists.
- **MCP Apps and browser agents** give models a UI or a page and a screenshot back. Manifest gives the tree back, and the engine is the only thing that paints it.
- **HTML** paints everything but tells a model nothing about what it painted. The importer (`import/`) reads a rendered page and writes MX with semantics first and paint where nothing semantic fits.

## Limits

- Coordinates are expensive in any text format and models place things badly. The format's answer is defaults and layout owned by the engine; the loop's answer is `observe` level 2 and the `!overlap` and `!offcanvas` diagnostics.
- Level 1 drops unlabelled shapes by design, so arrows that carry meaning need labels or a `flow`.
- Buffers hold key:value numbers; `each` over records is not there yet.
- The importer demotes a chart drawn with divs to paint; reading its values from an `aria-label` sentence is the next step.
- No 3D, no keyframe animation, no plugin widgets.
