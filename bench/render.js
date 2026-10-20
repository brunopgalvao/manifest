// Render each corpus HTML file in Chromium and capture what each perception
// form would give a model: a screenshot, Playwright's ARIA snapshot (what
// browser agents get today), and the real layout boxes for every data-mx node,
// which stand in for a native Manifest renderer's layout.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const CORPUS = path.join(__dirname, 'corpus');
const OUT = path.join(__dirname, 'out');
for (const d of ['shots', 'aria', 'boxes']) fs.mkdirSync(path.join(OUT, d), { recursive: true });

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 720, height: 480 }, deviceScaleFactor: 1 });
  for (const f of fs.readdirSync(CORPUS).filter(f => f.endsWith('.html'))) {
    const name = f.replace('.html', '');
    await page.goto('file://' + path.join(CORPUS, f));
    const size = await page.evaluate(() => ({ w: document.documentElement.scrollWidth, h: document.documentElement.scrollHeight }));
    await page.screenshot({ path: path.join(OUT, 'shots', name + '.full.png'), fullPage: true });
    await page.locator('[data-mx=root]').screenshot({ path: path.join(OUT, 'shots', name + '.png') });
    fs.writeFileSync(path.join(OUT, 'aria', name + '.txt'), await page.locator('body').ariaSnapshot());
    const boxes = await page.evaluate(() => {
      const out = {};
      for (const el of document.querySelectorAll('[data-mx]')) {
        const r = el.getBoundingClientRect();
        out[el.dataset.mx] = {
          x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height),
          visible: el.checkVisibility(),
          offscreen: r.right < 0 || r.bottom < 0 || r.left > innerWidth,
        };
      }
      // Renderer diagnostics a native renderer would emit: truncated text.
      const diags = [];
      for (const el of document.querySelectorAll('*')) {
        const cs = getComputedStyle(el);
        if (el.children.length === 0 && el.scrollWidth > el.clientWidth + 1 && cs.overflow !== 'visible' && el.checkVisibility()) {
          const owner = el.closest('[data-mx]');
          const row = el.closest('tr'), cell = el.closest('td');
          diags.push({ kind: 'overflow', node: owner && owner.dataset.mx, text: el.textContent.trim(),
            row: row ? [...row.parentNode.children].indexOf(row) : null,
            col: cell ? [...cell.parentNode.children].indexOf(cell) : null,
            shown: el.clientWidth, needed: el.scrollWidth });
        }
      }
      return { boxes: out, diags };
    });
    boxes.size = size;
    fs.writeFileSync(path.join(OUT, 'boxes', name + '.json'), JSON.stringify(boxes, null, 1));
    console.log(name, size, Object.keys(boxes.boxes).length, 'nodes', boxes.diags.length, 'diags');
  }
  await browser.close();
})();
