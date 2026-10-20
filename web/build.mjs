// Build the site into web/dist: the README as the front page, the spec, the overview, the benchmark report,
// the A2UI proposal, and the playground, which runs the engine as WebAssembly.
// Needs the wasm: cd engine && cargo build --release --target wasm32-unknown-unknown -p mx-render
import { marked } from "marked";
import { readFileSync, writeFileSync, mkdirSync, readdirSync, existsSync, copyFileSync } from "node:fs";
import { dirname, join, basename } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const out = join(here, "dist");
mkdirSync(out, { recursive: true });

const PAGES = [
  { src: "README.md", out: "index.html", title: "Manifest" },
  { src: "docs/overview.md", out: "overview.html", title: "Manifest overview" },
  { src: "spec/mx.md", out: "spec.html", title: "MX specification" },
  { src: "bench/results.md", out: "results.html", title: "Benchmark results" },
  { src: "docs/a2ui-extension.md", out: "a2ui-extension.html", title: "A2UI extension proposal" },
];
const NAV = [
  ["index.html", "Manifest"], ["playground.html", "Playground"], ["overview.html", "Overview"], ["spec.html", "Spec"],
  ["results.html", "Benchmark"], ["a2ui-extension.html", "A2UI proposal"], ["https://github.com/brunopgalvao/manifest", "GitHub"],
];
const LINKS = Object.fromEntries(PAGES.map((p) => [p.src, p.out]));

const css = readFileSync(join(here, "site.css"), "utf8");
const nav = (active) => NAV.map(([h, t]) => `<a href="${h}"${h === active ? ' class="on"' : ""}>${t}</a>`).join("");
const shell = (title, body, active) => `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>${title}</title><style>${css}</style></head>
<body><nav>${nav(active)}</nav>
<main>${body}</main>
<footer>Manifest is Apache-2.0. <a href="https://github.com/brunopgalvao/manifest">Source, issues and discussion on GitHub.</a></footer>
</body></html>`;

for (const p of PAGES) {
  let md = readFileSync(join(root, p.src), "utf8");
  for (const [src, dst] of Object.entries(LINKS)) md = md.replaceAll("`" + src + "`", `[\`${src}\`](${dst})`);
  writeFileSync(join(out, p.out), shell(p.title, marked.parse(md, { gfm: true }), p.out));
}

// Playground: the engine, every corpus visual as an example, the cheat sheet.
const wasm = join(root, "engine/target/wasm32-unknown-unknown/release/mx_render.wasm");
if (!existsSync(wasm)) throw new Error("build the engine first: cd engine && cargo build --release --target wasm32-unknown-unknown -p mx-render");
copyFileSync(wasm, join(out, "mx_render.wasm"));
copyFileSync(join(here, "manifest.js"), join(out, "manifest.js"));
const corpus = join(root, "bench/corpus");
const examples = readdirSync(corpus).filter((f) => f.endsWith(".mx")).sort().map((f) => [basename(f, ".mx"), readFileSync(join(corpus, f), "utf8")]);
const cheat = readFileSync(join(root, "spec/cheatsheet.md"), "utf8");
const play = readFileSync(join(here, "playground.html"), "utf8")
  .replace("/*EXAMPLES*/", () => JSON.stringify(Object.fromEntries(examples)))
  .replace("<!--CHEAT-->", () => cheat.replace(/</g, "&lt;"))
  .replace("<!--NAV-->", () => nav("playground.html"))
  .replace("/*CSS*/", () => css);
writeFileSync(join(out, "playground.html"), play);
console.log(`site: wrote ${PAGES.length + 1} pages to web/dist`);
