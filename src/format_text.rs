use std::cell::RefCell;
use std::rc::Rc;

use dprint_core::configuration::resolve_new_line_kind;
use dprint_core::formatting::*;

use super::configuration::Configuration;
use super::generation::count_new_lines;
use super::generation::generate;
use super::generation::is_ignore_comment;
use super::generation::strip_metadata_header;
use super::generation::Context;

/// Error that can occur while formatting markdown text.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
  /// The text could not be parsed as markdown.
  ///
  /// Whatever the parser can't make sense of is read as the paragraphs it
  /// renders as, so this is only for a document written in a way the parser
  /// won't follow at all -- blocks nested more deeply than it will go.
  #[error("{0}")]
  Parse(String),
  /// An error occurred while formatting the text of a code block.
  #[error("{0}")]
  CodeBlock(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl FormatError {
  /// Says which line of the file the code block that failed to format was
  /// written on.
  fn in_code_block_on_line(self, line: u32) -> FormatError {
    FormatError::CodeBlock(Box::new(CodeBlockErrorOnLine { line, error: self }))
  }
}

impl From<std::string::FromUtf8Error> for FormatError {
  fn from(err: std::string::FromUtf8Error) -> Self {
    FormatError::CodeBlock(err.into())
  }
}

/// Formats a file.
///
/// Returns the formatted text, or `None` when it's the same as what was given.
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
  let full_text = file_text;
  let file_text = strip_bom(file_text);
  // the lines taken off the front of the file, which the lines the rest of it
  // is reported on are still counted from
  let stripped_lines = count_new_lines(&full_text[..full_text.len() - file_text.len()]);
  let (source_file, markdown_text) = match parse_source_file(file_text, config)? {
    ParseFileResult::IgnoreFile => return Ok(None),
    ParseFileResult::SourceFile(file) => file,
  };

  // a code block whose plugin errors can't fail the file from where it's
  // generated, so the error is carried out of the generation and raised here
  let code_block_error = Rc::new(RefCell::new(None));
  let text = dprint_core::formatting::format(
    || {
      let mut context = Context::new(markdown_text, config, format_code_block_text, code_block_error.clone());
      generate(&source_file.into(), &mut context)
    },
    config_to_print_options(file_text, config),
  );

  let code_block_error = code_block_error.borrow_mut().take();
  match code_block_error {
    Some(code_block_error) => {
      let line = stripped_lines + count_new_lines(&markdown_text[..code_block_error.pos]) + 1;
      Err(code_block_error.error.in_code_block_on_line(line))
    }
    None => Ok(Some(text)),
  }
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
      let mut context = Context::new(markdown_text, config, format_code_block_text, Default::default());
      generate(&source_file.into(), &mut context)
    },
    config_to_print_options(file_text, config),
  )
}

/// Removes the byte order marks a file begins with, along with any whitespace
/// written before them.
///
/// More than one is removed, and the whitespace with them, because taking off
/// only the first would leave another at the start of the file once the
/// whitespace around it was written out -- for the next run to take off in
/// turn. Text that begins with no mark at all is left as it is.
fn strip_bom(text: &str) -> &str {
  let mut rest = text;
  loop {
    let trimmed = rest.trim_start_matches([' ', '\t', '\n', '\r']);
    match trimmed.strip_prefix('\u{FEFF}') {
      Some(after) => rest = after,
      None => return rest,
    }
  }
}

enum ParseFileResult<'a> {
  IgnoreFile,
  SourceFile((crate::generation::common::SourceFile<'a>, &'a str)),
}

fn parse_source_file<'a>(file_text: &'a str, config: &Configuration) -> Result<ParseFileResult<'a>, FormatError> {
  // check for the presence of a dprint-ignore-file comment before parsing
  if is_ignore_comment(strip_metadata_header(file_text), &config.ignore_file_directive) {
    return Ok(ParseFileResult::IgnoreFile);
  }

  let file = crate::parser::parse(file_text).map_err(|err| FormatError::Parse(err.to_string()))?;
  Ok(ParseFileResult::SourceFile((file, file_text)))
}

/// An error a code block's plugin ran into, along with where in the file the
/// code block was written.
#[derive(Debug, thiserror::Error)]
#[error("the code block on line {line} failed to format\n\n{error}")]
struct CodeBlockErrorOnLine {
  /// The line the code block starts on, counting from one.
  line: u32,
  error: FormatError,
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
    for input_text in [
      "\u{FEFF}#  Title",
      "\u{FEFF}# Title\n",
      "\u{FEFF}\u{FEFF}# Title\n",
      "  \u{FEFF}# Title\n",
      "\u{FEFF}\n\u{FEFF}# Title\n",
    ] {
      let config = ConfigurationBuilder::new().build();
      let result = format_text(input_text, &config, |_, _, _| Ok(None)).unwrap();
      assert_eq!(result, Some("# Title\n".to_string()));
    }
  }

  #[test]
  fn reports_the_line_the_code_block_starts_on() {
    for (file_text, line) in [
      ("# Title\n\n```error\nnot  code\n```\n", 3),
      ("# Title\r\n\r\n```error\r\nnot  code\r\n```\r\n", 3),
      ("---\na: b\n---\n\n# Title\n\n```error\nnot  code\n```\n", 7),
      // the lines taken off with the byte order marks still count
      ("\n\n\u{FEFF}# Title\n\n```error\nnot  code\n```\n", 5),
    ] {
      let error = format_failing_code_block(file_text, &raise_errors_config())
        .err()
        .unwrap();
      assert_eq!(
        error.to_string(),
        format!("the code block on line {} failed to format\n\nsyntax error", line)
      );
    }
  }

  #[test]
  fn leaves_code_block_as_written_when_not_enabled() {
    let config = ConfigurationBuilder::new().build();
    let result = format_failing_code_block("#  Title\n\n```error\nnot  code\n```\n", &config).unwrap();
    assert_eq!(result, Some("# Title\n\n```error\nnot  code\n```\n".to_string()));
  }

  #[test]
  fn does_not_error_when_code_blocks_are_not_formatted() {
    let config = ConfigurationBuilder::new()
      .code_block_raise_syntax_errors(true)
      .code_block_skip_format(true)
      .build();
    let result = format_failing_code_block("# Title\n\n```error\nnot  code\n```\n", &config).unwrap();
    assert_eq!(result, None);
  }

  #[test]
  fn does_not_error_on_ignored_code_block() {
    let file_text = "<!-- dprint-ignore -->\n\n```error\nnot  code\n```\n";
    let result = format_failing_code_block(file_text, &raise_errors_config()).unwrap();
    assert_eq!(result, None);
  }

  #[test]
  fn errors_on_code_block_within_a_markdown_code_block() {
    let file_text = r#"````md
# Title

```error
not  code
```
````
"#;
    let error = format_failing_code_block(file_text, &raise_errors_config())
      .err()
      .unwrap();
    // the line the inner error reports is the line of the markdown the code
    // block holds, rather than a line of the file it's written in
    assert_eq!(
      error.to_string(),
      concat!(
        "the code block on line 1 failed to format\n\n",
        "the code block on line 3 failed to format\n\nsyntax error"
      )
    );
  }

  /// A configuration that fails a file for an error a code block's plugin runs
  /// into.
  fn raise_errors_config() -> Configuration {
    ConfigurationBuilder::new().code_block_raise_syntax_errors(true).build()
  }

  /// Formats a file, with the plugin of any code block tagged `error` failing
  /// to format the code it holds.
  fn format_failing_code_block(file_text: &str, config: &Configuration) -> Result<Option<String>, FormatError> {
    format_text(file_text, config, |tag, _, _| {
      if tag == "error" {
        Err(FormatError::CodeBlock("syntax error".into()))
      } else {
        Ok(None)
      }
    })
  }
}
