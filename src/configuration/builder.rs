use dprint_core::configuration::ConfigKeyMap;
use dprint_core::configuration::ConfigKeyValue;
use dprint_core::configuration::GlobalConfiguration;
use dprint_core::configuration::NewLineKind;

use super::*;

/// Markdown formatting configuration builder.
///
/// # Example
///
/// ```
/// use dprint_plugin_markdown::configuration::*;
///
/// let config = ConfigurationBuilder::new()
///     .line_width(80)
///     .build();
/// ```
#[derive(Default)]
pub struct ConfigurationBuilder {
  pub(super) config: ConfigKeyMap,
  global_config: Option<GlobalConfiguration>,
}

impl ConfigurationBuilder {
  /// Constructs a new configuration builder.
  pub fn new() -> Self {
    Self::default()
  }

  /// Gets the final configuration that can be used to format a file.
  pub fn build(&self) -> Configuration {
    if let Some(global_config) = &self.global_config {
      resolve_config(self.config.clone(), global_config).config
    } else {
      resolve_config(self.config.clone(), &Default::default()).config
    }
  }

  /// Set the global configuration.
  pub fn global_config(&mut self, global_config: GlobalConfiguration) -> &mut Self {
    self.global_config = Some(global_config);
    self
  }

  /// The width of a line the printer will try to stay under. Note that the printer may exceed this width in certain cases.
  /// Default: 80
  pub fn line_width(&mut self, value: u32) -> &mut Self {
    self.insert("lineWidth", (value as i32).into())
  }

  /// The kind of newline to use.
  /// Default: `NewLineKind::LineFeed`
  pub fn new_line_kind(&mut self, value: NewLineKind) -> &mut Self {
    self.insert("newLineKind", value.to_string().into())
  }

  /// The kind of text wrapping to use.
  /// Default: `TextWrap::Maintain`
  pub fn text_wrap(&mut self, value: TextWrap) -> &mut Self {
    self.insert("textWrap", value.to_string().into())
  }

  /// The character to use for emphasis/italics.
  /// Default: `EmphasisKind::Underscores`
  pub fn emphasis_kind(&mut self, value: EmphasisKind) -> &mut Self {
    self.insert("emphasisKind", value.to_string().into())
  }

  /// The character to use for strong emphasis/bold.
  /// Default: `StrongKind::Underscores`
  pub fn strong_kind(&mut self, value: StrongKind) -> &mut Self {
    self.insert("strongKind", value.to_string().into())
  }

  /// The style of hard line break to use.
  /// Default: `HardBreakKind::Backslash`
  pub fn hard_break_kind(&mut self, value: HardBreakKind) -> &mut Self {
    self.insert("hardBreakKind", value.to_string().into())
  }

  /// The maximum number of consecutive blank lines to keep between blocks.
  /// Default: 1
  pub fn max_blank_lines(&mut self, value: u32) -> &mut Self {
    self.insert("maxBlankLines", (value as i32).into())
  }

  /// The type of heading to use.
  /// Default: `HeadingKind::Atx`
  pub fn heading_kind(&mut self, value: HeadingKind) -> &mut Self {
    self.insert("heading.kind", value.to_string().into())
  }

  /// The number of blank lines to write above a heading. That many are written
  /// even where `maxBlankLines` is lower, though a heading drawn up against the
  /// block above it within a list is left there, which keeps the list tight.
  /// Default: the blank lines that were written are kept, up to `maxBlankLines`
  pub fn heading_blank_lines_above(&mut self, value: u32) -> &mut Self {
    self.insert("heading.blankLinesAbove", (value as i32).into())
  }

  /// The character to write the marker of an unordered list's items with.
  /// Default: `ListUnorderedMarker::Dashes`
  pub fn list_unordered_marker(&mut self, value: ListUnorderedMarker) -> &mut Self {
    self.insert("list.unorderedMarker", value.to_string().into())
  }

  /// The style of list indentation to use.
  /// Default: `ListIndentKind::CommonMark`
  pub fn list_indent_kind(&mut self, value: ListIndentKind) -> &mut Self {
    self.insert("list.indentKind", value.to_string().into())
  }

  /// Whether to leave the code within a code block as it was written rather
  /// than formatting it with the plugin that handles the code's language.
  /// Default: `false`
  pub fn code_block_skip_format(&mut self, value: bool) -> &mut Self {
    self.insert("codeBlock.skipFormat", value.into())
  }

  /// Whether to fail the file when the plugin that formats the code within a
  /// code block reports an error, which is what it does when the code doesn't
  /// parse, rather than leaving that code as it was written.
  /// Default: `false`
  pub fn code_block_raise_syntax_errors(&mut self, value: bool) -> &mut Self {
    self.insert("codeBlock.raiseSyntaxErrors", value.into())
  }

  /// Whether to keep the indentation the code within a code block was
  /// written with, rather than unindenting it.
  /// Default: `false`
  pub fn code_block_preserve_indentation(&mut self, value: bool) -> &mut Self {
    self.insert("codeBlock.preserveIndentation", value.into())
  }

  /// Whether to keep the blank lines at the start and end of a fenced code
  /// block, rather than trimming them.
  /// Default: `false`
  pub fn code_block_preserve_blank_lines(&mut self, value: bool) -> &mut Self {
    self.insert("codeBlock.preserveBlankLines", value.into())
  }

  /// Whether the plugin that formats the code within a code block should use
  /// tabs, overriding that plugin's own `useTabs` configuration.
  /// Default: not overridden
  pub fn code_block_use_tabs(&mut self, value: bool) -> &mut Self {
    self.insert("codeBlock.useTabs", value.into())
  }

  /// The indentation width the plugin that formats the code within a code
  /// block should use, overriding that plugin's own `indentWidth`
  /// configuration.
  /// Default: not overridden
  pub fn code_block_indent_width(&mut self, value: u8) -> &mut Self {
    self.insert("codeBlock.indentWidth", (value as i32).into())
  }

  /// Whether to leave the html written within the file as it was rather than
  /// laying out its tags and their content.
  /// Default: `false`
  pub fn html_skip_format(&mut self, value: bool) -> &mut Self {
    self.insert("html.skipFormat", value.into())
  }

  /// Whether to indent the html written within the file with tabs.
  /// Default: `false`
  pub fn html_use_tabs(&mut self, value: bool) -> &mut Self {
    self.insert("html.useTabs", value.into())
  }

  /// The number of spaces to indent the html written within the file with.
  /// Default: `2`
  pub fn html_indent_width(&mut self, value: u8) -> &mut Self {
    self.insert("html.indentWidth", (value as i32).into())
  }

  /// Whether to write a space before the `/>` that closes a self-closing tag,
  /// so that a tag is written as `<br />` rather than as `<br/>`.
  /// Default: `true`
  pub fn html_self_closing_space(&mut self, value: bool) -> &mut Self {
    self.insert("html.selfClosingSpace", value.into())
  }

  /// Whether to leave a table's rows as they were written rather than
  /// aligning their cells into columns.
  /// Default: `false`
  pub fn table_skip_format(&mut self, value: bool) -> &mut Self {
    self.insert("table.skipFormat", value.into())
  }

  /// The padding to write around the text of a table's cells.
  /// Default: `TableCellPadding::Align`
  pub fn table_cell_padding(&mut self, value: TableCellPadding) -> &mut Self {
    self.insert("table.cellPadding", value.to_string().into())
  }

  /// The directive used to ignore a line.
  /// Default: `dprint-ignore`
  pub fn ignore_directive(&mut self, value: &str) -> &mut Self {
    self.insert("ignoreDirective", value.to_string().into())
  }

  /// The directive used to ignore a file.
  /// Default: `dprint-ignore-file`
  pub fn ignore_file_directive(&mut self, value: &str) -> &mut Self {
    self.insert("ignoreFileDirective", value.to_string().into())
  }

  /// The directive used to mark start of ignored section.
  /// Default: `dprint-ignore-start`
  pub fn ignore_start_directive(&mut self, value: &str) -> &mut Self {
    self.insert("ignoreStartDirective", value.to_string().into())
  }

  /// The directive used to mark end of ignored section.
  /// Default: `dprint-ignore-end`
  pub fn ignore_end_directive(&mut self, value: &str) -> &mut Self {
    self.insert("ignoreEndDirective", value.to_string().into())
  }

  pub fn deno(&mut self) -> &mut Self {
    self
      .text_wrap(TextWrap::Always)
      .ignore_directive("deno-fmt-ignore")
      .ignore_start_directive("deno-fmt-ignore-start")
      .ignore_end_directive("deno-fmt-ignore-end")
      .ignore_file_directive("deno-fmt-ignore-file")
  }

  #[cfg(test)]
  pub(super) fn get_inner_config(&self) -> ConfigKeyMap {
    self.config.clone()
  }

  fn insert(&mut self, name: &str, value: ConfigKeyValue) -> &mut Self {
    self.config.insert(String::from(name), value);
    self
  }
}

#[cfg(test)]
mod tests {
  use dprint_core::configuration::resolve_global_config;
  use dprint_core::configuration::NewLineKind;

  use super::*;

  #[test]
  fn check_all_values_set() {
    let mut config = ConfigurationBuilder::new();
    config
      .new_line_kind(NewLineKind::CarriageReturnLineFeed)
      .line_width(90)
      .text_wrap(TextWrap::Always)
      .emphasis_kind(EmphasisKind::Asterisks)
      .strong_kind(StrongKind::Underscores)
      .hard_break_kind(HardBreakKind::DoubleSpace)
      .max_blank_lines(2)
      .heading_kind(HeadingKind::Atx)
      .heading_blank_lines_above(2)
      .list_unordered_marker(ListUnorderedMarker::Asterisks)
      .list_indent_kind(ListIndentKind::PythonMarkdown)
      .code_block_skip_format(true)
      .code_block_raise_syntax_errors(true)
      .code_block_preserve_indentation(true)
      .code_block_preserve_blank_lines(true)
      .code_block_use_tabs(true)
      .code_block_indent_width(2)
      .table_skip_format(true)
      .table_cell_padding(TableCellPadding::Space)
      .ignore_directive("test")
      .ignore_file_directive("test")
      .ignore_start_directive("test")
      .ignore_end_directive("test");

    let inner_config = config.get_inner_config();
    assert_eq!(inner_config.len(), 23);
    let diagnostics = resolve_config(inner_config, &Default::default()).diagnostics;
    assert_eq!(diagnostics.len(), 0);
  }

  #[test]
  fn handle_global_config() {
    let mut global_config = ConfigKeyMap::new();
    global_config.insert(String::from("lineWidth"), 90.into());
    global_config.insert(String::from("newLineKind"), "crlf".into());
    global_config.insert(String::from("useTabs"), true.into());
    let global_config = resolve_global_config(&mut global_config).config;
    let mut config_builder = ConfigurationBuilder::new();
    let config = config_builder.global_config(global_config).build();
    assert_eq!(config.line_width, 90);
    assert_eq!(config.new_line_kind == NewLineKind::CarriageReturnLineFeed, true);
  }

  #[test]
  fn use_markdown_defaults_when_global_not_set() {
    let global_config = GlobalConfiguration::default();
    let mut config_builder = ConfigurationBuilder::new();
    let config = config_builder.global_config(global_config).build();
    assert_eq!(config.line_width, 80); // this is different
    assert_eq!(config.new_line_kind == NewLineKind::LineFeed, true);
  }

  #[test]
  fn code_block_use_tabs_only_set_when_specified() {
    let config = ConfigurationBuilder::new().build();
    assert_eq!(config.code_block_use_tabs, None);

    let config = ConfigurationBuilder::new().code_block_use_tabs(true).build();
    assert_eq!(config.code_block_use_tabs, Some(true));

    let config = ConfigurationBuilder::new().code_block_use_tabs(false).build();
    assert_eq!(config.code_block_use_tabs, Some(false));
  }

  #[test]
  fn code_block_indent_width_only_set_when_specified() {
    let config = ConfigurationBuilder::new().build();
    assert_eq!(config.code_block_indent_width, None);

    let config = ConfigurationBuilder::new().code_block_indent_width(2).build();
    assert_eq!(config.code_block_indent_width, Some(2));
  }

  #[test]
  fn max_blank_lines_below_one() {
    let mut config = ConfigKeyMap::new();
    config.insert("maxBlankLines".into(), 0.into());

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].property_name, "maxBlankLines");
    assert!(result.diagnostics[0].message.contains("at least 1"));
    assert_eq!(result.config.max_blank_lines, 1);
  }

  #[test]
  fn heading_blank_lines_above_only_set_when_specified() {
    let config = ConfigurationBuilder::new().build();
    assert_eq!(config.heading_blank_lines_above, None);

    let config = ConfigurationBuilder::new().heading_blank_lines_above(2).build();
    assert_eq!(config.heading_blank_lines_above, Some(2));
  }

  #[test]
  fn heading_blank_lines_above_below_one() {
    let mut config = ConfigKeyMap::new();
    config.insert("heading.blankLinesAbove".into(), 0.into());

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].property_name, "heading.blankLinesAbove");
    assert!(result.diagnostics[0].message.contains("at least 1"));
    assert_eq!(result.config.heading_blank_lines_above, Some(1));
  }

  #[test]
  fn deprecated_names_still_resolve() {
    let mut config = ConfigKeyMap::new();
    config.insert("headingKind".into(), "setext".into());
    config.insert("unorderedListKind".into(), "asterisks".into());
    config.insert("listIndentKind".into(), "pythonMarkdown".into());

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 0);
    assert!(result.config.heading_kind == HeadingKind::Setext);
    assert!(result.config.list_unordered_marker == ListUnorderedMarker::Asterisks);
    assert!(result.config.list_indent_kind == ListIndentKind::PythonMarkdown);
  }

  #[test]
  fn deprecated_names_lose_to_current_names() {
    let mut config = ConfigKeyMap::new();
    config.insert("headingKind".into(), "atx".into());
    config.insert("heading.kind".into(), "setext".into());

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 0);
    assert!(result.config.heading_kind == HeadingKind::Setext);
  }

  #[test]
  fn tags_valid_object() {
    let mut config = ConfigKeyMap::new();
    let mut tags_obj = ConfigKeyMap::new();
    tags_obj.insert("markdown".into(), "md".into());
    tags_obj.insert("JSX".into(), "tsx".into());
    config.insert("tags".into(), ConfigKeyValue::Object(tags_obj));

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 0);
    assert_eq!(result.config.tags.get("markdown").unwrap(), "md");
    // keys should be lowercased
    assert_eq!(result.config.tags.get("jsx").unwrap(), "tsx");
    assert!(!result.config.tags.contains_key("JSX"));
  }

  #[test]
  fn tags_extension_with_period() {
    let mut config = ConfigKeyMap::new();
    let mut tags_obj = ConfigKeyMap::new();
    tags_obj.insert("markdown".into(), ".md".into());
    config.insert("tags".into(), ConfigKeyValue::Object(tags_obj));

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].property_name, "tags.markdown");
    assert!(result.diagnostics[0].message.contains("without a period"));
  }

  #[test]
  fn tags_non_string_value() {
    let mut config = ConfigKeyMap::new();
    let mut tags_obj = ConfigKeyMap::new();
    tags_obj.insert("markdown".into(), true.into());
    config.insert("tags".into(), ConfigKeyValue::Object(tags_obj));

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].property_name, "tags.markdown");
    assert!(result.diagnostics[0].message.contains("Expected string value"));
  }

  #[test]
  fn tags_not_an_object() {
    let mut config = ConfigKeyMap::new();
    config.insert("tags".into(), "not_an_object".into());

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].property_name, "tags");
    assert!(result.diagnostics[0].message.contains("Expected an object"));
  }
}
