//! manifest-mcp: the Manifest MCP server on the Rust renderer. No browser, no Node.
//!   manifest-mcp              a native window shows the surfaces; the model talks over stdio
//!   manifest-mcp --headless   no window (also MANIFEST_HEADLESS=1): layout, perception and look still work
//!   --dark                    paint with the dark palette (also MANIFEST_DARK=1)
mod mcp;
mod window;

use mx_render::host::Host;
use mx_render::{Renderer, Theme};
use std::sync::{Arc, Mutex};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str, env: &str| args.iter().any(|a| a == name) || std::env::var(env).map(|v| v == "1").unwrap_or(false);
    let headless = flag("--headless", "MANIFEST_HEADLESS");
    let dark = flag("--dark", "MANIFEST_DARK");
    let mut r = Renderer::new();
    if dark {
        r.theme = Theme::dark();
    }
    let host = Arc::new(Mutex::new(Host::new(r)));
    if headless {
        mcp::serve(host, None);
    } else {
        window::run(host);
    }
}
