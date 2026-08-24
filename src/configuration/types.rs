use std::collections::HashMap;

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
  pub hard_break_kind: HardBreakKind,
  /// The maximum number of consecutive blank lines to keep between blocks.
  pub max_blank_lines: u32,
  #[serde(rename = "heading.kind")]
  pub heading_kind: HeadingKind,
  /// The number of blank lines to write above a heading. That many are written
  /// even where `max_blank_lines` is lower, though a heading drawn up against
  /// the block above it within a list is left there, which keeps the list
  /// tight. `None` keeps the blank lines that were written, the same as any
  /// other block.
  #[serde(rename = "heading.blankLinesAbove")]
  pub heading_blank_lines_above: Option<u32>,
  /// The character to write the marker of an unordered list's items with.
  #[serde(rename = "list.unorderedMarker")]
  pub list_unordered_marker: ListUnorderedMarker,
  #[serde(rename = "list.indentKind")]
  pub list_indent_kind: ListIndentKind,
  #[serde(rename = "codeBlock.skipFormat")]
  pub code_block_skip_format: bool,
  #[serde(rename = "codeBlock.preserveIndentation")]
  pub code_block_preserve_indentation: bool,
  #[serde(rename = "codeBlock.preserveBlankLines")]
  pub code_block_preserve_blank_lines: bool,
  /// Whether to override the `useTabs` configuration of the plugin that
  /// formats the code within a code block. `None` leaves it to that plugin.
  #[serde(rename = "codeBlock.useTabs")]
  pub code_block_use_tabs: Option<bool>,
  /// Whether to override the `indentWidth` configuration of the plugin that
  /// formats the code within a code block. `None` leaves it to that plugin.
  #[serde(rename = "codeBlock.indentWidth")]
  pub code_block_indent_width: Option<u8>,
  /// Whether to leave a table's rows as they were written rather than
  /// aligning their cells into columns.
  #[serde(rename = "table.skipFormat")]
  pub table_skip_format: bool,
  /// The padding to write around the text of a table's cells. Has no effect on
  /// a table left as it was written by `table.skipFormat`.
  #[serde(rename = "table.cellPadding")]
  pub table_cell_padding: TableCellPadding,
  pub ignore_directive: String,
  pub ignore_file_directive: String,
  pub ignore_start_directive: String,
  pub ignore_end_directive: String,
  /// Custom tag to file extension mappings for formatting code blocks.
  /// For example: { "custom-tag": "md" }
  #[serde(default)]
  pub tags: HashMap<String, String>,
}

/// Text wrapping possibilities.
#[derive(Clone, PartialEq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextWrap {
  /// Always wraps text.
  Always,
  /// Maintains line breaks (default).
  Maintain,
  /// Maintains line breaks, but wraps a line that runs past the line width.
  ///
  /// This is `Maintain` with the wrapping of `Always` layered on top: no line
  /// is ever drawn up into the one above it, so the breaks that were written
  /// stay where they are, while a line too long to fit is broken up.
  MaintainAndWrap,
  /// Never wraps text.
  Never,
  /// Writes one sentence per line, ignoring the line width.
  ///
  /// This is the part of [semantic line breaks](https://sembr.org/) that can be
  /// applied without knowing what the text means: a line break is written where
  /// a sentence ends and nowhere else, which keeps a diff to the sentences that
  /// were edited.
  Sentence,
}

impl TextWrap {
  /// Whether the line breaks the file was written with are kept where they
  /// are, rather than the text being written back out over the lines it fits
  /// on.
  pub(crate) fn keeps_line_breaks(&self) -> bool {
    matches!(self, TextWrap::Maintain | TextWrap::MaintainAndWrap)
  }
}

generate_str_to_from![
  TextWrap,
  [Always, "always"],
  [Maintain, "maintain"],
  [MaintainAndWrap, "maintainAndWrap"],
  [Never, "never"],
  [Sentence, "sentence"]
];

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

/// The style of heading to use for level 1 and level 2 headings:
/// [setext](https://spec.commonmark.org/0.31.2/#setext-headings) or
/// [ATX](https://spec.commonmark.org/0.31.2/#atx-headings). Level 3 and
/// higher headings always use ATX headings, since Markdown only supports
/// setext headers for levels 1 and 2.
#[derive(Clone, PartialEq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeadingKind {
  /// Uses an underline of `=` or `-` beneath the heading text for level 1 and
  /// 2 headings.
  Setext,
  /// Uses `#` or `##` before the heading text for level 1 and 2 headings.
  Atx,
}

generate_str_to_from![HeadingKind, [Setext, "setext"], [Atx, "atx"]];

/// The style of [hard line break](https://spec.commonmark.org/0.31.2/#hard-line-breaks)
/// to use.
#[derive(Clone, PartialEq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HardBreakKind {
  /// Uses a trailing backslash (default).
  Backslash,
  /// Uses two trailing spaces.
  ///
  /// Note that some tools strip trailing whitespace, which would
  /// silently remove these hard line breaks.
  DoubleSpace,
}

generate_str_to_from![HardBreakKind, [Backslash, "backslash"], [DoubleSpace, "doubleSpace"]];

/// The padding to write around the text of a table's cells.
#[derive(Clone, PartialEq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TableCellPadding {
  /// Pads a cell out to the width of its column, which aligns the cells of a
  /// column with each other (default).
  ///
  /// The width of one cell then decides how every row of the table is written,
  /// so editing a single cell can rewrite all of them.
  Align,
  /// Writes a single space on either side of a cell's text.
  Space,
  /// Writes no space around a cell's text.
  None,
}

generate_str_to_from![TableCellPadding, [Align, "align"], [Space, "space"], [None, "none"]];

/// The character to use primarily for lists.
///
/// Unnumbered lists will be formatted to use a common list character, i.e., the primary list
/// character. Additionally, an alternate list character is used to separate lists which are not
/// separated by other paragraphs. This parameter defines which character should be used as primary
/// list character, i.e., either '-' (default) or '*'. The alternate list character will be the one
/// which is _not_ primary.
#[derive(Clone, PartialEq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListUnorderedMarker {
  /// Uses dashes (-) as primary character for lists (default).
  ///
  /// In this case, asterisks are used as alternate list characters.
  Dashes,
  /// Uses asterisks (*) as primary character for lists.
  ///
  /// In this case, dashes are used as alternate list characters.
  Asterisks,
}

impl ListUnorderedMarker {
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

generate_str_to_from![ListUnorderedMarker, [Dashes, "dashes"], [Asterisks, "asterisks"]];

/// The style of indentation to use for list items.
///
/// CommonMark aligns continuation lines to the content column after the marker
/// (e.g. 3 spaces for `1. `, 4 spaces for `10. `). PythonMarkdown uses a fixed
/// 4-space indent regardless of marker width, which is required by tools like
/// mkdocs-material.
#[derive(Clone, PartialEq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListIndentKind {
  /// Indents continuation lines to align with the content after the list marker (default).
  CommonMark,
  /// Always indents by 4 spaces, regardless of marker width.
  PythonMarkdown,
}

generate_str_to_from![
  ListIndentKind,
  [CommonMark, "commonMark"],
  [PythonMarkdown, "pythonMarkdown"]
];
