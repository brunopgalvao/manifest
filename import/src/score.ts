/**
 * Scores an import against a hand-written MX reference and against the page it came from:
 *   recall    reference nodes (type + label) the import recovered
 *   text      visible text runs of the page that appear in the import
 *   roundtrip imported nodes whose rendered size (engine) is within 20% or 8px of the source box, heights
 *             and widths apart (widths follow Manifest's column, not the page's CSS)
 *
 *   node import/src/score.ts <imported.mx> <reference.mx> <import.json> [--boxes engine-boxes.json]
 *
 * Parsing is the engine's (the wasm build), so there is one implementation of the format.
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadManifest } from "../../web/manifest.js";

export type Tree = { id: string; root: TreeNode };
export type TreeNode = { type: string; id: string; label: string | null; args: Record<string, any>; flags: string[]; children: TreeNode[]; nodes?: TreeNode[] };

let engine: Awaited<ReturnType<typeof loadManifest>> | null = null;
export async function parse(mx: string): Promise<Tree> {
  engine ??= await loadManifest(readFileSync(fileURLToPath(new URL("../../engine/target/wasm32-unknown-unknown/release/mx_render.wasm", import.meta.url))));
  return engine.tree(mx);
}

export function nodes(t: Tree): TreeNode[] {
  const out: TreeNode[] = [];
  const walk = (n: TreeNode) => { for (const c of n.children) { out.push(c); walk(c); } for (const x of n.nodes ?? []) out.push(x); };
  walk(t.root);
  return out;
}

const slug = (s: string | null) => { const first = (s ?? "").split(" ")[0].toLowerCase().replace(/[^a-z0-9]/g, ""); return first || null; };

/** Reference nodes matched by type and label (explicit ids in the reference are an author's choice, not a fact of the page). */
export function recall(imported: Tree, reference: Tree): { hit: number; total: number; missing: string[] } {
  const key = (t: Tree) => nodes(t).filter((n) => !["row", "col"].includes(n.type)).map((n) => `${n.type} ${slug(n.label) ?? ""}`);
  const have = new Set(key(imported));
  const want = key(reference);
  const missing = want.filter((k) => !have.has(k));
  return { hit: want.length - missing.length, total: want.length, missing };
}

export function textCoverage(mx: string, runs: string[]): { hit: number; total: number } {
  const norm = (s: string) => s.replace(/\s+/g, " ").trim().toLowerCase();
  const body = norm(mx);
  const all = runs.map(norm).filter(Boolean);
  return { hit: all.filter((r) => body.includes(r)).length, total: all.length };
}

export function roundTrip(srcBoxes: Array<{ line: number; box: { w: number; h: number } }>, engineBoxes: Record<string, { w: number; h: number }>, ids: Map<number, string>): { heights: number; widths: number; total: number } {
  let heights = 0, widths = 0, total = 0;
  for (const b of srcBoxes) {
    const id = ids.get(b.line);
    const e = id ? engineBoxes[id] : undefined;
    if (!e) continue;
    total++;
    const ok = (a: number, c: number) => Math.abs(a - c) <= Math.max(8, 0.2 * a);
    if (ok(b.box.h, e.h)) heights++;
    if (ok(b.box.w, e.w)) widths++;
  }
  return { heights, widths, total };
}

/** Emitted line number -> the id the parser gave that line (nodes are created in document order). */
export function idsByLine(mx: string, t: Tree): Map<number, string> {
  const out = new Map<number, string>();
  const lines = mx.split("\n");
  const order = nodes(t).filter((n) => n.type !== "node");
  let k = 0;
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    if (!l.trim() || l.startsWith("@") || /^\s+\S+ \| /.test(l)) continue;
    const n = order[k++];
    if (n) out.set(i, n.id);
  }
  return out;
}

export function textRuns(tree: any): string[] {
  const runs: string[] = [];
  const walkSvg = (s: any) => { if (s.text) runs.push(s.text); for (const c of s.children ?? []) walkSvg(c); };
  const walk = (e: any) => { if (e.text) runs.push(e.text); for (const c of e.children ?? []) walk(c); if (e.svg) for (const s of e.svg) walkSvg(s); };
  walk(tree);
  return runs;
}

if (process.argv[1] && /score\.ts$/.test(process.argv[1])) {
  const [mxFile, refFile, jsonFile] = process.argv.slice(2);
  const mx = readFileSync(mxFile, "utf8");
  const ref = await parse(readFileSync(refFile, "utf8"));
  const imp = await parse(mx);
  const r = recall(imp, ref);
  const meta = JSON.parse(readFileSync(jsonFile, "utf8"));
  const tc = textCoverage(mx, textRuns(meta.tree));
  const bi = process.argv.indexOf("--boxes");
  let rt = { heights: 0, widths: 0, total: 0 };
  if (bi > 0) rt = roundTrip(meta.boxes, JSON.parse(readFileSync(process.argv[bi + 1], "utf8")).boxes, idsByLine(mx, imp));
  console.log(JSON.stringify({ recall: r, text: tc, roundtrip: rt }));
}
