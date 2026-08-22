use dprint_core::configuration::resolve_new_line_kind;
use dprint_core::formatting::*;

use super::configuration::Configuration;
use super::generation::file_has_ignore_file_directive;
use super::generation::generate;
use super::generation::parse_cmark_ast;
use super::generation::strip_metadata_header;
use super::generation::Context;

/// Error that can occur while formatting markdown text.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
  /// The text could not be parsed as markdown.
  #[error("{0}")]
  Parse(String),
  /// An error occurred while formatting the text of a code block.
  #[error("{0}")]
  CodeBlock(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl From<std::string::FromUtf8Error> for FormatError {
  fn from(err: std::string::FromUtf8Error) -> Self {
    FormatError::CodeBlock(err.into())
  }
}

/// Formats a file.
///
/// Returns the file text or an error when it failed to parse.
pub fn format_text(
  file_text: &str,
  config: &Configuration,
  format_code_block_text: impl for<'a> FnMut(&str, &'a str, u32) -> Result<Option<String>, FormatError>,
) -> Result<Option<String>, FormatError> {
  let result = format_text_inner(file_text, config, format_code_block_text)?;

  match result {
    Some(result) if result == file_text => Ok(None),
    Some(result) => Ok(Some(result)),
    None => Ok(None),
  }
}

fn format_text_inner(
  file_text: &str,
  config: &Configuration,
  format_code_block_text: impl for<'a> FnMut(&str, &'a str, u32) -> Result<Option<String>, FormatError>,
) -> Result<Option<String>, FormatError> {
  let file_text = strip_bom(file_text);
  let (source_file, markdown_text) = match parse_source_file(file_text, config)? {
    ParseFileResult::IgnoreFile => return Ok(None),
    ParseFileResult::SourceFile(file) => file,
  };

  Ok(Some(dprint_core::formatting::format(
    || {
      let mut context = Context::new(markdown_text, config, format_code_block_text);
      #[allow(clippy::let_and_return)]
      let print_items = generate(&source_file.into(), &mut context);
      // eprintln!("{}", print_items.get_as_text());
      print_items
    },
    config_to_print_options(file_text, config),
  )))
}

#[cfg(feature = "tracing")]
pub fn trace_file(
  file_text: &str,
  config: &Configuration,
  format_code_block_text: impl for<'a> FnMut(&str, &'a str, u32) -> Result<Option<String>, FormatError>,
) -> dprint_core::formatting::TracingResult {
  let (source_file, markdown_text) = match parse_source_file(file_text, config).unwrap() {
    ParseFileResult::IgnoreFile => panic!("Cannot trace file because it has an ignore file comment."),
    ParseFileResult::SourceFile(file) => file,
  };
  dprint_core::formatting::trace_printing(
    || {
      let mut context = Context::new(markdown_text, config, format_code_block_text);
      #[allow(clippy::let_and_return)]
      let print_items = generate(&source_file.into(), &mut context);
      // eprintln!("{}", print_items.get_as_text());
      print_items
    },
    config_to_print_options(file_text, config),
  )
}

fn strip_bom(text: &str) -> &str {
  text.strip_prefix("\u{FEFF}").unwrap_or(text)
}

enum ParseFileResult<'a> {
  IgnoreFile,
  SourceFile((crate::generation::common::SourceFile, &'a str)),
}

fn parse_source_file<'a>(file_text: &'a str, config: &Configuration) -> Result<ParseFileResult<'a>, FormatError> {
  // check for the presence of a dprint-ignore-file comment before parsing
  if file_has_ignore_file_directive(strip_metadata_header(file_text), &config.ignore_file_directive) {
    return Ok(ParseFileResult::IgnoreFile);
  }

  match parse_cmark_ast(file_text) {
    Ok(source_file) => Ok(ParseFileResult::SourceFile((source_file, file_text))),
    Err(error) => Err(FormatError::Parse(
      dprint_core::formatting::utils::string_utils::format_diagnostic(
        Some((error.range.start, error.range.end)),
        &error.message,
        file_text,
      ),
    )),
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

#[cfg(test)]
mod test {
  use super::*;
  use crate::configuration::ConfigurationBuilder;

  #[test]
  fn strips_bom() {
    for input_text in ["\u{FEFF}#  Title", "\u{FEFF}# Title\n"] {
      let config = ConfigurationBuilder::new().build();
      let result = format_text(input_text, &config, |_, _, _| Ok(None)).unwrap();
      assert_eq!(result, Some("# Title\n".to_string()));
    }
  }
}
