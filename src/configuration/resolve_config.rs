use std::collections::HashMap;

use super::builder::ConfigurationBuilder;
use super::types::*;
use super::Configuration;
use dprint_core::configuration::*;

/// Resolves configuration from a collection of key value strings.
///
/// # Example
///
/// ```
/// use dprint_core::configuration::ConfigKeyMap;
/// use dprint_core::configuration::resolve_global_config;
/// use dprint_plugin_markdown::configuration::resolve_config;
///
/// let mut config_map = ConfigKeyMap::new(); // get a collection of key value pairs from somewhere
/// let global_config_result = resolve_global_config(&mut config_map);
///
/// // check global_config_result.diagnostics here...
///
/// let markdown_config_map = ConfigKeyMap::new(); // get a collection of k/v pairs from somewhere
/// let config_result = resolve_config(
///     markdown_config_map,
///     &global_config_result.config
/// );
///
/// // check config_result.diagnostics here and use config_result.config
/// ```
pub fn resolve_config(
  config: ConfigKeyMap,
  global_config: &GlobalConfiguration,
) -> ResolveConfigurationResult<Configuration> {
  let mut diagnostics = Vec::new();
  let mut config = config;

  if get_value(&mut config, "deno", false, &mut diagnostics) {
    fill_deno_config(&mut config);
  }

  let tags = get_tags(&mut config, &mut diagnostics);

  let resolved_config = Configuration {
    line_width: get_value(
      &mut config,
      "lineWidth",
      global_config.line_width.unwrap_or(80),
      &mut diagnostics,
    ),
    new_line_kind: get_value(
      &mut config,
      "newLineKind",
      global_config
        .new_line_kind
        .unwrap_or(RECOMMENDED_GLOBAL_CONFIGURATION.new_line_kind),
      &mut diagnostics,
    ),
    text_wrap: get_value(&mut config, "textWrap", TextWrap::Maintain, &mut diagnostics),
    wrap_unspaced_scripts: get_value(&mut config, "wrapUnspacedScripts", false, &mut diagnostics),
    emphasis_kind: get_value(&mut config, "emphasisKind", EmphasisKind::Underscores, &mut diagnostics),
    strong_kind: get_value(&mut config, "strongKind", StrongKind::Asterisks, &mut diagnostics),
    hard_break_kind: get_value(&mut config, "hardBreakKind", HardBreakKind::Backslash, &mut diagnostics),
    max_blank_lines: get_max_blank_lines(&mut config, &mut diagnostics),
    heading_kind: get_renamed_value(
      &mut config,
      "heading.kind",
      "headingKind",
      HeadingKind::Atx,
      &mut diagnostics,
    ),
    heading_blank_lines_above: get_heading_blank_lines_above(&mut config, &mut diagnostics),
    list_unordered_marker: get_renamed_value(
      &mut config,
      "list.unorderedMarker",
      "unorderedListKind",
      ListUnorderedMarker::Dashes,
      &mut diagnostics,
    ),
    list_indent_kind: get_renamed_value(
      &mut config,
      "list.indentKind",
      "listIndentKind",
      ListIndentKind::CommonMark,
      &mut diagnostics,
    ),
    code_block_skip_format: get_value(&mut config, "codeBlock.skipFormat", false, &mut diagnostics),
    code_block_raise_syntax_errors: get_value(&mut config, "codeBlock.raiseSyntaxErrors", false, &mut diagnostics),
    code_block_preserve_indentation: get_value(&mut config, "codeBlock.preserveIndentation", false, &mut diagnostics),
    code_block_preserve_blank_lines: get_value(&mut config, "codeBlock.preserveBlankLines", false, &mut diagnostics),
    code_block_use_tabs: get_nullable_value(&mut config, "codeBlock.useTabs", &mut diagnostics),
    code_block_indent_width: get_nullable_value(&mut config, "codeBlock.indentWidth", &mut diagnostics),
    html_skip_format: get_value(&mut config, "html.skipFormat", false, &mut diagnostics),
    html_use_tabs: get_value(
      &mut config,
      "html.useTabs",
      global_config.use_tabs.unwrap_or(false),
      &mut diagnostics,
    ),
    html_indent_width: get_value(
      &mut config,
      "html.indentWidth",
      global_config.indent_width.unwrap_or(2),
      &mut diagnostics,
    ),
    html_self_closing_space: get_value(&mut config, "html.selfClosingSpace", true, &mut diagnostics),
    html_prefer_single_line: get_value(&mut config, "html.preferSingleLine", false, &mut diagnostics),
    table_skip_format: get_value(&mut config, "table.skipFormat", false, &mut diagnostics),
    table_cell_padding: get_value(
      &mut config,
      "table.cellPadding",
      TableCellPadding::Align,
      &mut diagnostics,
    ),
    ignore_directive: get_value(
      &mut config,
      "ignoreDirective",
      "dprint-ignore".to_string(),
      &mut diagnostics,
    ),
    ignore_file_directive: get_value(
      &mut config,
      "ignoreFileDirective",
      "dprint-ignore-file".to_string(),
      &mut diagnostics,
    ),
    ignore_start_directive: get_value(
      &mut config,
      "ignoreStartDirective",
      "dprint-ignore-start".to_string(),
      &mut diagnostics,
    ),
    ignore_end_directive: get_value(
      &mut config,
      "ignoreEndDirective",
      "dprint-ignore-end".to_string(),
      &mut diagnostics,
    ),
    tags,
  };

  for (key, _) in config.into_iter() {
    diagnostics.push(ConfigurationDiagnostic {
      message: format!("Unknown property in configuration: {}", key),
      property_name: key,
    });
  }

  ResolveConfigurationResult {
    config: resolved_config,
    diagnostics,
  }
}

/// Reads a property, falling back to the deprecated name the property used to
/// go by. Both names are taken from the map so that a deprecated one isn't
/// reported as unknown.
/// A key written under a name the plugin has since renamed, which is what
/// `dprint config update` moves over.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigKeyRename {
  pub old_key: &'static str,
  pub new_key: &'static str,
  /// The value to write under the new key, or `None` when the new key was
  /// already set and the old one is only to be dropped.
  pub value: Option<ConfigKeyValue>,
}

/// Finds the keys of a configuration that are written under a deprecated name.
pub fn get_config_key_renames(config: &ConfigKeyMap) -> Vec<ConfigKeyRename> {
  RENAMED_KEYS
    .iter()
    .filter_map(|(old_key, new_key)| {
      let value = config.get(*old_key)?;
      Some(ConfigKeyRename {
        old_key,
        new_key,
        value: (!config.contains_key(*new_key)).then(|| value.clone()),
      })
    })
    .collect()
}

/// Each key that was renamed, as the old name then the new one.
const RENAMED_KEYS: &[(&str, &str)] = &[
  ("headingKind", "heading.kind"),
  ("unorderedListKind", "list.unorderedMarker"),
  ("listIndentKind", "list.indentKind"),
];

fn get_renamed_value<T>(
  config: &mut ConfigKeyMap,
  key: &str,
  deprecated_key: &str,
  default_value: T,
  diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> T
where
  T: std::str::FromStr,
  <T as std::str::FromStr>::Err: std::fmt::Display,
{
  let deprecated_value = get_nullable_value(config, deprecated_key, diagnostics);
  get_nullable_value(config, key, diagnostics)
    .or(deprecated_value)
    .unwrap_or(default_value)
}

/// A block is always separated from the one above it by a blank line, so a
/// maximum below one isn't something the formatter could ever stay under.
fn get_max_blank_lines(config: &mut ConfigKeyMap, diagnostics: &mut Vec<ConfigurationDiagnostic>) -> u32 {
  let value = get_value(config, "maxBlankLines", 1, diagnostics);
  ensure_at_least_one_blank_line("maxBlankLines", value, diagnostics)
}

/// With `heading.kind: setext` a heading written against the paragraph above it
/// would underline that paragraph rather than follow it, so a count below one
/// isn't one that could be written wherever the option applies.
fn get_heading_blank_lines_above(
  config: &mut ConfigKeyMap,
  diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> Option<u32> {
  let value = get_nullable_value(config, "heading.blankLinesAbove", diagnostics)?;
  Some(ensure_at_least_one_blank_line(
    "heading.blankLinesAbove",
    value,
    diagnostics,
  ))
}

fn ensure_at_least_one_blank_line(
  property_name: &str,
  value: u32,
  diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> u32 {
  if value < 1 {
    diagnostics.push(ConfigurationDiagnostic {
      property_name: property_name.to_string(),
      message: "Expected a value of at least 1.".to_string(),
    });
    return 1;
  }
  value
}

fn get_tags(config: &mut ConfigKeyMap, diagnostics: &mut Vec<ConfigurationDiagnostic>) -> HashMap<String, String> {
  let mut tags = HashMap::new();

  if let Some(value) = config.shift_remove("tags") {
    match value {
      ConfigKeyValue::Object(obj) => {
        tags.reserve(obj.len());
        for (key, val) in obj.into_iter() {
          match val {
            ConfigKeyValue::String(s) => {
              if s.contains('.') {
                diagnostics.push(ConfigurationDiagnostic {
                  property_name: format!("tags.{}", key),
                  message: format!(
                    "Expected a file extension without a period for tag '{}', but got '{}'",
                    key, s
                  ),
                });
              }
              tags.insert(key.to_lowercase(), s);
            }
            _ => {
              diagnostics.push(ConfigurationDiagnostic {
                property_name: format!("tags.{}", key),
                message: format!("Expected string value for tag '{}', but got a different type", key),
              });
            }
          }
        }
      }
      _ => {
        diagnostics.push(ConfigurationDiagnostic {
          property_name: "tags".to_string(),
          message: "Expected an object for 'tags' configuration".to_string(),
        });
      }
    }
  }

  tags
}

fn fill_deno_config(config: &mut ConfigKeyMap) {
  for (key, value) in ConfigurationBuilder::new().deno().config.iter() {
    if !config.contains_key(key) {
      config.insert(key.clone(), value.clone());
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn renames_deprecated_keys() {
    let mut config = ConfigKeyMap::new();
    config.insert("headingKind".into(), "setext".into());
    config.insert("unorderedListKind".into(), "asterisks".into());
    config.insert("listIndentKind".into(), "pythonMarkdown".into());
    config.insert("textWrap".into(), "always".into());

    assert_eq!(
      get_config_key_renames(&config),
      vec![
        ConfigKeyRename {
          old_key: "headingKind",
          new_key: "heading.kind",
          value: Some("setext".into()),
        },
        ConfigKeyRename {
          old_key: "unorderedListKind",
          new_key: "list.unorderedMarker",
          value: Some("asterisks".into()),
        },
        ConfigKeyRename {
          old_key: "listIndentKind",
          new_key: "list.indentKind",
          value: Some("pythonMarkdown".into()),
        },
      ]
    );
  }

  #[test]
  fn rename_keeps_the_new_key_when_both_are_set() {
    let mut config = ConfigKeyMap::new();
    config.insert("headingKind".into(), "setext".into());
    config.insert("heading.kind".into(), "atx".into());

    assert_eq!(
      get_config_key_renames(&config),
      vec![ConfigKeyRename {
        old_key: "headingKind",
        new_key: "heading.kind",
        value: None,
      }]
    );
  }

  #[test]
  fn no_renames_for_a_current_config() {
    let mut config = ConfigKeyMap::new();
    config.insert("heading.kind".into(), "setext".into());
    assert!(get_config_key_renames(&config).is_empty());
    assert!(get_config_key_renames(&ConfigKeyMap::new()).is_empty());
  }

  #[test]
  fn deprecated_key_is_used_when_the_new_one_is_not_set() {
    let mut config = ConfigKeyMap::new();
    config.insert("headingKind".into(), "setext".into());
    config.insert("unorderedListKind".into(), "asterisks".into());
    config.insert("listIndentKind".into(), "pythonMarkdown".into());

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 0);
    assert_eq!(result.config.heading_kind, HeadingKind::Setext);
    assert_eq!(result.config.list_unordered_marker, ListUnorderedMarker::Asterisks);
    assert_eq!(result.config.list_indent_kind, ListIndentKind::PythonMarkdown);
  }

  #[test]
  fn new_key_wins_over_deprecated_key() {
    let mut config = ConfigKeyMap::new();
    config.insert("headingKind".into(), "setext".into());
    config.insert("heading.kind".into(), "atx".into());

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 0);
    assert_eq!(result.config.heading_kind, HeadingKind::Atx);
  }

  #[test]
  fn unknown_property_diagnostic() {
    let mut config = ConfigKeyMap::new();
    config.insert("unknownKey".into(), true.into());

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].property_name, "unknownKey");
    assert_eq!(
      result.diagnostics[0].message,
      "Unknown property in configuration: unknownKey"
    );
  }

  #[test]
  fn invalid_enum_value_uses_default() {
    let mut config = ConfigKeyMap::new();
    config.insert("textWrap".into(), "bogus".into());

    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].property_name, "textWrap");
    assert!(result.diagnostics[0].message.contains("bogus"));
    assert_eq!(result.config.text_wrap, TextWrap::Maintain);
  }

  #[test]
  fn invalid_value_type_uses_default() {
    let mut config = ConfigKeyMap::new();
    config.insert("lineWidth".into(), "abc".into());
    config.insert("codeBlock.useTabs".into(), "yes".into());
    config.insert("heading.blankLinesAbove".into(), "x".into());
    config.insert("html.indentWidth".into(), 300.into());

    let result = resolve_config(config, &Default::default());
    let mut names = result
      .diagnostics
      .iter()
      .map(|diagnostic| diagnostic.property_name.as_str())
      .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
      names,
      vec![
        "codeBlock.useTabs",
        "heading.blankLinesAbove",
        "html.indentWidth",
        "lineWidth"
      ]
    );
    assert_eq!(result.config.line_width, 80);
    assert_eq!(result.config.code_block_use_tabs, None);
    assert_eq!(result.config.heading_blank_lines_above, None);
    assert_eq!(result.config.html_indent_width, 2);
  }

  #[test]
  fn deno_fills_defaults_without_overriding_explicit_values() {
    let mut config = ConfigKeyMap::new();
    config.insert("deno".into(), true.into());
    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 0);
    assert_eq!(result.config.text_wrap, TextWrap::Always);
    assert_eq!(result.config.ignore_directive, "deno-fmt-ignore");
    assert_eq!(result.config.ignore_file_directive, "deno-fmt-ignore-file");
    assert_eq!(result.config.ignore_start_directive, "deno-fmt-ignore-start");
    assert_eq!(result.config.ignore_end_directive, "deno-fmt-ignore-end");

    let mut config = ConfigKeyMap::new();
    config.insert("deno".into(), true.into());
    config.insert("textWrap".into(), "never".into());
    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 0);
    assert_eq!(result.config.text_wrap, TextWrap::Never);

    let mut config = ConfigKeyMap::new();
    config.insert("deno".into(), "yes".into());
    let result = resolve_config(config, &Default::default());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].property_name, "deno");
  }

  #[test]
  fn html_indentation_falls_back_to_global_config() {
    let mut global_config = ConfigKeyMap::new();
    global_config.insert("useTabs".into(), true.into());
    global_config.insert("indentWidth".into(), 4.into());
    let global_config = resolve_global_config(&mut global_config).config;

    let result = resolve_config(ConfigKeyMap::new(), &global_config);
    assert_eq!(result.config.html_use_tabs, true);
    assert_eq!(result.config.html_indent_width, 4);
    // the code block indentation is only ever what was asked for
    assert_eq!(result.config.code_block_use_tabs, None);
    assert_eq!(result.config.code_block_indent_width, None);

    let result = resolve_config(ConfigKeyMap::new(), &Default::default());
    assert_eq!(result.config.html_use_tabs, false);
    assert_eq!(result.config.html_indent_width, 2);
  }
}
