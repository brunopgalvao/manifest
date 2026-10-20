// The Manifest engine in JavaScript: the Rust renderer as WebAssembly with a plain C ABI, no bindgen.
// Browser:  const m = await loadManifest("mx_render.wasm");
// Node:     const m = await loadManifest(fs.readFileSync("engine/target/wasm32-unknown-unknown/release/mx_render.wasm"));
// Then m.render(mx), m.observe(id, 2), m.compose(w, h, dpr), m.click(id, x, y), m.tree(mx) ...
export async function loadManifest(source, { dark = false } = {}) {
  let module;
  if (typeof source === "string") {
    module = (await WebAssembly.instantiateStreaming(fetch(source), {})).instance;
  } else {
    module = (await WebAssembly.instantiate(source, {})).instance;
  }
  const ex = module.exports;
  const enc = new TextEncoder(), dec = new TextDecoder();
  const host = ex.mx_host_new(dark ? 1 : 0);
  const mem = () => new Uint8Array(ex.memory.buffer);

  // Strings go in through mx_alloc; results come back as [u32 status][u32 len][bytes] and are freed here.
  function put(s) {
    const b = enc.encode(s);
    const p = ex.mx_alloc(b.length);
    if (b.length) mem().set(b, p);
    return [p, b.length];
  }
  function take(ptr) {
    const head = new Uint32Array(ex.memory.buffer, ptr, 2);
    const status = head[0], len = head[1];
    const bytes = mem().slice(ptr + 8, ptr + 8 + len);
    ex.mx_result_free(ptr);
    return [status, bytes];
  }
  function text(ptr) {
    const [status, bytes] = take(ptr);
    const t = dec.decode(bytes);
    if (status) throw new Error(t);
    return t;
  }
  function call(strs, fn) {
    const args = strs.map(put);
    try { return fn(...args.flat()); } finally { for (const [p, l] of args) ex.mx_dealloc(p, l); }
  }
  function rgba(ptr) {
    const [status, bytes] = take(ptr);
    if (status) throw new Error(dec.decode(bytes));
    const dv = new DataView(bytes.buffer);
    const w = dv.getUint32(0, true), h = dv.getUint32(4, true);
    const pixels = new Uint8ClampedArray(bytes.buffer, 8, w * h * 4);
    return typeof ImageData !== "undefined" ? new ImageData(pixels, w, h) : { width: w, height: h, data: pixels };
  }

  return {
    /** The parsed tree of a document, without rendering: { id, life, rev, root: { type, id, label, args, flags, children } }. */
    tree: (mx) => call([mx], (p, l) => JSON.parse(text(ex.mx_tree(p, l)))),
    render: (mx) => call([mx], (p, l) => text(ex.mx_render(host, p, l))),
    patch: (id, patch) => call([id, patch], (a, al, b, bl) => text(ex.mx_patch(host, a, al, b, bl))),
    observe: (id, level = 1, since = -1, focus = "") => call([id, focus], (a, al, f, fl) => text(ex.mx_observe(host, a, al, level, BigInt(since ?? -1), f, fl))),
    list: () => text(ex.mx_list(host)),
    drain: () => { const t = text(ex.mx_drain(host)); return t ? t.split("\n") : []; },
    turn: () => ex.mx_turn(host),
    dissolve: (id, reason = "model") => call([id, reason], (a, al, b, bl) => ex.mx_dissolve(host, a, al, b, bl)),
    tick: (now = typeof performance !== "undefined" ? performance.now() : Date.now()) => ex.mx_tick(host, now),
    animating: () => ex.mx_animating(host) === 1,
    setDark: (dark) => ex.mx_set_dark(host, dark ? 1 : 0),
    setScale: (scale) => ex.mx_set_scale(host, scale),
    addImage: (name, bytes) => call([name], (a, al) => { const p = ex.mx_alloc(bytes.length); mem().set(bytes, p); try { return text(ex.mx_add_image(host, a, al, p, bytes.length)); } finally { ex.mx_dealloc(p, bytes.length); } }),
    /** Pixels of one surface. */
    pixels: (id) => call([id], (a, al) => rgba(ex.mx_pixels(host, a, al))),
    /** PNG bytes of one node (or the surface). */
    look: (id, node = "") => call([id, node], (a, al, b, bl) => { const [s, bytes] = take(ex.mx_look(host, a, al, b, bl)); if (s) throw new Error(dec.decode(bytes)); return bytes; }),
    click: (id, x, y) => call([id], (a, al) => text(ex.mx_click(host, a, al, x, y))),
    key: (id, t) => call([id, t], (a, al, b, bl) => text(ex.mx_key(host, a, al, b, bl))),
    hit: (id, x, y) => call([id], (a, al) => text(ex.mx_hit(host, a, al, x, y))),
    editing: (id) => call([id], (a, al) => text(ex.mx_editing(host, a, al))),
    /** The whole screen (every surface in a column) as pixels, and where each surface sits. */
    compose: (w, h, scale, scroll = 0) => {
      const img = rgba(ex.mx_compose(host, w, h, scale, scroll));
      const placed = text(ex.mx_placed(host, w, h, scale, scroll)).split("\n").filter(Boolean)
        .map((l) => { const [id, y, pw, ph, live] = l.split(" "); return { id, y: +y, w: +pw, h: +ph, live: live === "1" }; });
      return { img, placed };
    },
    columnX: (cssW) => (cssW - Math.min(760, cssW)) / 2 + 16,
  };
}
