mod ast_nodes;
mod char_scanner;

pub use ast_nodes::*;
pub use char_scanner::*;

pub struct ParseError {
  /// This range the parse error occurred.
  pub range: Range,
  /// The associated error message.
  pub message: String,
}

impl ParseError {
  pub(super) fn new(range: Range, message: &str) -> ParseError {
    ParseError {
      range,
      message: String::from(message),
    }
  }
}
