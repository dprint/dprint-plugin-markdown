use std::path::Path;
use std::path::PathBuf;

use dprint_core::configuration::ConfigKeyMap;
use dprint_core::configuration::GlobalConfiguration;
use dprint_core::configuration::ResolveConfigurationResult;
use dprint_core::generate_plugin_code;
use dprint_core::plugins::FileMatchingInfo;
use dprint_core::plugins::FormatResult;
use dprint_core::plugins::PluginInfo;
use dprint_core::plugins::SyncPluginHandler;
use dprint_core::plugins::SyncPluginInfo;

use super::configuration::resolve_config;
use super::configuration::Configuration;

struct MarkdownPluginHandler;

impl SyncPluginHandler<Configuration> for MarkdownPluginHandler {
  fn resolve_config(
    &mut self,
    config: ConfigKeyMap,
    global_config: &GlobalConfiguration,
  ) -> ResolveConfigurationResult<Configuration> {
    resolve_config(config, global_config)
  }

  // Markdown extensions: markdown, mdown, mkdn, mdwn, mkd, md
  // ref: https://superuser.com/questions/249436/file-extension-for-markdown-files/285878#285878
  // ref: https://github.com/denoland/deno_registry2/issues/206
  fn plugin_info(&mut self) -> SyncPluginInfo {
    let version = env!("CARGO_PKG_VERSION").to_string();
    SyncPluginInfo {
      info: PluginInfo {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: version.clone(),
        config_key: "markdown".to_string(),
        help_url: "https://dprint.dev/plugins/markdown".to_string(),
        config_schema_url: format!(
          "https://plugins.dprint.dev/dprint/dprint-plugin-markdown/{}/schema.json",
          version
        ),
        update_url: Some("https://plugins.dprint.dev/dprint/dprint-plugin-markdown/latest.json".to_string()),
      },
      file_matching: FileMatchingInfo {
        file_extensions: vec![
          "md".to_string(),
          "mkd".to_string(),
          "mdwn".to_string(),
          "mkdn".to_string(),
          "mdown".to_string(),
          "markdown".to_string(),
        ],
        file_names: vec![],
      },
    }
  }

  fn license_text(&mut self) -> String {
    std::str::from_utf8(include_bytes!("../LICENSE")).unwrap().into()
  }

  fn format(
    &mut self,
    _file_path: &Path,
    file_text: &str,
    config: &Configuration,
    mut format_with_host: impl FnMut(&Path, String, &ConfigKeyMap) -> FormatResult,
  ) -> FormatResult {
    return super::format_text(file_text, config, |tag, file_text, line_width| {
      if let Some(ext) = tag_to_extension(tag) {
        let file_path = PathBuf::from(format!("file.{}", ext));
        let mut additional_config = ConfigKeyMap::new();
        additional_config.insert("lineWidth".into(), (line_width as i32).into());
        format_with_host(&file_path, file_text.to_string(), &additional_config)
      } else {
        Ok(None)
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

generate_plugin_code!(MarkdownPluginHandler, MarkdownPluginHandler);
