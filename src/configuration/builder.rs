use std::collections::HashMap;
use dprint_core::configuration::{GlobalConfiguration, resolve_global_config, NewLineKind, ConfigKeyMap, ConfigKeyValue};

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
pub struct ConfigurationBuilder {
    pub(super) config: ConfigKeyMap,
    global_config: Option<GlobalConfiguration>,
}

impl ConfigurationBuilder {
    /// Constructs a new configuration builder.
    pub fn new() -> ConfigurationBuilder {
        ConfigurationBuilder {
            config: HashMap::new(),
            global_config: None,
        }
    }

    /// Gets the final configuration that can be used to format a file.
    pub fn build(&self) -> Configuration {
        if let Some(global_config) = &self.global_config {
            resolve_config(self.config.clone(), global_config).config
        } else {
            let global_config = resolve_global_config(HashMap::new()).config;
            resolve_config(self.config.clone(), &global_config).config
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

    /// The directive used to ignore a line.
    /// Default: `dprint-ignore`
    pub fn ignore_directive(&mut self, value: &str) -> &mut Self {
        self.insert("ignoreDirective", value.to_string().into())
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
        self.text_wrap(TextWrap::Always)
            .ignore_directive("deno-fmt-ignore")
            .ignore_start_directive("deno-fmt-ignore-start")
            .ignore_end_directive("deno-fmt-ignore-end")
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
    use std::collections::HashMap;
    use dprint_core::configuration::{resolve_global_config, NewLineKind};

    use super::*;

    #[test]
    fn check_all_values_set() {
        let mut config = ConfigurationBuilder::new();
        config.new_line_kind(NewLineKind::CarriageReturnLineFeed)
            .line_width(90)
            .text_wrap(TextWrap::Always)
            .emphasis_kind(EmphasisKind::Asterisks)
            .strong_kind(StrongKind::Underscores);

        let inner_config = config.get_inner_config();
        assert_eq!(inner_config.len(), 5);
        let diagnostics = resolve_config(inner_config, &resolve_global_config(HashMap::new()).config).diagnostics;
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn handle_global_config() {
        let mut global_config = HashMap::new();
        global_config.insert(String::from("lineWidth"), 90.into());
        global_config.insert(String::from("newLineKind"), "crlf".into());
        global_config.insert(String::from("useTabs"), true.into());
        let global_config = resolve_global_config(global_config).config;
        let mut config_builder = ConfigurationBuilder::new();
        let config = config_builder.global_config(global_config).build();
        assert_eq!(config.line_width, 90);
        assert_eq!(config.new_line_kind == NewLineKind::CarriageReturnLineFeed, true);
    }

    #[test]
    fn use_markdown_defaults_when_global_not_set() {
        let global_config = resolve_global_config(HashMap::new()).config;
        let mut config_builder = ConfigurationBuilder::new();
        let config = config_builder.global_config(global_config).build();
        assert_eq!(config.line_width, 80); // this is different
        assert_eq!(config.new_line_kind == NewLineKind::LineFeed, true);
    }
}
