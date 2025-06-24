use dprint_core::configuration::*;
use dprint_core::generate_str_to_from;
use serde::Deserialize;
use serde::Serialize;

/// Resolved markdown configuration.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
  pub line_width: u32,
  pub indent_width: u32,
  pub new_line_kind: NewLineKind,
  pub text_wrap: TextWrap,
  pub emphasis_kind: EmphasisKind,
  pub strong_kind: StrongKind,
  pub unordered_list_kind: UnorderedListKind,
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

/// The character to use primarily for lists.
///
/// Unnumbered lists will be formatted to use a common list character, i.e., the primary list
/// character. Additionally, an alternate list character is used to separate lists which are not
/// separated by other paragraphs. This parameter defines which character should be used as primary
/// list character, i.e., either '-' (default) or '*'. The alternate list character will be the one
/// which is _not_ primary.
#[derive(Clone, PartialEq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnorderedListKind {
  /// Uses dashes (-) as primary character for lists (default).
  ///
  /// In this case, asterisks are used as alternate list chracters.
  Dashes,
  /// Uses asterisks (*) as primary character for lists.
  ///
  /// In this case, dashes are used as alternate list chracters.
  Asterisks,
}

impl UnorderedListKind {
  /// Determine the character to use for a list, i.e., '-' or '*'.
  ///
  /// The result depends on the configuration and whether the primary or alternate character is
  /// requested. See [`Self`].
  pub fn list_char(&self, is_alternate: bool) -> char {
    match (self, is_alternate) {
      (Self::Dashes, true) | (Self::Asterisks, false) => '*',
      _ => '-',
    }
  }
}

generate_str_to_from![UnorderedListKind, [Dashes, "dashes"], [Asterisks, "asterisks"]];
