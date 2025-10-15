use std::path::PathBuf;

use dprint_core::configuration::ConfigKeyMap;
use dprint_core::configuration::GlobalConfiguration;
use dprint_core::generate_plugin_code;
use dprint_core::plugins::CheckConfigUpdatesMessage;
use dprint_core::plugins::ConfigChange;
use dprint_core::plugins::FileMatchingInfo;
use dprint_core::plugins::FormatRange;
use dprint_core::plugins::FormatResult;
use dprint_core::plugins::PluginInfo;
use dprint_core::plugins::PluginResolveConfigurationResult;
use dprint_core::plugins::SyncFormatRequest;
use dprint_core::plugins::SyncHostFormatRequest;
use dprint_core::plugins::SyncPluginHandler;

use super::configuration::resolve_config;
use super::configuration::Configuration;

struct MarkdownPluginHandler;

impl SyncPluginHandler<Configuration> for MarkdownPluginHandler {
  fn resolve_config(
    &mut self,
    config: ConfigKeyMap,
    global_config: &GlobalConfiguration,
  ) -> PluginResolveConfigurationResult<Configuration> {
    let config = resolve_config(config, global_config);
    PluginResolveConfigurationResult {
      config: config.config,
      diagnostics: config.diagnostics,
      file_matching: FileMatchingInfo {
        // Markdown extensions: markdown, mdown, mkdn, mdwn, mkd, md
        // ref: https://superuser.com/questions/249436/file-extension-for-markdown-files/285878#285878
        // ref: https://github.com/denoland/deno_registry2/issues/206
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

  fn check_config_updates(&self, _message: CheckConfigUpdatesMessage) -> Result<Vec<ConfigChange>, anyhow::Error> {
    Ok(Vec::new())
  }

  fn plugin_info(&mut self) -> PluginInfo {
    let version = env!("CARGO_PKG_VERSION").to_string();
    PluginInfo {
      name: env!("CARGO_PKG_NAME").to_string(),
      version: version.clone(),
      config_key: "markdown".to_string(),
      help_url: "https://dprint.dev/plugins/markdown".to_string(),
      config_schema_url: format!(
        "https://plugins.dprint.dev/dprint/dprint-plugin-markdown/{}/schema.json",
        version
      ),
      update_url: Some("https://plugins.dprint.dev/dprint/dprint-plugin-markdown/latest.json".to_string()),
    }
  }

  fn license_text(&mut self) -> String {
    std::str::from_utf8(include_bytes!("../LICENSE")).unwrap().into()
  }

  fn format(
    &mut self,
    request: SyncFormatRequest<Configuration>,
    mut format_with_host: impl FnMut(SyncHostFormatRequest) -> FormatResult,
  ) -> FormatResult {
    let file_text = String::from_utf8(request.file_bytes)?;
    return super::format_text(&file_text, request.config, |tag, file_text, line_width| {
      if let Some(ext) = tag_to_extension(tag) {
        let file_path = PathBuf::from(format!("file.{}", ext));
        let mut additional_config = ConfigKeyMap::new();
        additional_config.insert("lineWidth".into(), (line_width as i32).into());
        let request = SyncHostFormatRequest {
          file_path: &file_path,
          file_bytes: file_text.as_bytes(),
          range: FormatRange::None,
          override_config: &additional_config,
        };
        let result = format_with_host(request);
        match result {
          Ok(Some(bytes)) => Ok(Some(String::from_utf8(bytes)?)),
          Ok(None) => Ok(None),
          Err(err) => Err(err),
        }
      } else {
        Ok(None)
      }
    })
    .map(|maybe_text| maybe_text.map(|t| t.into_bytes()));

    fn tag_to_extension(tag: &str) -> Option<&'static str> {
      match tag.trim().to_lowercase().as_str() {
        "typescript" | "ts" => Some("ts"),
        "tsx" => Some("tsx"),
        "javascript" | "js" => Some("js"),
        "jsx" => Some("jsx"),
        "json" => Some("json"),
        "jsonc" => Some("jsonc"),
        "rust" | "rs" => Some("rs"),
        "csharp" | "cs" => Some("cs"),
        "visualbasic" | "vb" => Some("vb"),
        "css" => Some("css"),
        "less" => Some("less"),
        "toml" => Some("toml"),
        "scss" => Some("scss"),
        "svelte" => Some("svelte"),
        "vue" => Some("vue"),
        "astro" => Some("astro"),
        "xml" => Some("xml"),
        "yml" | "yaml" => Some("yaml"),
        "graphql" => Some("graphql"),
        "python" | "py" => Some("py"),
        "dockerfile" => Some("dockerfile"),
        _ => None,
      }
    }
  }
}

generate_plugin_code!(MarkdownPluginHandler, MarkdownPluginHandler);
