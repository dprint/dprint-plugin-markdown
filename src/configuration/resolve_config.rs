use super::builder::ConfigurationBuilder;
use super::types::*;
use super::Configuration;
use dprint_core::configuration::*;

/// Resolves configuration from a collection of key value strings.
///
/// # Example
///
/// ```
/// use std::collections::HashMap;
/// use dprint_core::configuration::{resolve_global_config};
/// use dprint_plugin_markdown::configuration::{resolve_config};
///
/// let config_map = HashMap::new(); // get a collection of key value pairs from somewhere
/// let global_config_result = resolve_global_config(config_map);
///
/// // check global_config_result.diagnostics here...
///
/// let markdown_config_map = HashMap::new(); // get a collection of k/v pairs from somewhere
/// let config_result = resolve_config(
///     markdown_config_map,
///     &global_config_result.config
/// );
///
/// // check config_result.diagnostics here and use config_result.config
/// ```
pub fn resolve_config(config: ConfigKeyMap, global_config: &GlobalConfiguration) -> ResolveConfigurationResult<Configuration> {
  let mut diagnostics = Vec::new();
  let mut config = config;

  if get_value(&mut config, "deno", false, &mut diagnostics) {
    fill_deno_config(&mut config);
  }

  let resolved_config = Configuration {
    line_width: get_value(&mut config, "lineWidth", global_config.line_width.unwrap_or(80), &mut diagnostics),
    new_line_kind: get_value(
      &mut config,
      "newLineKind",
      global_config.new_line_kind.unwrap_or(DEFAULT_GLOBAL_CONFIGURATION.new_line_kind),
      &mut diagnostics,
    ),
    text_wrap: get_value(&mut config, "textWrap", TextWrap::Maintain, &mut diagnostics),
    emphasis_kind: get_value(&mut config, "emphasisKind", EmphasisKind::Underscores, &mut diagnostics),
    strong_kind: get_value(&mut config, "strongKind", StrongKind::Asterisks, &mut diagnostics),
    ignore_directive: get_value(&mut config, "ignoreDirective", "dprint-ignore".to_string(), &mut diagnostics),
    ignore_file_directive: get_value(&mut config, "ignoreFileDirective", "dprint-ignore-file".to_string(), &mut diagnostics),
    ignore_start_directive: get_value(&mut config, "ignoreStartDirective", "dprint-ignore-start".to_string(), &mut diagnostics),
    ignore_end_directive: get_value(&mut config, "ignoreEndDirective", "dprint-ignore-end".to_string(), &mut diagnostics),
  };

  for (key, _) in config.iter() {
    diagnostics.push(ConfigurationDiagnostic {
      property_name: String::from(key),
      message: format!("Unknown property in configuration: {}", key),
    });
  }

  ResolveConfigurationResult {
    config: resolved_config,
    diagnostics,
  }
}

fn fill_deno_config(config: &mut ConfigKeyMap) {
  for (key, value) in ConfigurationBuilder::new().deno().config.iter() {
    if !config.contains_key(key) {
      config.insert(key.clone(), value.clone());
    }
  }
}
