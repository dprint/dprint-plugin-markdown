extern crate dprint_core;
extern crate pulldown_cmark;
extern crate regex;

pub mod configuration;
mod format_text;
mod generation;

pub use format_text::format_text;

#[cfg(feature = "tracing")]
pub use format_text::trace_file;

#[cfg(feature = "wasm")]
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod wasm_plugin;
#[cfg(feature = "wasm")]
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use wasm_plugin::*;
