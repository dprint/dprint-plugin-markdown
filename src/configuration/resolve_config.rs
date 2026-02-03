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
    emphasis_kind: get_value(&mut config, "emphasisKind", EmphasisKind::Underscores, &mut diagnostics),
    strong_kind: get_value(&mut config, "strongKind", StrongKind::Asterisks, &mut diagnostics),
    unordered_list_kind: get_value(
      &mut config,
      "unorderedListKind",
      UnorderedListKind::Dashes,
      &mut diagnostics,
    ),
    heading_kind: get_value(&mut config, "headingKind", HeadingKind::Atx, &mut diagnostics),
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
