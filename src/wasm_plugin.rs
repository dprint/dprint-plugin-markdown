use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use dprint_core::configuration::ConfigKeyMap;
use dprint_core::configuration::GlobalConfiguration;
use dprint_core::configuration::ResolveConfigurationResult;
use dprint_core::generate_plugin_code;
use dprint_core::plugins::PluginHandler;
use dprint_core::plugins::PluginInfo;

use super::configuration::resolve_config;
use super::configuration::Configuration;

struct MarkdownPluginHandler {}

impl MarkdownPluginHandler {
  pub const fn new() -> Self {
    MarkdownPluginHandler {}
  }
}

impl PluginHandler<Configuration> for MarkdownPluginHandler {
  fn resolve_config(&mut self, config: ConfigKeyMap, global_config: &GlobalConfiguration) -> ResolveConfigurationResult<Configuration> {
    resolve_config(config, global_config)
  }

  // Markdown extensions: markdown, mdown, mkdn, mdwn, mkd, md
  // ref: https://superuser.com/questions/249436/file-extension-for-markdown-files/285878#285878
  // ref: https://github.com/denoland/deno_registry2/issues/206
  fn get_plugin_info(&mut self) -> PluginInfo {
    let version = env!("CARGO_PKG_VERSION").to_string();
    PluginInfo {
      name: env!("CARGO_PKG_NAME").to_string(),
      version: version.clone(),
      config_key: "markdown".to_string(),
      file_extensions: vec![
        "md".to_string(),
        "mkd".to_string(),
        "mdwn".to_string(),
        "mkdn".to_string(),
        "mdown".to_string(),
        "markdown".to_string(),
      ],
      file_names: vec![],
      help_url: "https://dprint.dev/plugins/markdown".to_string(),
      config_schema_url: format!("https://plugins.dprint.dev/dprint/dprint-plugin-markdown/{}/schema.json", version),
      update_url: Some("https://plugins.dprint.dev/dprint/dprint-plugin-markdown/latest.json".to_string()),
    }
  }

  fn get_license_text(&mut self) -> String {
    std::str::from_utf8(include_bytes!("../LICENSE")).unwrap().into()
  }

  fn format_text(
    &mut self,
    _file_path: &Path,
    file_text: &str,
    config: &Configuration,
    mut format_with_host: impl FnMut(&Path, String, &ConfigKeyMap) -> Result<String>,
  ) -> Result<String> {
    return super::format_text(file_text, config, |tag, file_text, line_width| {
      if let Some(ext) = tag_to_extension(tag) {
        let file_path = PathBuf::from(format!("file.{}", ext));
        let mut additional_config = HashMap::new();
        additional_config.insert("lineWidth".into(), (line_width as i32).into());
        format_with_host(&file_path, file_text.to_string(), &additional_config).map(|text| Cow::Owned(text))
      } else {
        Ok(Cow::Borrowed(file_text))
      }
    });

    fn tag_to_extension(tag: &str) -> Option<&'static str> {
      match tag.trim().to_lowercase().as_str() {
        "typescript" | "ts" => Some("ts"),
        "tsx" => Some("tsx"),
        "javascript" | "js" => Some("js"),
        "jsx" => Some("jsx"),
        "json" | "jsonc" => Some("json"),
        "rust" | "rs" => Some("rs"),
        "csharp" | "cs" => Some("cs"),
        "visualbasic" | "vb" => Some("vb"),
        "css" => Some("css"),
        "less" => Some("less"),
        "toml" => Some("toml"),
        "scss" => Some("scss"),
        "svelte" => Some("svelte"),
        "vue" => Some("vue"),
        _ => None,
      }
    }
  }
}

generate_plugin_code!(MarkdownPluginHandler, MarkdownPluginHandler::new());
