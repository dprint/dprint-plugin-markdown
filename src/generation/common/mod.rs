pub use crate::parser::*;

/// The checks the formatter makes about the text surrounding a node in order
/// to decide how to space it out.
pub trait NodeSurroundings {
  fn has_preceding_space(&self, file_text: &str) -> bool;
  fn has_preceding_whitespace(&self, file_text: &str) -> bool;
  fn starts_with_punctuation(&self, file_text: &str) -> bool;
  fn ends_with_punctuation(&self, file_text: &str) -> bool;
  fn starts_with_unspaced_script(&self, file_text: &str) -> bool;
  fn ends_with_unspaced_script(&self, file_text: &str) -> bool;
}

impl<T: Ranged> NodeSurroundings for T {
  /// Whether a space separates this node from what is written before it, which
  /// a tab is as much as a space.
  fn has_preceding_space(&self, file_text: &str) -> bool {
    let start = self.span().start;
    start > 0 && matches!(file_text.as_bytes()[start - 1], b' ' | b'\t')
  }

  /// Whether any whitespace, not only a space, comes before this node.
  fn has_preceding_whitespace(&self, file_text: &str) -> bool {
    matches!(file_text[..self.span().start].chars().next_back(), Some(c) if c.is_whitespace())
  }

  fn starts_with_punctuation(&self, file_text: &str) -> bool {
    matches!(self.span().text(file_text).chars().next(), Some(c) if c.is_ascii_punctuation())
  }

  fn ends_with_punctuation(&self, file_text: &str) -> bool {
    matches!(self.span().text(file_text).chars().last(), Some(c) if c.is_ascii_punctuation())
  }

  /// Whether the node begins with a character of a script written without
  /// spaces between its words.
  fn starts_with_unspaced_script(&self, file_text: &str) -> bool {
    matches!(self.span().text(file_text).chars().next(), Some(c) if crate::generation::utils::is_unspaced_script(c))
  }

  /// Whether the node ends with a character of a script written without spaces
  /// between its words.
  fn ends_with_unspaced_script(&self, file_text: &str) -> bool {
    matches!(self.span().text(file_text).chars().last(), Some(c) if crate::generation::utils::is_unspaced_script(c))
  }
}

impl<'a> Node<'a> {
  /// Whether the node is text that starts with a word that would become a list
  /// marker if it were moved to the start of a line.
  pub fn starts_with_list_word(&self) -> bool {
    self.first_word().is_some_and(crate::generation::utils::is_list_word)
  }

  /// Whether the node is text that would start a new block if it were moved to
  /// the start of a line, which the formatter can't do to it.
  pub fn starts_block_in_paragraph(&self) -> bool {
    match self {
      Node::Text(text) => crate::parser::starts_block_in_paragraph(text.text),
      // a block of html already stands on its own, so nothing about where it
      // is written can turn it into one
      Node::Html(html) => !html.is_block && crate::parser::starts_block_in_paragraph(&html.text),
      _ => false,
    }
  }

  /// Whether the node would start a block of its own, or turn the line above
  /// it into one, if it were moved to the start of a line by wrapping.
  ///
  /// Unlike [`Self::starts_block_in_paragraph`] this counts what the node's
  /// first word would start on its own, since the text after it can wrap onto
  /// the line below and leave it there alone.
  pub fn starts_block_at_line_start(&self, word_can_be_left_alone: bool) -> bool {
    match self {
      Node::Text(text) => crate::generation::utils::starts_block_at_line_start(text.text, word_can_be_left_alone),
      // a block of html already stands on its own, so nothing about where it
      // is written can turn it into one
      Node::Html(html) => {
        !html.is_block && crate::generation::utils::starts_block_at_line_start(&html.text, word_can_be_left_alone)
      }
      _ => false,
    }
  }

  fn first_word(&self) -> Option<&'a str> {
    let Node::Text(text) = self else {
      return None;
    };
    text.text.split(char::is_whitespace).next()
  }
}
