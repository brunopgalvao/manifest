/**
 * The dataset runner: every URL in a list becomes a page record, so the importer can be measured and its
 * output used as training data.
 *
 *   node import/src/batch.ts bench/pages/urls.txt bench/pages/out [--limit N] [--only slug]
 *
 * Per page: <slug>.mx (the import), <slug>.html (the rendered DOM as a snapshot), <slug>.png (a screenshot of
 * what was imported), <slug>.json (the captured tree with boxes, the importer's stats, the scores), and one
 * line in index.jsonl. Pages are fetched one at a time with an identified user agent, at a desktop width,
 * with consent banners declined when a decline control exists.
 */
import { chromium } from "playwright";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync, appendFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { importLoaded, USER_AGENT } from "./import.ts";
import { parse, textCoverage, textRuns, roundTrip, idsByLine, nodes } from "./score.ts";

const [listFile, outDir] = process.argv.slice(2);
const opt = (k: string) => { const i = process.argv.indexOf(k); return i >= 0 ? process.argv[i + 1] : undefined; };
const limit = Number(opt("--limit") ?? 0), only = opt("--only");
mkdirSync(outDir, { recursive: true });
const mxr = fileURLToPath(new URL("../../engine/target/release/mxr", import.meta.url));

const slugOf = (url: string) => url.replace(/^https?:\/\//, "").replace(/[^a-z0-9]+/gi, "-").replace(/^-|-$/g, "").toLowerCase().slice(0, 80);
const lines = readFileSync(listFile, "utf8").split("\n").filter((l) => l.trim() && !l.startsWith("#"));
const browser = await chromium.launch();
const index = join(outDir, "index.jsonl");
let done = 0;
for (const line of lines) {
  const [url, source, licence] = line.split("\t");
  const slug = slugOf(url);
  if (only && !slug.includes(only)) continue;
  if (limit && done >= limit) break;
  done++;
  const rec: Record<string, unknown> = { url, slug, source, licence };
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1, userAgent: USER_AGENT });
  const t0 = Date.now();
  try {
    await page.goto(url, { waitUntil: "load", timeout: 45000 });
    await page.waitForTimeout(800);
    // consent banners: decline when a decline control exists, otherwise leave them
    for (const label of [/reject all/i, /decline/i, /necessary only/i, /only essential/i, /reject/i]) {
      const b = page.getByRole("button", { name: label }).first();
      if (await b.count() && await b.isVisible().catch(() => false)) { await b.click({ timeout: 2000 }).catch(() => {}); await page.waitForTimeout(300); break; }
    }
    const html = await page.content();
    await page.screenshot({ path: join(outDir, slug + ".png"), fullPage: false });
    const { mx, imp, tree } = await importLoaded(page, slug.replace(/-/g, "_").slice(0, 40));
    writeFileSync(join(outDir, slug + ".mx"), mx);
    writeFileSync(join(outDir, slug + ".html"), html);
    for (const a of imp.assets) writeFileSync(join(outDir, a.name), a.bytes);
    rec.ms = Date.now() - t0;
    rec.stats = imp.stats;
    rec.lines = mx.split("\n").length - 1;
    rec.htmlBytes = html.length;
    rec.mxBytes = mx.length;
    // scores: does it parse, how much text survives, how do the engine's boxes compare
    let parsed = null as Awaited<ReturnType<typeof parse>> | null;
    try { parsed = await parse(mx); rec.parse = "ok"; } catch (e) { rec.parse = String((e as Error).message); }
    const runs = textRuns(tree);
    rec.text = textCoverage(mx, runs);
    if (parsed) {
      const ns = nodes(parsed);
      rec.nodes = ns.length;
      rec.kinds = ns.reduce((m: Record<string, number>, n) => { m[n.type] = (m[n.type] ?? 0) + 1; return m; }, {});
      try {
        const boxes = JSON.parse(execFileSync(mxr, ["boxes", resolve(outDir, slug + ".mx")], { encoding: "utf8", maxBuffer: 64 << 20 })).boxes;
        rec.roundtrip = roundTrip(imp.boxes, boxes, idsByLine(mx, parsed));
      } catch (e) { rec.roundtrip = String((e as Error).message).split("\n")[0]; }
    }
    writeFileSync(join(outDir, slug + ".json"), JSON.stringify({ url, tree, boxes: imp.boxes, stats: imp.stats }));
    console.log(`${slug}: ${rec.parse} ${rec.lines} lines, text ${JSON.stringify(rec.text)}, ${imp.stats.semantic} semantic / ${imp.stats.painted} painted / ${imp.stats.demoted} demoted`);
  } catch (e) {
    rec.error = String((e as Error).message).split("\n")[0];
    console.log(`${slug}: ERROR ${rec.error}`);
  } finally {
    await page.close();
  }
  appendFileSync(index, JSON.stringify(rec) + "\n");
}
await browser.close();
