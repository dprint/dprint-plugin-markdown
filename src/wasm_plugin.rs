use std::path::PathBuf;
use dprint_core::generate_plugin_code;
use super::configuration::{Configuration, resolve_config};

fn get_plugin_config_key() -> String {
    String::from("markdown")
}

fn get_plugin_file_extensions() -> Vec<String> {
    vec![String::from("md")]
}

fn format_text(_: &PathBuf, file_text: &str, config: &Configuration) -> Result<String, String> {
    return super::format_text(
        file_text,
        config,
        Box::new(|tag, file_text| {
            if let Some(ext) = tag_to_extension(tag) {
                let file_path = PathBuf::from(format!("file.{}", ext));
                format_with_host(&file_path, file_text.to_string())
            } else {
                Ok(file_text.to_string())
            }
        })
    );

    fn tag_to_extension(tag: &str) -> Option<&'static str> {
        match tag.trim().to_lowercase().as_str() {
            "typescript" | "ts" => Some("ts"),
            "tsx" => Some("tsx"),
            "javascript" | "js" => Some("js"),
            "jsx" => Some("jsx"),
            "json" | "jsonc" => Some("json"),
            "rust" | "rs" => Some("rs"),
            _ => None,
        }
    }
}

fn get_plugin_help_url() -> String {
    String::from("https://dprint.dev/plugins/markdown")
}

fn get_plugin_config_schema_url() -> String {
    String::new() // none until https://github.com/microsoft/vscode/issues/98443 is resolved
}

fn get_plugin_license_text() -> String {
    std::str::from_utf8(include_bytes!("../LICENSE")).unwrap().into()
}

generate_plugin_code!();
