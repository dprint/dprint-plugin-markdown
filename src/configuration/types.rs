use dprint_core::configuration::*;
use dprint_core::generate_str_to_from;
use serde::Deserialize;
use serde::Serialize;

/// Resolved markdown configuration.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
  pub line_width: u32,
  pub new_line_kind: NewLineKind,
  pub text_wrap: TextWrap,
  pub emphasis_kind: EmphasisKind,
  pub strong_kind: StrongKind,
  pub ignore_directive: String,
  pub ignore_file_directive: String,
  pub ignore_start_directive: String,
  pub ignore_end_directive: String,
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

generate_str_to_from![TextWrap, [Always, "always"], [Maintain, "maintain"], [Never, "never"]];

/// The character to use for emphasis/italics.
#[derive(Clone, PartialEq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmphasisKind {
  /// Uses asterisks (*) for emphasis.
  Asterisks,
  /// Uses underscores (_) for emphasis (default).
  Underscores,
}

generate_str_to_from![EmphasisKind, [Asterisks, "asterisks"], [Underscores, "underscores"]];

/// The character to use for strong emphasis/bold.
#[derive(Clone, PartialEq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StrongKind {
  /// Uses asterisks (**) for strong emphasis (default).
  Asterisks,
  /// Uses underscores (__) for strong emphasis.
  Underscores,
}

generate_str_to_from![StrongKind, [Asterisks, "asterisks"], [Underscores, "underscores"]];
