import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { importPage } from "../src/import.ts";
import { parse, recall, textCoverage, textRuns } from "../src/score.ts";

// The importer on the benchmark's HTML twins: every import parses in the engine, recovers the hand-written
// nodes (except what the page does not show: a hidden tab's content, a bar chart drawn with divs, which comes
// back as paint, a caption the author chose), and keeps every visible text run.
const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(here, "../../bench/corpus");

const EXPECT: Record<string, { recall: number; missing: string[] }> = {
  dash: { recall: 6, missing: ["bar monthly"] },
  form: { recall: 7, missing: [] },
  flow: { recall: 5, missing: ["flow agent"] },
  table: { recall: 6, missing: [] },
  confirm: { recall: 5, missing: [] },
  settings: { recall: 8, missing: ["toggle dark"] },
};

for (const [name, want] of Object.entries(EXPECT)) {
  test(`imports ${name}`, async () => {
    const { mx, imp, tree } = await importPage(join(corpus, name + ".html"), name);
    const s = await parse(mx);
    const ref = await parse(readFileSync(join(corpus, name + ".mx"), "utf8"));
    const r = recall(s, ref);
    assert.deepEqual(r.missing, want.missing, mx);
    assert.equal(r.hit, want.recall);
    const runs = textRuns(tree);
    const tc = textCoverage(mx, runs);
    assert.equal(tc.hit, tc.total, `text runs lost: ${runs.filter((x) => !mx.toLowerCase().includes(x.toLowerCase())).join(" | ")}`);
    assert.ok(imp.stats.semantic > 0);
  });
}

test("a painted chart comes back as a sized region with its bars and labels", async () => {
  const { mx } = await importPage(join(corpus, "dash.html"), "dash");
  assert.match(mx, /^region "Monthly revenue, Apr to Sep: 38, 42, 51, 47, 57, 73 thousand dollars" 688x\d+$/m);
  assert.equal((mx.match(/^  rect @/gm) ?? []).length, 6);
  assert.match(mx, /^  text @[\d.]+,[\d.]+ "Sep" size:12/m);
  assert.match(mx, /^  metric Revenue \$308k "\+14%"$/m);
  assert.match(mx, /^  button "Export CSV" disabled$/m);
});

test("an SVG diagram of boxes and arrows is a flow", async () => {
  const { mx } = await importPage(join(corpus, "flow.html"), "flow");
  assert.match(mx, /^flow "Pipeline[^"]*"\n  Agent:primary >writes MX:primary >draws Renderer:primary >reads Perception:success\n  Perception >"tree, boxes, diagnostics" Agent below dashed$/m);
});

test("an SVG with roles becomes positioned nodes that take input", async () => {
  const { mx } = await importPage(join(here, "../../bench/corpus/v2html/funnel.html"), "funnel");
  assert.match(mx, /^  button "Export PNG" @520,272 100x32\n    rect @0,0 100x32 fill:#2f5bea radius:16$/m);
  assert.match(mx, /^  region "Funnel" @40,20\n    rect @0,0 560x48/m);
  const t = await parse(mx);
  assert.equal(t.id, "funnel");
});
