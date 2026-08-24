pub use crate::parser::*;

/// The checks the formatter makes about the text surrounding a node in order
/// to decide how to space it out.
pub trait NodeSurroundings {
  fn has_preceding_space(&self, file_text: &str) -> bool;
  fn has_preceding_whitespace(&self, file_text: &str) -> bool;
  fn starts_with_punctuation(&self, file_text: &str) -> bool;
  fn ends_with_punctuation(&self, file_text: &str) -> bool;
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
}

impl<'a> Node<'a> {
  /// The first character of the node that a reader sees, which is what the
  /// whitespace before the node reads against.
  ///
  /// The delimiters a node is written with (ex. the `[` and `](url)` around a
  /// link) say how it's read rather than being read themselves, so they're
  /// looked through to the text within them. A node that isn't read as text of
  /// its own (ex. an image, which stands in the line as a picture) has none,
  /// and so does a link whose text begins with one -- what's read there is the
  /// text of that node, not of the one after it.
  ///
  /// Whether a decoration read through this way keeps reading as one once the
  /// whitespace beside it is taken away is decided separately, since which
  /// character delimits it is chosen by what ends up written against it.
  pub fn first_read_char(&self) -> Option<char> {
    match self {
      Node::Text(node) => node.text.chars().next(),
      Node::Code(node) => node.code.chars().next(),
      Node::TextDecoration(node) => node.children.first().and_then(|child| child.first_read_char()),
      Node::InlineLink(node) => node.children.first().and_then(|child| child.first_read_char()),
      Node::ReferenceLink(node) => node.children.first().and_then(|child| child.first_read_char()),
      Node::ShortcutLink(node) => node.children.first().and_then(|child| child.first_read_char()),
      _ => None,
    }
  }

  /// The last character of the node that a reader sees, which is what the
  /// whitespace after the node reads against. See [`Self::first_read_char`].
  pub fn last_read_char(&self) -> Option<char> {
    match self {
      Node::Text(node) => node.text.chars().last(),
      Node::Code(node) => node.code.chars().last(),
      Node::TextDecoration(node) => node.children.last().and_then(|child| child.last_read_char()),
      Node::InlineLink(node) => node.children.last().and_then(|child| child.last_read_char()),
      Node::ReferenceLink(node) => node.children.last().and_then(|child| child.last_read_char()),
      Node::ShortcutLink(node) => node.children.last().and_then(|child| child.last_read_char()),
      _ => None,
    }
  }

  /// Whether the character the node begins with belongs to a script written
  /// without spaces between its words.
  pub fn starts_with_unspaced_script(&self) -> bool {
    matches!(self.first_read_char(), Some(c) if crate::generation::utils::is_unspaced_script(c))
  }

  /// Whether the character the node ends with belongs to a script written
  /// without spaces between its words.
  pub fn ends_with_unspaced_script(&self) -> bool {
    matches!(self.last_read_char(), Some(c) if crate::generation::utils::is_unspaced_script(c))
  }

  /// Whether a sentence ends with this node, which is where a line break is
  /// written when text is wrapped by sentence.
  pub fn ends_sentence(&self) -> bool {
    self
      .last_read_text()
      .is_some_and(crate::generation::utils::ends_sentence)
  }

  /// Whether the node could begin a sentence.
  pub fn starts_sentence(&self) -> bool {
    self
      .first_read_text()
      .is_some_and(crate::generation::utils::starts_sentence)
  }

  /// The text the node begins with that a reader sees, which is what tells
  /// whether a sentence could begin with it.
  ///
  /// The delimiters a node is written with are looked through the way
  /// [`Self::first_read_char`] looks through them, and so are the text
  /// decorations: which character delimits one says nothing about the sentence
  /// written within it, so a sentence that begins in emphasis (ex. `**Done.**`)
  /// begins where its text does.
  fn first_read_text(&self) -> Option<&str> {
    match self {
      Node::Text(node) => Some(node.text),
      Node::Code(node) => Some(&node.code),
      Node::TextDecoration(node) => node.children.first().and_then(|child| child.first_read_text()),
      Node::InlineLink(node) => node.children.first().and_then(|child| child.first_read_text()),
      Node::ReferenceLink(node) => node.children.first().and_then(|child| child.first_read_text()),
      Node::ShortcutLink(node) => node.children.first().and_then(|child| child.first_read_text()),
      _ => None,
    }
  }

  /// The text the node ends with that a reader sees, which is what tells
  /// whether a sentence ends with it. See [`Self::first_read_text`].
  fn last_read_text(&self) -> Option<&str> {
    match self {
      Node::Text(node) => Some(node.text),
      Node::Code(node) => Some(&node.code),
      Node::TextDecoration(node) => node.children.last().and_then(|child| child.last_read_text()),
      Node::InlineLink(node) => node.children.last().and_then(|child| child.last_read_text()),
      Node::ReferenceLink(node) => node.children.last().and_then(|child| child.last_read_text()),
      Node::ShortcutLink(node) => node.children.last().and_then(|child| child.last_read_text()),
      _ => None,
    }
  }

  /// Whether the node is text that starts with a word that would become a list
  /// marker if it were moved to the start of a line.
  pub fn starts_with_list_word(&self) -> bool {
    self.first_word().is_some_and(crate::generation::utils::is_list_word)
  }

  /// Whether the node would begin a block of its own where a block begins,
  /// rather than being read as the paragraph it's written as (ex. the text a
  /// setext heading's underline is written below).
  pub fn starts_block_at_block_start(&self) -> bool {
    match self {
      Node::Text(text) => crate::parser::starts_block_at_block_start(text.text),
      Node::Html(html) => !html.is_block && crate::parser::starts_block_at_block_start(&html.text),
      _ => false,
    }
  }

  /// Whether the node would start a block of its own, or turn the line above
  /// it into one, if it were moved to the start of a line by wrapping.
  ///
  /// Unlike [`Self::starts_block_in_paragraph`] this counts what the node's
  /// first word would start on its own, since the text after it can wrap onto
  /// the line below and leave it there alone.
  pub fn starts_block_at_line_start(&self, following_text: &str, word_can_be_left_alone: bool) -> bool {
    match self {
      Node::Text(text) => {
        crate::generation::utils::starts_block_at_line_start(text.text, following_text, word_can_be_left_alone)
      }
      // a block of html already stands on its own, so nothing about where it
      // is written can turn it into one
      Node::Html(html) => {
        !html.is_block
          && crate::generation::utils::starts_block_at_line_start(&html.text, following_text, word_can_be_left_alone)
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
