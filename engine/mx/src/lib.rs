//! Manifest MX v1.2 core in Rust: parse, derive ids, perceive, patch, diff. No I/O.
//! A port of `packages/core/src/index.ts`; the parity test in `tests/parity.rs` keeps them in step on
//! the same fixtures. Nodes live in an arena (`Surface::nodes`) and refer to each other by index, which
//! is what gives the id rules and perception a stable notion of identity.

pub mod model;
pub mod parse;
pub mod patch;
pub mod perceive;

pub use model::{Args, Box, Diag, Edge, MXError, Node, Surface, Value};
pub use parse::{badge_tone, metric_default_tone, parse, parse_node, slug, unq, words};
pub use patch::{apply, data_model, diff, find_parent, fmt_value, parse_patch_head, primary_value, residue_line, resolve, value_json};
pub use perceive::{derive_all, format_diag, hidden_summary, line_of, perceive, q};
