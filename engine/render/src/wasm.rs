//! WebAssembly surface: the host in the browser with no DOM and no bindgen. Plain C ABI, so the build is
//! `cargo build --target wasm32-unknown-unknown --release -p mx-render` and the JS glue is sixty lines
//! (site/manifest.js). Every call returns a result block: `u32 status` (0 ok, 1 error), `u32 len`, then
//! `len` bytes (text, PNG, or `u32 w, u32 h, rgba...` for pixels), freed with `mx_result_free`.
use crate::host::Host;
use crate::{Renderer, Theme};

#[no_mangle]
pub extern "C" fn mx_alloc(len: u32) -> *mut u8 {
    let mut v: Vec<u8> = Vec::with_capacity(len as usize);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` must come from `mx_alloc(len)`.
#[no_mangle]
pub unsafe extern "C" fn mx_dealloc(ptr: *mut u8, len: u32) {
    drop(Vec::from_raw_parts(ptr, 0, len as usize));
}

fn result(status: u32, bytes: &[u8]) -> *mut u8 {
    let mut v: Vec<u8> = Vec::with_capacity(8 + bytes.len());
    v.extend_from_slice(&status.to_le_bytes());
    v.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    v.extend_from_slice(bytes);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

fn ok(bytes: &[u8]) -> *mut u8 {
    result(0, bytes)
}

fn text<E: std::fmt::Display>(r: Result<String, E>) -> *mut u8 {
    match r {
        Ok(s) => result(0, s.as_bytes()),
        Err(e) => result(1, e.to_string().as_bytes()),
    }
}

/// # Safety
/// `ptr` must be a block returned by one of the `mx_*` calls, freed once.
#[no_mangle]
pub unsafe extern "C" fn mx_result_free(ptr: *mut u8) {
    let len = u32::from_le_bytes([*ptr.add(4), *ptr.add(5), *ptr.add(6), *ptr.add(7)]) as usize;
    drop(Vec::from_raw_parts(ptr, 8 + len, 8 + len));
}

unsafe fn s<'a>(ptr: *const u8, len: u32) -> &'a str {
    if ptr.is_null() || len == 0 {
        return "";
    }
    std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize))
}

unsafe fn h<'a>(host: *mut Host) -> &'a mut Host {
    &mut *host
}

#[no_mangle]
pub extern "C" fn mx_host_new(dark: u32) -> *mut Host {
    let mut r = Renderer::new();
    if dark != 0 {
        r.theme = Theme::dark();
    }
    Box::into_raw(Box::new(Host::new(r)))
}

/// # Safety
/// `host` from `mx_host_new`; every other pointer-length pair is readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn mx_host_free(host: *mut Host) {
    drop(Box::from_raw(host));
}

#[no_mangle]
pub unsafe extern "C" fn mx_set_dark(host: *mut Host, dark: u32) {
    h(host).renderer.theme = if dark != 0 { Theme::dark() } else { Theme::light() };
    // repaint what is on screen
    let ids: Vec<String> = h(host).entries.iter().map(|e| e.surface.id.clone()).collect();
    for id in ids {
        let _ = h(host).repaint(&id);
    }
}

/// Device pixels per CSS pixel for painting (window.devicePixelRatio). Repaints what is on screen.
#[no_mangle]
pub unsafe extern "C" fn mx_set_scale(host: *mut Host, scale: f32) {
    if scale > 0.0 && h(host).renderer.scale != scale {
        h(host).renderer.scale = scale;
        let ids: Vec<String> = h(host).entries.iter().map(|e| e.surface.id.clone()).collect();
        for id in ids {
            let _ = h(host).repaint(&id);
        }
    }
}

/// The whole screen: every surface and residue line in a column, `u32 w, u32 h, rgba`, plus the placed
/// list as text after the pixels is not included; use `mx_placed` for hit testing.
#[no_mangle]
pub unsafe extern "C" fn mx_compose(host: *mut Host, w: u32, hh: u32, scale: f32, scroll: f32) -> *mut u8 {
    let (pm, _) = h(host).compose(w, hh, scale, scroll);
    let mut v = Vec::with_capacity(8 + pm.data().len());
    v.extend_from_slice(&pm.width().to_le_bytes());
    v.extend_from_slice(&pm.height().to_le_bytes());
    v.extend_from_slice(pm.data());
    ok(&v)
}

/// What `mx_compose` placed, one per line: `id y w h live`.
#[no_mangle]
pub unsafe extern "C" fn mx_placed(host: *mut Host, w: u32, hh: u32, scale: f32, scroll: f32) -> *mut u8 {
    let (_, placed) = h(host).compose(w, hh, scale, scroll);
    let lines: Vec<String> = placed.iter().map(|p| format!("{} {} {} {} {}", p.id, p.y, p.w, p.h, p.live as u8)).collect();
    ok(lines.join("\n").as_bytes())
}

#[no_mangle]
pub unsafe extern "C" fn mx_add_image(host: *mut Host, name: *const u8, name_len: u32, data: *const u8, data_len: u32) -> *mut u8 {
    let bytes = std::slice::from_raw_parts(data, data_len as usize);
    text(h(host).renderer.add_image(s(name, name_len), bytes).map(|_| String::new()))
}

#[no_mangle]
pub unsafe extern "C" fn mx_render(host: *mut Host, mx: *const u8, len: u32) -> *mut u8 {
    text(h(host).render(s(mx, len)))
}

#[no_mangle]
pub unsafe extern "C" fn mx_patch(host: *mut Host, id: *const u8, id_len: u32, patch: *const u8, len: u32) -> *mut u8 {
    text(h(host).patch(s(id, id_len), s(patch, len)))
}

#[no_mangle]
pub unsafe extern "C" fn mx_observe(host: *mut Host, id: *const u8, id_len: u32, level: u32, since: i64, focus: *const u8, focus_len: u32) -> *mut u8 {
    let f = s(focus, focus_len);
    text(h(host).observe(s(id, id_len), level as u8, if since < 0 { None } else { Some(since as u32) }, if f.is_empty() { None } else { Some(f) }))
}

/// The parsed tree of a document as JSON (`{"id","type","label","args","flags","children":[...]}`), for tools
/// that need the structure without re-implementing the parser: the importer's scorer, the A2UI emitter.
#[no_mangle]
pub unsafe extern "C" fn mx_tree(mx: *const u8, len: u32) -> *mut u8 {
    match mx::parse(s(mx, len)) {
        Ok(surf) => {
            fn node(surf: &mx::Surface, n: usize) -> serde_json::Value {
                let nd = &surf.nodes[n];
                let mut v = serde_json::to_value(nd).unwrap_or(serde_json::Value::Null);
                v["type"] = serde_json::Value::String(nd.ty.clone());
                v["children"] = serde_json::Value::Array(nd.children.iter().map(|&c| node(surf, c)).collect());
                if nd.ty == "flow" {
                    v["nodes"] = serde_json::Value::Array(nd.args.nodes.iter().map(|&c| node(surf, c)).collect());
                }
                v
            }
            let root = node(&surf, surf.root());
            let out = serde_json::json!({ "id": surf.id, "life": surf.life, "rev": surf.rev, "root": root });
            ok(out.to_string().as_bytes())
        }
        Err(e) => result(1, e.to_string().as_bytes()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mx_list(host: *mut Host) -> *mut u8 {
    ok(h(host).list().as_bytes())
}

#[no_mangle]
pub unsafe extern "C" fn mx_drain(host: *mut Host) -> *mut u8 {
    ok(h(host).drain().join("\n").as_bytes())
}

#[no_mangle]
pub unsafe extern "C" fn mx_turn(host: *mut Host) {
    h(host).turn();
}

#[no_mangle]
pub unsafe extern "C" fn mx_dissolve(host: *mut Host, id: *const u8, id_len: u32, reason: *const u8, reason_len: u32) {
    h(host).dissolve(s(id, id_len), s(reason, reason_len));
}

#[no_mangle]
pub unsafe extern "C" fn mx_tick(host: *mut Host, now_ms: f64) {
    h(host).tick(now_ms as u64);
}

/// 1 while a tween runs: tick and repaint at frame rate.
#[no_mangle]
pub unsafe extern "C" fn mx_animating(host: *mut Host) -> u32 {
    h(host).animating() as u32
}

/// Premultiplied RGBA8 of a live surface: `u32 w, u32 h, pixels`. The surface is opaque, so the pixels
/// go straight into an ImageData.
#[no_mangle]
pub unsafe extern "C" fn mx_pixels(host: *mut Host, id: *const u8, id_len: u32) -> *mut u8 {
    match h(host).pixmap(s(id, id_len)) {
        Some(pm) => {
            let mut v = Vec::with_capacity(8 + pm.data().len());
            v.extend_from_slice(&pm.width().to_le_bytes());
            v.extend_from_slice(&pm.height().to_le_bytes());
            v.extend_from_slice(pm.data());
            ok(&v)
        }
        None => result(1, b"not on screen"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mx_look(host: *mut Host, id: *const u8, id_len: u32, node: *const u8, node_len: u32) -> *mut u8 {
    let n = s(node, node_len);
    match h(host).look(s(id, id_len), if n.is_empty() { None } else { Some(n) }) {
        Ok((png, _, _)) => ok(&png),
        Err(e) => result(1, e.to_string().as_bytes()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn mx_click(host: *mut Host, id: *const u8, id_len: u32, x: f32, y: f32) -> *mut u8 {
    ok(h(host).click(s(id, id_len), x, y).unwrap_or_default().as_bytes())
}

#[no_mangle]
pub unsafe extern "C" fn mx_key(host: *mut Host, id: *const u8, id_len: u32, t: *const u8, t_len: u32) -> *mut u8 {
    ok(h(host).key(s(id, id_len), s(t, t_len)).unwrap_or_default().as_bytes())
}

/// The kind of interactive item under a point (`button`, `field`, ...), or empty: for the cursor.
#[no_mangle]
pub unsafe extern "C" fn mx_hit(host: *mut Host, id: *const u8, id_len: u32, x: f32, y: f32) -> *mut u8 {
    use crate::layout::Hit;
    let kind = match h(host).hit(s(id, id_len), x, y).map(|(hit, _)| hit) {
        Some(Hit::Button(_)) => "button",
        Some(Hit::Check(_)) => "check",
        Some(Hit::Toggle(_)) => "toggle",
        Some(Hit::Tab(..)) => "tab",
        Some(Hit::Slider(..)) => "slider",
        Some(Hit::Field(_)) => "field",
        Some(Hit::Select(_)) => "select",
        Some(Hit::Link(..)) => "link",
        _ => "",
    };
    ok(kind.as_bytes())
}

#[no_mangle]
pub unsafe extern "C" fn mx_editing(host: *mut Host, id: *const u8, id_len: u32) -> *mut u8 {
    ok(h(host).editing(s(id, id_len)).unwrap_or("").as_bytes())
}
