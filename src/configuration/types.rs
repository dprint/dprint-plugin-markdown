use serde::{Serialize, Deserialize};
use dprint_core::configuration::*;

/// Resolved markdown configuration.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    pub line_width: u32,
    pub new_line_kind: NewLineKind,
}
