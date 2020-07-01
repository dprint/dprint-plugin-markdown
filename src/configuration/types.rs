use serde::{Serialize, Deserialize};
use dprint_core::generate_str_to_from;
use dprint_core::configuration::*;

/// Resolved markdown configuration.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    pub line_width: u32,
    pub new_line_kind: NewLineKind,
    pub text_wrap: TextWrap,
}

/// Text wrapping possibilities.
#[derive(Clone, PartialEq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextWrap {
    /// Always wraps text.
    Always,
    /// Maintains line breaks (default).
    Maintain,
    /// Never wraps text.
    Never,
}

generate_str_to_from![
    TextWrap,
    [Always, "always"],
    [Maintain, "maintain"],
    [Never, "never"]
];
