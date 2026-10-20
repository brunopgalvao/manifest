// MX (paint-heavy visuals) -> A2UI v1.0: the same visual as a createSurface message over a custom paint
// catalog (Canvas, Rect, Circle, Line, Path, Label, Image, Group with role, Series) plus the basic catalog.
// Usage: node bench/a2ui_v2.mjs <name>  (reads bench/corpus/<name>.mx, writes bench/corpus/v2a2ui/<name>.jsonl)
import { readFileSync, writeFileSync } from "node:fs";
import { loadManifest } from "../web/manifest.js";
const name = process.argv[2];
const m = await loadManifest(readFileSync(new URL("../engine/target/wasm32-unknown-unknown/release/mx_render.wasm", import.meta.url)));
const tree = m.tree(readFileSync(new URL(`./corpus/${name}.mx`, import.meta.url), "utf8"));
const comps = [];
const num = (v) => Math.round(v * 100) / 100;
const painted = (n) => ["rect", "circle", "path"].includes(n.type) || (["text", "image", "line"].includes(n.type) && n.args.pos);
function emit(n) {
  const a = n.args, c = { id: n.id };
  const style = a.style ?? {};
  const flags = new Set(n.flags);
  if (painted(n)) {
    switch (n.type) {
      case "rect": c.component = "Rect"; c.x = num(a.pos[0]); c.y = num(a.pos[1]); c.width = num(a.size[0]); c.height = num(a.size[1]); break;
      case "circle": c.component = "Circle"; c.cx = num(a.pos[0]); c.cy = num(a.pos[1]); c.r = num(a.radius); break;
      case "line": c.component = "Line"; c.x1 = num(a.pos[0]); c.y1 = num(a.pos[1]); c.x2 = num(a.pos2[0]); c.y2 = num(a.pos2[1]); break;
      case "path": c.component = "Path"; c.d = a.path; break;
      case "text": c.component = "Label"; c.x = num(a.pos[0]); c.y = num(a.pos[1]); c.text = n.label; break;
      case "image": c.component = "Image"; c.x = a.pos[0]; c.y = a.pos[1]; c.width = a.size[0]; c.height = a.size[1]; c.url = a.href; c.alt = a.alt; break;
    }
    if (n.label && n.type !== "text") c.label = n.label;
  } else if (a.size && n.children.length) {
    c.component = "Canvas"; c.role = n.type; c.width = a.size[0]; c.height = a.size[1]; if (n.label) c.label = n.label; if (a.pos) { c.x = num(a.pos[0]); c.y = num(a.pos[1]); }
    if (["button", "link"].includes(n.type)) c.action = { event: { name: n.id } };
  } else if (a.pos) {
    c.component = "Group"; c.role = n.type; c.x = num(a.pos[0]); c.y = num(a.pos[1]); if (a.size) { c.width = num(a.size[0]); c.height = num(a.size[1]); } if (n.label) c.label = n.label;
    if (["button", "link"].includes(n.type)) c.action = { event: { name: n.id } };
    if (n.type === "metric") { c.component = "Metric"; c.value = a.value; if (a.delta) c.delta = a.delta; }
  } else {
    switch (n.type) {
      case "col": case "stack": c.component = "Column"; break;
      case "row": c.component = "Row"; if (flags.has("end")) c.align = "end"; break;
      case "h1": case "h2": case "h3": c.component = "Text"; c.text = n.label; c.variant = n.type; break;
      case "text": c.component = "Text"; c.text = a.bind ? { "@path": "/" + a.bind } : n.label; break;
      case "button": c.component = "Button"; c.child = n.id + "_t"; c.action = { event: { name: n.id } }; comps.push({ id: n.id + "_t", component: "Text", text: n.label }); if (a.tone) c.variant = a.tone; break;
      case "metric": c.component = "Metric"; c.label = a.bind_label ? { "@path": "/" + a.bind_label } : n.label; c.value = a.bind ? { "@path": "/" + a.bind } : a.value; if (a.delta) c.delta = a.delta; break;
      case "series": c.component = "Series"; c.name = n.label; c.labels = a.data.map((d) => d[0]); c.values = a.data.map((d) => d[1]); break;
      case "line": case "bar": case "area": c.component = n.type === "bar" ? "BarChart" : "LineChart"; c.description = n.label; if (a.bind) c.series = { "@path": "/" + a.bind }; else { c.labels = a.data.map((d) => d[0]); c.values = a.data.map((d) => d[1]); } break;
      case "each": c.component = "Repeat"; c.items = { "@path": "/" + a.bind }; c.direction = flags.has("row") ? "row" : "column"; break;
      case "region": case "item": case "figure": c.component = "Group"; c.role = n.type; if (n.label) c.label = n.label; break;
      default: c.component = n.type;
    }
  }
  for (const [k, v] of Object.entries(style)) c[{ fill: "fill", stroke: "stroke", width: "strokeWidth", radius: "cornerRadius", opacity: "opacity", size: "fontSize", weight: "fontWeight", anchor: "textAnchor", rotate: "rotate", scale: "scale", anim: "animate" }[k] ?? k] = isNaN(Number(v)) ? v : Number(v);
  if (flags.has("dashed")) c.dashed = true;
  if (flags.has("disabled")) c.disabled = true;
  if (n.children.length) c.children = n.children.map((ch) => ch.id);
  comps.push(c);
  for (const ch of n.children) emit(ch);
}
emit(tree.root);
const msg = { version: "v1.0", createSurface: { surfaceId: tree.id, catalogId: "https://manifest.dev/bench/catalog-paint.json", components: comps } };
const dm = {};
const series = (n) => { if (n.type === "series") dm[n.id] = Object.fromEntries(n.args.data.map((d) => [d[0], d[1]])); for (const c of n.children) series(c); };
series(tree.root);
if (Object.keys(dm).length) msg.createSurface.dataModel = dm;
writeFileSync(new URL(`./corpus/v2a2ui/${name}.jsonl`, import.meta.url), JSON.stringify(msg) + "\n");
console.log(name, comps.length, "components");
