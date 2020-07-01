use dprint_core::*;
use dprint_core::configuration::{resolve_new_line_kind};

use super::configuration::Configuration;
use super::parsing::{parse_cmark_ast, parse_yaml_header, parse_node, Context};

/// Formats a file.
///
/// Returns the file text or an error when it failed to parse.
pub fn format_text(file_text: &str, config: &Configuration) -> Result<String, String> {
    let yaml_header = parse_yaml_header(file_text); // todo: improve... this is kind of hacked into here
    let markdown_text = match &yaml_header {
        Some(yaml_header) => &file_text[yaml_header.range.end..],
        None => file_text,
    };
    let source_file = match parse_cmark_ast(markdown_text) {
        Ok(source_file) => {
            let mut source_file = source_file;
            source_file.yaml_header = yaml_header;
            source_file
        },
        Err(error) => {
            return Err(dprint_core::utils::string_utils::format_diagnostic(
                Some((error.range.start, error.range.end)),
                &error.message,
                file_text
            ));
        }
    };
    let print_items = parse_node(&source_file.into(), &mut Context::new(markdown_text, config));

    // println!("{}", print_items.get_as_text());

    Ok(print(print_items, PrintOptions {
        indent_width: 1, // force
        max_width: config.line_width,
        use_tabs: false, // ignore tabs, always use spaces
        new_line_text: resolve_new_line_kind(file_text, config.new_line_kind),
    }))
}
