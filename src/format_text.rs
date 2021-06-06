use dprint_core::formatting::*;
use dprint_core::configuration::{resolve_new_line_kind};
use dprint_core::types::ErrBox;

use super::configuration::Configuration;
use super::parsing::{parse_cmark_ast, parse_yaml_header, parse_node, Context, file_has_ignore_file_directive};

/// Formats a file.
///
/// Returns the file text or an error when it failed to parse.
pub fn format_text(
    file_text: &str,
    config: &Configuration,
    format_code_block_text: impl FnMut(&str, &str, u32) -> Result<String, ErrBox>,
) -> Result<String, ErrBox> {
    let (source_file, markdown_text) = match parse_source_file(file_text, config)? {
        ParseFileResult::IgnoreFile => return Ok(file_text.to_string()),
        ParseFileResult::SourceFile(file) => file,
    };

    Ok(dprint_core::formatting::format(|| {
        let mut context = Context::new(&markdown_text, config, format_code_block_text);
        let print_items = parse_node(&source_file.into(), &mut context);
        // println!("{}", print_items.get_as_text());
        print_items
    }, config_to_print_options(file_text, config)))
}


#[cfg(feature = "tracing")]
pub fn trace_file(
    file_text: &str,
    config: &Configuration,
    format_code_block_text: Box<dyn Fn(&str, &str, u32) -> Result<String, String>>,
) -> dprint_core::formatting::TracingResult {
    let (source_file, markdown_text) = match parse_source_file(file_text, config).unwrap() {
        ParseFileResult::IgnoreFile => panic!("Cannot trace file because it has an ignore file comment."),
        ParseFileResult::SourceFile(file) => file,
    };
    dprint_core::formatting::trace_printing(
        || {
            let mut context = Context::new(&markdown_text, config, format_code_block_text);
            let print_items = parse_node(&source_file.into(), &mut context);
            // println!("{}", print_items.get_as_text());
            print_items
        },
        config_to_print_options(file_text, config),
    )
}

enum ParseFileResult<'a> {
    IgnoreFile,
    SourceFile((crate::parsing::common::SourceFile, &'a str)),
}

fn parse_source_file<'a>(
    file_text: &'a str,
    config: &Configuration,
) -> Result<ParseFileResult<'a>, String> {
    let yaml_header = parse_yaml_header(file_text); // todo: improve... this is kind of hacked into here
    let markdown_text = match &yaml_header {
        Some(yaml_header) => &file_text[yaml_header.range.end..],
        None => file_text,
    };

    // check for the presence of an dprint-ignore-file comment before parsing
    if file_has_ignore_file_directive(&markdown_text, &config.ignore_file_directive) {
        return Ok(ParseFileResult::IgnoreFile);
    }

    match parse_cmark_ast(markdown_text) {
        Ok(source_file) => {
            let mut source_file = source_file;
            source_file.yaml_header = yaml_header;
            Ok(ParseFileResult::SourceFile((source_file, markdown_text)))
        },
        Err(error) => {
            Err(dprint_core::formatting::utils::string_utils::format_diagnostic(
                Some((error.range.start, error.range.end)),
                &error.message,
                file_text
            ))
        }
    }
}

fn config_to_print_options(file_text: &str, config: &Configuration) -> PrintOptions {
    PrintOptions {
        indent_width: 1, // force
        max_width: config.line_width,
        use_tabs: false, // ignore tabs, always use spaces
        new_line_text: resolve_new_line_kind(file_text, config.new_line_kind),
    }
}