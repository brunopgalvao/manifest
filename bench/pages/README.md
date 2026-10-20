# Pages: rendered web pages as MX

A dataset of real pages imported into MX by `import/src/batch.ts`, for measuring the importer and for anyone who wants paired examples of a rendered page and its MX.

```bash
cd engine && cargo build --release && cargo build --release --target wasm32-unknown-unknown -p mx-render
cd ../import && npm install
node src/batch.ts ../bench/pages/urls.txt ../bench/pages/out          # fetch and import every page
cd ../bench && .venv/bin/python pages/report.py pages/out              # tokens, coverage, the table
```

## What a record is

For each URL in `urls.txt` (tab-separated: url, source, licence), `out/` holds:

| file | what |
|---|---|
| `<slug>.mx` | the import: MX the engine renders and reads back |
| `<slug>.html` | the rendered DOM at capture time, as a snapshot |
| `<slug>.png` | a screenshot of the viewport that was imported (1280 wide) |
| `<slug>.json` | the captured tree with every element's box, computed style and role, the importer's source boxes per emitted line, and its stats |
| `<slug>-N.png` | pictures the page held as data URLs or canvases, referenced by the MX |
| `index.jsonl` | one line per page: timing, line counts, whether the MX parses, text runs kept, node kinds, the round trip of the engine's boxes against the page's |

The snapshots and screenshots are not committed (they are large and the pages belong to their authors); `urls.txt`, the MX files, `index.jsonl` and the report are.

## Licences

Only pages under permissive licences are listed: Bootstrap, shadcn, Tabler, AdminLTE, TodoMVC, Pico and Bulma examples (MIT), Wikipedia and MDN (CC BY-SA), the Python docs (PSF), the Rust docs (MIT/Apache), US government pages (public domain), the HTML standard (CC BY), W3C's ARIA practices (W3C document licence), and this project's own site. Pages are fetched one at a time with an identified user agent, and consent banners are declined when a decline control exists.

## What the scores mean

- **parse**: the MX parses in the engine. Anything else is an importer bug.
- **text kept**: visible text runs of the page that appear in the MX. The target is all of them.
- **semantic / painted / demoted**: nodes with a default look, shapes, and subtrees the importer could not read semantically and wrote as a sized region of shapes.
- **round trip**: imported nodes whose rendered height (engine) is within 20% or 8 px of the page's box. Widths follow Manifest's column, so they are reported apart.
- **tokens**: the page's HTML against its MX, Claude's tokenizer.
