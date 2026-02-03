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

  /// The character to use for lists.
  /// Default: `UnorderedListKind::Dashes`
  pub fn unordered_list_kind(&mut self, value: UnorderedListKind) -> &mut Self {
    self.insert("unorderedListKind", value.to_string().into())
  }

  /// The type of heading to use.
  /// Default: `HeadingKind::Atx`
  pub fn heading_kind(&mut self, value: HeadingKind) -> &mut Self {
    self.insert("headingKind", value.to_string().into())
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
      .unordered_list_kind(UnorderedListKind::Asterisks)
      .heading_kind(HeadingKind::Atx)
      .ignore_directive("test")
      .ignore_file_directive("test")
      .ignore_start_directive("test")
      .ignore_end_directive("test");

    let inner_config = config.get_inner_config();
    assert_eq!(inner_config.len(), 11);
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
    assert!(result.config.tags.get("JSX").is_none());
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
