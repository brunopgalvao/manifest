/**
 * The HTML importer: a rendered page becomes an MX surface with semantics first and paint where nothing
 * semantic fits. It reads the rendered result from Chromium (layout, computed styles, accessibility
 * roles), not the HTML source, because that is where everything is resolved.
 *
 *   node packages/import/src/import.ts <file-or-url> [--id name] [--out file.mx] [--json tree.json]
 *
 * Nodes with a default look where a role fits (headings, text, buttons, links, inputs, tables, images,
 * metrics, diagrams as `flow`); positioned primitives inside a sized region for inline SVG, for <canvas>
 * (as a raster) and for any painted subtree with no semantic reading (the demotion rule).
 */
import { chromium } from "playwright";
import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export interface El {
  tag: string;
  role: string;
  name: string;
  text: string;
  box: { x: number; y: number; w: number; h: number };
  style: { bg: string; border: string; borderW: number; color: string; fs: number; fw: number; radius: number; display: string; dir: string; opacity: number };
  attrs: Record<string, string>;
  options?: Array<{ label: string; selected: boolean }>;
  children: El[];
  svg?: SvgNode[];
  dataUrl?: string;
}

export interface SvgNode {
  tag: string;
  attrs: Record<string, string>;
  text: string;
  /** translation and uniform scale of the element's CTM relative to the svg root */
  tx: number;
  ty: number;
  sc: number;
  box: { x: number; y: number; w: number; h: number };
  children: SvgNode[];
}

/** Runs in the page: the rendered tree with boxes, computed styles and roles. */
function capture(): El {
  const implicit: Record<string, string> = { h1: "heading", h2: "heading", h3: "heading", h4: "heading", h5: "heading", h6: "heading", p: "paragraph", a: "link", button: "button", img: "img", table: "table", select: "combobox", textarea: "textbox", ul: "list", ol: "list", li: "listitem", nav: "navigation", main: "main", header: "banner", footer: "contentinfo", section: "region", article: "article", form: "form", label: "label", svg: "img", canvas: "img" };
  // any CSS colour (rgb, lab, oklab, color-mix) to hex through a canvas; transparent stays empty
  const cctx = document.createElement("canvas").getContext("2d", { willReadFrequently: true })!;
  const hexCache = new Map<string, string>();
  const hex = (c: string): string => {
    if (!c || c === "none" || c === "transparent") return "";
    const cached = hexCache.get(c);
    if (cached !== undefined) return cached;
    const m = c.match(/^rgba?\(([^)]+)\)$/);
    let out = "";
    if (m) {
      const p = m[1].split(/[,\s/]+/).map(Number);
      out = p.length > 3 && p[3] === 0 ? "" : "#" + p.slice(0, 3).map((v) => Math.round(v).toString(16).padStart(2, "0")).join("");
    } else {
      cctx.clearRect(0, 0, 1, 1); cctx.fillStyle = "#000"; cctx.fillStyle = c; cctx.fillRect(0, 0, 1, 1);
      const d = cctx.getImageData(0, 0, 1, 1).data;
      out = d[3] === 0 ? "" : "#" + [d[0], d[1], d[2]].map((v) => v.toString(16).padStart(2, "0")).join("");
    }
    hexCache.set(c, out);
    return out;
  };
  const nameOf = (el: Element): string => {
    const aria = el.getAttribute("aria-label");
    if (aria !== null && el.getAttribute("role") === "img" && el.tagName.toLowerCase() !== "svg") return "";
    if (el.tagName.toLowerCase() === "svg") return (aria ?? el.querySelector(":scope > title")?.textContent ?? "").trim();
    if (aria) return aria.trim();
    const lb = el.getAttribute("aria-labelledby");
    if (lb) return (document.getElementById(lb)?.textContent ?? "").trim();
    if (el instanceof HTMLInputElement || el instanceof HTMLSelectElement || el instanceof HTMLTextAreaElement) {
      const lab = el.labels?.[0];
      if (lab) return [...lab.childNodes].filter((n) => n.nodeType === 3).map((n) => n.textContent!.trim()).filter(Boolean).join(" ") || lab.textContent!.trim();
      return el.getAttribute("placeholder") ?? el.getAttribute("name") ?? "";
    }
    if (el instanceof HTMLImageElement) return el.alt;
    return (el.textContent ?? "").replace(/\s+/g, " ").trim();
  };
  const svgWalk = (root: SVGSVGElement, el: Element): SvgNode => {
    const attrs: Record<string, string> = {};
    for (const a of el.getAttributeNames()) attrs[a] = el.getAttribute(a)!;
    const cs = getComputedStyle(el);
    for (const k of ["fill", "stroke", "stroke-width", "font-size", "font-weight", "text-anchor", "opacity"]) if (!(k in attrs)) { const v = cs.getPropertyValue(k); if (v && v !== "none" && v !== "normal") attrs[k] = v; }
    for (const k of ["fill", "stroke"]) if (attrs[k] && attrs[k] !== "none" && !attrs[k].startsWith("url(")) attrs[k] = hex(attrs[k]) || "none";
    let tx = 0, ty = 0, sc = 1;
    const g = el as SVGGraphicsElement;
    if (g.getCTM && root.getCTM) {
      const m = root.getCTM()!.inverse().multiply(g.getCTM()!);
      tx = m.e; ty = m.f; sc = Math.sqrt(Math.abs(m.a * m.d - m.b * m.c)) || 1;
    }
    const r = (el as SVGGraphicsElement).getBoundingClientRect(); const rr = root.getBoundingClientRect();
    return { tag: el.tagName.toLowerCase(), attrs, text: el.tagName.toLowerCase() === "text" ? (el.textContent ?? "").trim() : "", tx, ty, sc, box: { x: r.x - rr.x, y: r.y - rr.y, w: r.width, h: r.height }, children: [...el.children].filter((c) => !["defs", "title", "desc", "style", "script"].includes(c.tagName.toLowerCase())).map((c) => svgWalk(root, c)) };
  };
  const walk = (el: Element): El | null => {
    const tag = el.tagName.toLowerCase();
    if (["script", "style", "noscript", "template", "head", "meta", "link", "title"].includes(tag)) return null;
    const he = el as HTMLElement;
    if (he.checkVisibility && !he.checkVisibility()) return null;
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    if (r.width === 0 && r.height === 0 && tag !== "body") return null;
    const attrs: Record<string, string> = {};
    for (const k of ["href", "type", "value", "placeholder", "src", "alt", "min", "max", "name", "aria-label", "title"]) { const v = el.getAttribute(k); if (v !== null) attrs[k] = v; }
    if (el instanceof HTMLInputElement) { attrs.value = el.value; if (el.checked) attrs.checked = "1"; if (el.disabled) attrs.disabled = "1"; if (el.required) attrs.required = "1"; }
    if (el instanceof HTMLButtonElement && el.disabled) attrs.disabled = "1";
    if (el instanceof HTMLSelectElement && el.disabled) attrs.disabled = "1";
    if (el.getAttribute("aria-selected") === "true" || el.getAttribute("aria-checked") === "true") attrs.checked = "1";
    const e: El = {
      tag, role: el.getAttribute("role") ?? (tag === "input" ? ({ checkbox: "checkbox", radio: "radio", range: "slider", submit: "button", button: "button", reset: "button" }[(el as HTMLInputElement).type] ?? "textbox") : implicit[tag] ?? ""),
      name: nameOf(el),
      text: [...el.childNodes].filter((n) => n.nodeType === 3).map((n) => n.textContent!.replace(/\s+/g, " ")).join("").trim(),
      box: { x: r.x + scrollX, y: r.y + scrollY, w: r.width, h: r.height },
      style: { bg: hex(cs.backgroundColor), border: parseFloat(cs.borderTopWidth) > 0 && cs.borderTopStyle !== "none" ? hex(cs.borderTopColor) : "", borderW: parseFloat(cs.borderTopWidth) || 0, color: hex(cs.color), fs: parseFloat(cs.fontSize), fw: parseInt(cs.fontWeight) || 400, radius: parseFloat(cs.borderTopLeftRadius) || 0, display: cs.display, dir: cs.flexDirection, opacity: parseFloat(cs.opacity) },
      attrs, children: [],
    };
    if (el instanceof HTMLImageElement) attrs.src = el.currentSrc || el.src;
    if (el instanceof HTMLSelectElement) e.options = [...el.options].map((o) => ({ label: o.textContent!.trim(), selected: o.selected }));
    if (tag === "svg") { e.svg = [...el.children].filter((c) => !["defs", "title", "desc", "style"].includes(c.tagName.toLowerCase())).map((c) => svgWalk(el as SVGSVGElement, c)); return e; }
    if (tag === "canvas") { try { e.dataUrl = (el as HTMLCanvasElement).toDataURL("image/png"); } catch { /* tainted */ } return e; }
    if (tag === "select" || tag === "textarea") return e;
    for (const c of el.children) { const ce = walk(c); if (ce) e.children.push(ce); }
    return e;
  };
  return walk(document.body)!;
}

// -- MX emission ----------------------------------------------------------------

const RESERVED = new Set(["primary", "success", "danger", "warning", "info", "muted", "disabled", "enabled", "on", "off", "required", "multi", "email", "date", "number", "search", "password", "end", "center", "between", "row", "col"]);
/** Quote a label only when a bare word would be misread: spaces, quotes, empty, a reserved word, or a leading modifier character. */
const q = (s: string) => (/[\s"]/.test(s) || !s || RESERVED.has(s) || /^[*@#=+\-~!]/.test(s) || /^\d+(\.\d+)?x\d/.test(s) || /:/.test(s) ? JSON.stringify(s) : s);
const isDark = (hex: string) => { if (!hex) return false; const v = parseInt(hex.slice(1), 16); const r = v >> 16, g = (v >> 8) & 255, b = v & 255; return 0.2126 * r + 0.7152 * g + 0.0722 * b < 140; };
const saturated = (hex: string) => { if (!hex) return false; const v = parseInt(hex.slice(1), 16); const r = v >> 16, g = (v >> 8) & 255, b = v & 255; return Math.max(r, g, b) - Math.min(r, g, b) > 60; };
const toneOf = (hex: string): string | null => {
  if (!hex || !saturated(hex)) return null;
  const v = parseInt(hex.slice(1), 16); const r = v >> 16, g = (v >> 8) & 255, b = v & 255;
  if (b > r && b > g) return "primary"; if (g > r && g > b) return "success"; if (r > g * 1.5 && r > b * 1.5) return "danger"; if (r > b && g > b) return "warning"; return "info";
};
/** Inside a canvas every label and string is quoted. */
const qq = (s: string) => JSON.stringify(s);
const fmt = (n: number) => (Math.round(n * 100) / 100).toString();

export class Importer {
  lines: string[] = [];
  /** source boxes for every emitted node line, in emission order, for the round-trip score */
  boxes: Array<{ line: number; box: El["box"]; text: string }> = [];
  stats = { semantic: 0, painted: 0, demoted: 0, textRuns: 0 };
  /** pictures the page held as data URLs or canvases, written next to the MX by the caller */
  assets: Array<{ name: string; bytes: Buffer }> = [];
  prefix = "page";

  /** A data URL becomes a file next to the MX; other sources stay as they are. */
  src(url: string): string {
    const m = url.match(/^data:image\/(png|jpeg|jpg|gif|webp);base64,(.*)$/);
    if (!m) return url;
    const name = `${this.prefix}-${this.assets.length + 1}.${m[1] === "jpeg" ? "jpg" : m[1]}`;
    this.assets.push({ name, bytes: Buffer.from(m[2], "base64") });
    return name;
  }

  /** A sized node that holds positioned children: `region "Caption" WxH`. */
  space(indent: number, w: number, h: number, caption: string | undefined, el?: El) {
    this.push(indent, `region${caption ? " " + JSON.stringify(caption) : ""} ${fmt(w)}x${fmt(h)}`, el);
  }

  push(indent: number, line: string, el?: El) {
    this.lines.push("  ".repeat(indent) + line);
    if (el) this.boxes.push({ line: this.lines.length - 1, box: el.box, text: el.name });
  }

  /** Whether the children sit side by side (a row) rather than stacked. */
  isRow(el: El): boolean {
    const kids = el.children.filter((c) => c.box.w > 0);
    if (kids.length < 2) return false;
    if (el.style.display.includes("flex") && el.style.dir.startsWith("row")) return true;
    if (el.style.display.includes("grid")) return true;
    let side = 0;
    for (let i = 1; i < kids.length; i++) {
      const a = kids[i - 1].box, b = kids[i].box;
      if (b.x >= a.x + a.w - 1 && Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y) > Math.min(a.h, b.h) / 2) side++;
    }
    return side === kids.length - 1;
  }

  /** The texts a node shows, flattened, for text-only subtrees. */
  flatText(el: El): string {
    return el.name;
  }

  isTextOnly(el: El): boolean {
    if (el.svg || el.dataUrl || el.options) return false;
    if (["input", "button", "select", "a", "img", "table"].includes(el.tag)) return false;
    return el.children.every((c) => ["span", "b", "i", "em", "strong", "small", "code", "br", "sup", "sub", "time"].includes(c.tag) && this.isTextOnly(c));
  }

  /** A metric: a small label, a big number, an optional delta, in a box. */
  asMetric(el: El): string | null {
    const kids = el.children.filter((c) => c.name);
    if (kids.length < 2 || kids.length > 3 || !kids.every((k) => this.isTextOnly(k))) return null;
    const [l, v, d] = kids;
    if (v.style.fs < l.style.fs * 1.4 || v.name.length > 16 || l.name.split(" ").length > 4) return null;
    const tone = d ? toneOf(d.style.color) : null;
    const def = d?.name.startsWith("+") ? "success" : d?.name.startsWith("-") ? "danger" : null;
    return `metric ${q(l.name)} ${q(v.name)}${d ? " " + q(d.name) : ""}${tone && tone !== def ? " " + tone : ""}`;
  }

  heading(el: El): string {
    const lvl = el.tag.match(/^h([1-6])$/)?.[1];
    const n = lvl ? Math.min(3, Number(lvl)) : el.style.fs >= 20 ? 1 : el.style.fs >= 16 ? 2 : 3;
    return `h${n} ${this.plain(el.name)}`;
  }

  /** Text for a heading, text or badge line: bare unless it would be read as a modifier. */
  plain(s: string): string {
    const last = s.split(" ").pop() ?? "";
    return /^(primary|success|danger|warning|info|muted|disabled|enabled)$/.test(last) || last.startsWith("#") || !s || /(^|\s)(@-?[\d.]+,|\d+x\d|\w+:\S)/.test(s) || /"/.test(s) ? JSON.stringify(s) : s;
  }

  emit(el: El, indent: number, inRow = false): void {
    const t = el.tag, role = el.role;
    if (!el.name && !el.children.length && !el.svg && !el.dataUrl && !el.style.bg && !el.style.border) return;
    // paint: inline SVG, canvas elements
    if (el.svg) { this.canvasFromSvg(el, indent); return; }
    if (el.dataUrl) { this.space(indent, el.box.w, el.box.h, undefined, el); this.push(indent + 1, `image @0,0 ${fmt(el.box.w)}x${fmt(el.box.h)} ${this.src(el.dataUrl)} "Canvas"`); this.stats.painted++; return; }
    // semantic roles
    if (role === "heading") { this.push(indent, this.heading(el), el); this.stats.semantic++; return; }
    if (role === "button") {
      const tone = el.style.bg && isDark(el.style.bg) || saturated(el.style.bg) ? toneOf(el.style.bg) ?? "primary" : null;
      const name = el.name || el.attrs["aria-label"] || el.attrs.title || el.children.find((c) => c.name)?.name || "button";
      this.push(indent, `button ${q(name)}${tone ? " " + tone : ""}${el.attrs.disabled ? " disabled" : ""}`, el); this.stats.semantic++; return;
    }
    if (role === "link" && el.attrs.href !== undefined) {
      const href = el.attrs.href.replace(/\s+/g, "") || "#";
      const name = el.name || el.attrs["aria-label"] || el.attrs.title || href;
      this.push(indent, `link ${q(name)} ${href}`, el); this.stats.semantic++; return;
    }
    if (role === "textbox" || (t === "input" && ["text", "email", "date", "number", "search", "password", "tel", "url"].includes(el.attrs.type ?? "text"))) {
      const kind = ({ email: " email", date: " date", number: " number", search: " search", password: " password" } as Record<string, string>)[el.attrs.type ?? ""] ?? "";
      this.push(indent, `field ${q(el.name || "Field")}${kind}${el.attrs.value ? " =" + q(el.attrs.value) : ""}${el.attrs.required ? " required" : ""}`, el); this.stats.semantic++; return;
    }
    if (role === "combobox" && el.options) { this.push(indent, `select ${q(el.name || "Select")} ${el.options.map((o) => (o.selected ? "*" : "") + q(o.label)).join(" ")}`, el); this.stats.semantic++; return; }
    if (role === "checkbox") { this.push(indent, `check ${q(el.name || "Option")} ${el.attrs.checked ? "on" : "off"}`, el); this.stats.semantic++; return; }
    if (role === "switch") { this.push(indent, `toggle ${q(el.name || "Option")} ${el.attrs.checked ? "on" : "off"}`, el); this.stats.semantic++; return; }
    if (role === "slider") { this.push(indent, `slider ${q(el.name || "Value")} ${el.attrs.value ?? 0} ${el.attrs.min ?? 0}..${el.attrs.max ?? 100}`, el); this.stats.semantic++; return; }
    if (role === "img" && t === "img") {
      const src = this.src(el.attrs.src ?? "");
      if (!src) return;
      this.push(indent, `image ${/\s/.test(src) ? JSON.stringify(src) : src} ${JSON.stringify(el.attrs.alt?.trim() || el.attrs.title?.trim() || "image")}`, el); this.stats.semantic++; return;
    }
    if (role === "img" || (role === "figure" && !el.children.some((c) => c.role && c.role !== "generic"))) { this.demote(el, indent, el.attrs["aria-label"]); return; }
    if (t === "table") { this.table(el, indent); return; }
    if (role === "tablist") { this.tabs(el, indent); return; }
    if (role === "tabpanel") return;
    if (t === "label") {
      // a label wrapping its input: the input carries the label's name
      const input = el.children.find((c) => ["input", "select", "textarea"].includes(c.tag));
      if (input) { for (const c of el.children) if (c !== input && c.name && !this.isTextOnly(c)) this.emit(c, indent, inRow); this.emit(input, indent, inRow); for (const c of el.children) if (c !== input && c.name && this.isTextOnly(c) && c.name !== input.name && !input.name.includes(c.name)) this.push(indent, `text ${this.plain(c.name)}${toneOf(c.style.color) ? " " + toneOf(c.style.color) : ""}`, c); return; }
    }
    // text-only subtrees: badge, text
    if (this.isTextOnly(el) && el.name) {
      if (el.name.length <= 24 && el.style.radius >= 8 && el.style.bg) { this.push(indent, `badge ${this.plain(el.name)}${toneOf(el.style.color) ? " " + toneOf(el.style.color) : ""}`, el); this.stats.semantic++; return; }
      const tone = toneOf(el.style.color);
      this.push(indent, `text ${this.plain(el.name)}${tone ? " " + tone : ""}`, el); this.stats.semantic++; this.stats.textRuns++; return;
    }
    // a metric
    const m = this.asMetric(el);
    if (m) { this.push(indent, m, el); this.stats.semantic++; return; }
    // a painted subtree with no semantics: boxes with backgrounds and no readable structure
    const visibleKids = el.children.filter((c) => c.name || c.children.length || c.svg || c.style.bg);
    const paintedLeaves = visibleKids.filter((c) => !c.name && !c.children.length && (c.style.bg || c.style.border));
    if (visibleKids.length && paintedLeaves.length >= visibleKids.length / 2 && !visibleKids.some((c) => c.role && c.role !== "generic")) { this.demote(el, indent); return; }
    // containers: a bordered box is a card, side-by-side children are a row, and anything else is a sequence
    // (MX flows vertically by default, so wrapper divs and lists add no node of their own)
    const shown = el.children.filter((c) => c.name || c.children.length || c.svg || c.dataUrl || c.style.bg || c.style.border);
    const row = this.isRow(el) && shown.length > 1;
    const bordered = (el.style.border || (el.style.bg && el.style.bg !== "#ffffff")) && el.style.radius > 0 && !inRow && shown.length > 0;
    const first = shown[0];
    const title = bordered && first && first.role === "heading" && first.style.fs <= 17 ? first.name : null;
    const kids = title ? shown.slice(1) : shown;
    this.panels = kids.filter((c) => c.role === "tabpanel");
    if (bordered) { this.push(indent, `card${title ? " " + q(title) : ""}`, el); this.stats.semantic++; }
    else if (row) this.push(indent, "row", el);
    else if (inRow && kids.length > 1) this.push(indent, "col", el);
    else { for (const c of kids) this.emit(c, indent, inRow && kids.length === 1); return; }
    for (const c of kids) this.emit(c, indent + 1, row);
  }

  table(el: El, indent: number) {
    const rows = (x: El): El[] => x.tag === "tr" ? [x] : x.children.flatMap(rows);
    const trs = rows(el);
    if (!trs.length) return;
    const header = trs.find((r) => r.children.some((c) => c.tag === "th")) ?? trs[0];
    const cols = header.children.map((c) => c.name.split(" ")[0] || "Col");
    const body = trs.filter((r) => r !== header);
    const badgeCols = cols.map((_, i) => body.length > 0 && body.every((r) => r.children[i] && r.children[i].children.length === 1 && r.children[i].children[0].style.radius >= 8));
    this.push(indent, `table ${cols.map((c, i) => c + (badgeCols[i] ? ":badge" : "")).join(" ")}`, el);
    for (const r of body) this.push(indent + 1, r.children.map((c) => c.name.replace(/\|/g, "/")).join(" | "));
    this.stats.semantic += 1 + body.length;
  }

  /** Tab panels are siblings of the tablist in the page; they nest under the selected tab in MX. */
  panels: El[] = [];

  tabs(el: El, indent: number) {
    this.push(indent, "tabs", el);
    const panels = this.panels.splice(0);
    for (const tab of el.children.filter((c) => c.role === "tab")) {
      this.push(indent + 1, `tab ${tab.attrs.checked ? "*" : ""}${q(tab.name)}`, tab); this.stats.semantic++;
      if (tab.attrs.checked) for (const c of panels[0]?.children ?? []) this.emit(c, indent + 2);
    }
  }

  /** A subtree with no semantic reading: a sized region of rects and text at exact positions. */
  demote(el: El, indent: number, caption?: string) {
    const ox = el.box.x, oy = el.box.y;
    this.space(indent, el.box.w, el.box.h, caption, el);
    this.stats.demoted++;
    const walk = (x: El) => {
      const b = x.box;
      if (x.style.bg || x.style.border) {
        const words = [`rect @${fmt(b.x - ox)},${fmt(b.y - oy)} ${fmt(b.w)}x${fmt(b.h)}`];
        if (x.style.bg) words.push(`fill:${x.style.bg}`); else words.push("fill:none");
        if (x.style.border) words.push(`stroke:${x.style.border}`, `width:${fmt(x.style.borderW)}`);
        if (x.style.radius) words.push(`radius:${fmt(x.style.radius)}`);
        if (x.name && !x.children.length) words.push(qq(x.name));
        this.push(indent + 1, words.join(" "), x);
        this.stats.painted++;
      }
      if (x.text) {
        this.push(indent + 1, `text @${fmt(b.x - ox)},${fmt(b.y - oy + x.style.fs * 0.8)} ${qq(x.text)} size:${fmt(x.style.fs)}${x.style.fw >= 600 ? " weight:" + x.style.fw : ""}${x.style.color && x.style.color !== "#000000" ? " fill:" + x.style.color : ""}`, x);
        this.stats.painted++; this.stats.textRuns++;
      }
      for (const c of x.children) walk(c);
    };
    for (const c of el.children) walk(c);
  }

  /** A diagram: an SVG of labelled boxes joined by arrows is a `flow`, not paint. */
  asFlow(el: El, indent: number): boolean {
    const nodes: Array<{ label: string; tone: string | null; box: SvgNode["box"] }> = [];
    const paths: SvgNode[] = [];
    const texts: SvgNode[] = [];
    const walk = (n: SvgNode) => {
      if (n.tag === "g") {
        const shapes = n.children.filter((c) => ["rect", "circle", "ellipse"].includes(c.tag));
        const tx = n.children.filter((c) => c.tag === "text" && c.text);
        if (shapes.length === 1 && tx.length === 1) { nodes.push({ label: tx[0].text, tone: toneOf(shapes[0].attrs.stroke ?? "") ?? toneOf(shapes[0].attrs.fill ?? ""), box: n.box }); return; }
      }
      if (n.tag === "path" || n.tag === "line") paths.push(n);
      else if (n.tag === "text" && n.text) texts.push(n);
      for (const c of n.children) walk(c);
    };
    for (const c of el.svg ?? []) walk(c);
    if (nodes.length < 2 || paths.length < 1) return false;
    const ends = (n: SvgNode): [number, number, number, number] | null => {
      if (n.tag === "line") return [+n.attrs.x1, +n.attrs.y1, +n.attrs.x2, +n.attrs.y2];
      const d = (n.attrs.d ?? "").replace(/,/g, " ");
      const toks = d.match(/[MLHVZ]|-?[\d.]+/gi);
      if (!toks) return null;
      let x = 0, y = 0, sx = 0, sy = 0, cmd = "M", i = 0, first = true;
      while (i < toks.length) {
        const tk = toks[i];
        if (/^[a-z]$/i.test(tk)) { cmd = tk.toUpperCase(); i++; continue; }
        if (cmd === "M" || cmd === "L") { x = +toks[i]; y = +toks[i + 1]; i += 2; if (cmd === "M") cmd = "L"; }
        else if (cmd === "H") { x = +toks[i]; i++; } else if (cmd === "V") { y = +toks[i]; i++; } else break;
        if (first) { sx = x; sy = y; first = false; }
      }
      return [sx + n.tx, sy + n.ty, x + n.tx, y + n.ty];
    };
    const near = (px: number, py: number) => { let best = -1, bd = 1e9; nodes.forEach((n, i) => { const b = n.box; const dx = Math.max(b.x - px, 0, px - (b.x + b.w)), dy = Math.max(b.y - py, 0, py - (b.y + b.h)); const d = Math.hypot(dx, dy); if (d < bd) { bd = d; best = i; } }); return bd <= 6 ? best : -1; };
    const edges: Array<{ from: number; to: number; label: string | null; dashed: boolean; back: boolean }> = [];
    for (const p of paths) {
      const e = ends(p);
      if (!e) continue;
      const from = near(e[0], e[1]), to = near(e[2], e[3]);
      if (from < 0 || to < 0 || from === to) continue;
      // the label is the free text nearest the arrow (distance from the text's centre to the arrow's box, within 24px)
      const pb = p.box;
      let label: string | null = null, bd = 24;
      for (const t of texts) {
        const cx = t.box.x + t.box.w / 2, cy = t.box.y + t.box.h / 2;
        const d = Math.hypot(Math.max(pb.x - cx, 0, cx - (pb.x + pb.w)), Math.max(pb.y - cy, 0, cy - (pb.y + pb.h)));
        if (d < bd) { bd = d; label = t.text; }
      }
      edges.push({ from, to, label, dashed: !!p.attrs["stroke-dasharray"], back: nodes[to].box.x < nodes[from].box.x });
    }
    if (edges.length < nodes.length - 1) return false;
    this.push(indent, `flow${el.name ? " " + q(el.name) : ""}`, el);
    const seen = new Set<number>();
    const name = (i: number) => { const n = nodes[i]; const s = q(n.label) + (n.tone && !seen.has(i) ? ":" + n.tone : ""); seen.add(i); return s; };
    // forward chains first, one line per run of edges, then the back edges below
    const used = new Set<number>();
    const forward = edges.filter((e) => !e.back), back = edges.filter((e) => e.back);
    for (let i = 0; i < forward.length; i++) {
      if (used.has(i)) continue;
      let e = forward[i]; used.add(i);
      let line = `${name(e.from)} >${e.label ? q(e.label) : ""} ${name(e.to)}`;
      let cur = e.to;
      for (;;) { const j = forward.findIndex((x, k) => !used.has(k) && x.from === cur && x.dashed === e.dashed); if (j < 0) break; used.add(j); e = forward[j]; line += ` >${e.label ? q(e.label) : ""} ${name(e.to)}`; cur = e.to; }
      this.push(indent + 1, line + (e.dashed ? " dashed" : ""));
    }
    for (const e of back) this.push(indent + 1, `${name(e.from)} >${e.label ? q(e.label) : ""} ${name(e.to)} below${e.dashed ? " dashed" : ""}`);
    this.stats.semantic += 1 + nodes.length;
    return true;
  }

  /** Inline SVG: primitives one to one, transforms baked in. */
  canvasFromSvg(el: El, indent: number) {
    if (this.asFlow(el, indent)) return;
    this.space(indent, el.box.w, el.box.h, el.name || undefined, el);
    const color = (v?: string) => (v === undefined || v === "none" ? "none" : v.startsWith("#") ? v : v.startsWith("rgb") ? "#" + v.match(/[\d.]+/g)!.slice(0, 3).map((x) => Math.round(Number(x)).toString(16).padStart(2, "0")).join("") : v);
    const num = (v: string | undefined, d = 0) => (v === undefined ? d : parseFloat(v));
    const style = (a: Record<string, string>, isText = false): string => {
      const w: string[] = [];
      const fill = color(a.fill), stroke = color(a.stroke);
      if (isText) { if (fill !== "none" && fill !== "#000000") w.push(`fill:${fill}`); if (a["font-size"]) w.push(`size:${fmt(num(a["font-size"]))}`); const fw = num(a["font-weight"], 400); if (fw >= 600) w.push(`weight:${fw}`); if (a["text-anchor"] && a["text-anchor"] !== "start") w.push(`anchor:${a["text-anchor"]}`); return w.join(" "); }
      if (fill !== "none" && fill !== "#000000") w.push(`fill:${fill}`); else if (fill === "none" && stroke !== "none") w.push("fill:none");
      if (stroke !== "none") { w.push(`stroke:${stroke}`); const sw = num(a["stroke-width"], 1); if (sw !== 1) w.push(`width:${fmt(sw)}`); }
      if (a["stroke-dasharray"]) w.push("dashed");
      if (a.rx) w.push(`radius:${fmt(num(a.rx))}`);
      if (a.opacity && num(a.opacity) < 1) w.push(`opacity:${a.opacity}`);
      return w.join(" ");
    };
    // a positioned node's children are written in its own space: `off` is the node's origin
    const off = { x: 0, y: 0 };
    const walk = (n: SvgNode, ind: number, label: string | null) => {
      const a = n.attrs, tx = n.tx - off.x, ty = n.ty - off.y, sc = n.sc;
      const p = (x: number, y: number) => `@${fmt(tx + x * sc)},${fmt(ty + y * sc)}`;
      const s = (x: number) => fmt(x * sc);
      const lab = label ? " " + qq(label) : "";
      let line: string | null = null;
      switch (n.tag) {
        case "rect": line = `rect ${p(num(a.x), num(a.y))} ${s(num(a.width))}x${s(num(a.height))} ${style(a)}${lab}`; break;
        case "circle": line = `circle ${p(num(a.cx), num(a.cy))} r:${s(num(a.r))} ${style(a)}${lab}`; break;
        case "ellipse": line = `circle ${p(num(a.cx), num(a.cy))} r:${s((num(a.rx) + num(a.ry)) / 2)} ${style(a)}${lab}`; break;
        case "line": line = `line ${p(num(a.x1), num(a.y1))} ${p(num(a.x2), num(a.y2))} ${style(a)}`; break;
        case "polyline": case "polygon": { const pts = (a.points ?? "").trim().split(/[\s,]+/).map(Number); let d = ""; for (let i = 0; i + 1 < pts.length; i += 2) d += (i ? " L " : "M ") + `${fmt(tx + pts[i] * sc)} ${fmt(ty + pts[i + 1] * sc)}`; if (n.tag === "polygon") d += " Z"; line = `path ${qq(d)} ${style(a)}${lab}`; break; }
        case "path": { const d = a.d ?? ""; if (tx === 0 && ty === 0 && sc === 1) line = `path ${qq(d.replace(/\s+/g, " ").trim())} ${style(a)}${lab}`; else { this.push(ind, `region @${fmt(tx)},${fmt(ty)}${sc !== 1 ? ` scale:${fmt(sc)}` : ""}`); this.push(ind + 1, `path ${qq(d.replace(/\s+/g, " ").trim())} ${style(a)}${lab}`); this.stats.painted++; return; } break; }
        case "text": if (n.text) line = `text ${p(num(a.x), num(a.y))} ${qq(n.text)} ${style(a, true)}`; break;
        case "image": line = `image ${p(num(a.x), num(a.y))} ${s(num(a.width))}x${s(num(a.height))} ${this.src(a.href ?? a["xlink:href"] ?? "")} ${qq(label || "image")}`; break;
        case "g": case "a": {
          const name = a["aria-label"] ?? null;
          const role = ({ button: "button", link: "link", group: "region", region: "region", figure: "figure", listitem: "item", img: "figure" } as Record<string, string>)[a.role ?? ""] ?? (name ? "region" : null);
          // a group that holds one shape and one text is a labelled item: the text names it
          const texts = n.children.filter((c) => c.tag === "text" && c.text);
          const shapes = n.children.filter((c) => ["rect", "circle", "ellipse", "path", "polygon"].includes(c.tag));
          const b = n.box;
          const inside = (kind: string, lab: string, size: boolean, kids: SvgNode[], labels: Array<string | null>) => {
            const saved = { ...off };
            this.push(ind, `${kind} ${qq(lab)} @${fmt(b.x - saved.x)},${fmt(b.y - saved.y)}${size ? ` ${fmt(b.w)}x${fmt(b.h)}` : ""}`, undefined);
            off.x = b.x; off.y = b.y;
            kids.forEach((c, i) => walk(c, ind + 1, labels[i] ?? null));
            off.x = saved.x; off.y = saved.y;
          };
          if (shapes.length === 1 && texts.length === 1 && n.children.length === 2 && !role) {
            inside("item", texts[0].text, false, [shapes[0], texts[0]], [texts[0].text, null]); return;
          }
          if (role) {
            inside(role, name ?? texts[0]?.text ?? role, ["button", "link"].includes(role), n.children, []); return;
          }
          for (const c of n.children) walk(c, ind, null);
          return;
        }
        default: for (const c of n.children) walk(c, ind, null); return;
      }
      if (line) { this.push(ind, line.replace(/\s+$/, "").replace(/  +/g, " ")); this.stats.painted++; if (n.tag === "text") this.stats.textRuns++; }
    };
    for (const c of el.svg ?? []) walk(c, indent + 1, null);
  }
}

export const USER_AGENT = "ManifestImporter/0.1 (+https://github.com/brunopgalvao/manifest)";

/** Import an already loaded page. */
export async function importLoaded(page: import("playwright").Page, id: string): Promise<{ mx: string; imp: Importer; tree: El }> {
  const tree: El = await page.evaluate(capture);
  const imp = new Importer();
  imp.prefix = id;
  imp.push(0, `@${id}`);
  imp.emit(tree, 0);
  return { mx: imp.lines.join("\n") + "\n", imp, tree };
}

export async function importPage(target: string, id = "page", opts: { width?: number; browser?: import("playwright").Browser } = {}): Promise<{ mx: string; imp: Importer; tree: El }> {
  const url = /^https?:/.test(target) ? target : pathToFileURL(resolve(target)).href;
  const browser = opts.browser ?? (await chromium.launch());
  const page = await browser.newPage({ viewport: { width: opts.width ?? 760, height: 900 }, deviceScaleFactor: 1, userAgent: USER_AGENT });
  try {
    await page.goto(url, { waitUntil: "load", timeout: 30000 });
    return await importLoaded(page, id);
  } finally {
    await page.close();
    if (!opts.browser) await browser.close();
  }
}

if (process.argv[1] && /import\.ts$/.test(process.argv[1])) {
  const args = process.argv.slice(2);
  const target = args.find((a) => !a.startsWith("--"))!;
  const opt = (k: string) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : undefined; };
  const { mx, imp, tree } = await importPage(target, opt("--id") ?? "page");
  const out = opt("--out");
  if (out) writeFileSync(out, mx); else process.stdout.write(mx);
  const js = opt("--json");
  if (js) writeFileSync(js, JSON.stringify({ tree, boxes: imp.boxes, stats: imp.stats }, null, 1));
  console.error(`import: ${imp.stats.semantic} semantic nodes, ${imp.stats.painted} painted, ${imp.stats.demoted} demoted subtrees, ${imp.stats.textRuns} text runs`);
}
