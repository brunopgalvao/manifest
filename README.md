# Manifest

One tree that the model writes, the screen renders and the model reads back.

MX is a text format for anything on a screen. An agent writes it, patches it and *observes* it in the same syntax, with the engine's own layout boxes and diagnostics. One vocabulary of roles with a default look (`button "Export CSV" disabled`, `metric Revenue $308k +14%`), and any node may take a position, a size and paint children, so the same tree draws a funnel, a floor plan or a gauge down to the pixel while every painted thing keeps a label and a role. One engine, in Rust, lays out, paints, takes input and answers perception with no browser: a native window, headless, or WebAssembly in the browser. An importer turns rendered HTML into MX.

Site and playground: https://brunopgalvao.github.io/manifest/

```
@dash
h1 Revenue overview
row
  metric Revenue $308k +14%
  metric Churn 2.1% +0.4pt danger
bar "Monthly revenue (k$)" Apr:38 May:42 Jun:51 *Sep:73
region "Pipeline" 640x120
  rect @0,0 560x48 fill:primary radius:6 "Leads 1,240"
  button "Export PNG" @520,80 100x32
    rect @0,0 100x32 fill:primary radius:16
    text @50,21 "Export PNG" fill:white anchor:middle
  toggle Live on @20,80
```

## Measured

Counted with Claude's tokenizer; read back by fresh Haiku readers that saw only the text. Tables and method in `bench/results.md`.

| | MX | next best | HTML |
|---|---|---|---|
| Six everyday visuals, tokens to create | 788 | 1,762 (A2UI Atom) | 4,521 |
| Sixteen edits | 238 | 622 (find and replace) | 12,506 (regenerate) |
| Read back, level 1 | 876 tokens, 30 of 30 questions | 1,544, 27 of 30 (cropped screenshots) | 1,415, 22 of 30 (accessibility tree) |
| Six paint-heavy visuals | 2,661 | 6,104 (HTML + SVG + ARIA) | 6,685 (A2UI, paint catalog) |
| A page imported from HTML | 1,158 | 1,463 (accessibility snapshot) | 4,569 |
| A hundred real pages imported | 736,715 | | 6,510,211 (8.8×) |

Fidelity: a screenshot comes back pixel for pixel; a photo under positioned, labelled nodes comes back with 0 differing pixels and those nodes take input; a figure drawn by another rasterizer, rebuilt from primitives, differs on 0.62% of pixels at anti-aliased edges. The engine reproduces Chromium's layout of the same stylesheet within 2 px on every box.

## Layout

| Path | What |
|---|---|
| `spec/mx.md` | The format: nodes, ids, geometry, buffers, lifecycle, patches, events, perception, transport |
| `spec/cheatsheet.md` | The one-page version the MCP server sends as its instructions |
| `engine/mx` | The core: parser, ids, patches, diff, perception |
| `engine/render` | Layout (taffy), paint (tiny-skia, Inter bundled), spaces and primitives, the host with lifecycle and input, screen composition; builds to WebAssembly with a plain C ABI |
| `engine/mcp` | `manifest-mcp`: the MCP server over stdio with a native window (winit) or `--headless` |
| `web/` | `manifest.js`, the JavaScript glue for the wasm build (browser and Node), the playground, the site build |
| `import/` | The HTML importer and its scorer |
| `bench/` | The corpus in every format, token accounting, reader accuracy, fidelity references, import scores, the report |
| `bench/pages/` | A hundred real pages imported into MX: the URL list with licences, the MX files, the index and the report |
| `docs/` | The overview and the A2UI v1.0 extension proposal |

## Use it from an agent

```bash
cd engine && cargo build --release
./target/release/manifest-mcp              # MCP over stdio; a window shows the surfaces
./target/release/manifest-mcp --headless   # no window: layout, perception and look still work
```

`.mcp.json` registers it for Claude Code when you run Claude Code from this directory. For any other MCP client:

```json
{ "mcpServers": { "manifest": { "command": "/path/to/manifest/engine/target/release/manifest-mcp" } } }
```

Tools: `manifest.render`, `patch`, `observe` (levels 0 to 3, `since=`, `focus=`, `wait=`), `look` (a PNG of one node), `dissolve`. Every result starts with the user events since the last call. The agent gets the cheat sheet as the server's instructions and the spec as the resource `manifest://spec`.

## The engine on the command line and in the browser

```bash
./target/release/mxr render ../bench/corpus/dash.mx dash.png        # paint (add --dark for the dark palette)
./target/release/mxr observe ../bench/corpus/funnel.mx 3            # perception from the engine's own layout
./target/release/mxr boxes ../bench/corpus/dash.mx                  # boxes and diagnostics as JSON
cargo test --release                                                 # parity, host, stdio, paint, fidelity
cargo build --release --target wasm32-unknown-unknown -p mx-render  # the engine for the browser
cd ../web && npm install && node build.mjs                           # the site, playground included, into web/dist
```

From JavaScript, in a page or in Node:

```js
import { loadManifest } from "./manifest.js";
const m = await loadManifest("mx_render.wasm");
m.render(mx); m.observe("dash", 2); m.compose(w, h, devicePixelRatio); m.click("dash", x, y);
```

## Import a page

```bash
cd import && npm install
node src/import.ts ../bench/corpus/dash.html --id dash                 # MX on stdout
node src/score.ts imported.mx reference.mx import.json                  # recall, text runs, round trip
node src/batch.ts ../bench/pages/urls.txt ../bench/pages/out            # the dataset: a page record per URL
```

## Documents

- `docs/overview.md`: what Manifest is, how it works, what it costs, where it sits, limits.
- `docs/a2ui-extension.md`: proposal for A2UI v1.0: perception, lifecycle, revisions, MX as an inference format.
- `bench/results.md`: the benchmark report.
- `HANDOFF.md`: state of the project and next steps.

## License

Apache-2.0. See `LICENSE`.
