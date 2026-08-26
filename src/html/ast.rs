//! AST types for the html parser.
//!
//! Every node borrows from the html text it was parsed out of, so a document
//! only allocates for the vectors that hold its children and attributes. The
//! text of a node is kept exactly as it was written -- nothing is unescaped or
//! normalized -- because the formatter has to be able to write back out what it
//! couldn't improve on.

/// A parsed html fragment.
#[derive(Debug, PartialEq, Eq)]
pub struct Document<'a> {
  pub children: Vec<Node<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Node<'a> {
  Element(Element<'a>),
  /// Character data between tags, as it was written.
  Text(&'a str),
  /// A `<!-- -->` comment, including its delimiters.
  Comment(&'a str),
  /// A `<!DOCTYPE html>` or other `<!` declaration, including its delimiters.
  Doctype(&'a str),
  /// A `<? ?>` processing instruction, including its delimiters.
  ProcessingInstruction(&'a str),
  /// A `<![CDATA[ ]]>` section, including its delimiters.
  CData(&'a str),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Element<'a> {
  /// The tag name as it was written, which may be in any case.
  pub name: &'a str,
  pub attributes: Vec<Attribute<'a>>,
  pub children: Vec<Node<'a>>,
  /// The text between the element's tags, exactly as it was written, which is
  /// what a preformatted or raw text element is written back out as. It is
  /// empty for an element that holds nothing, but still borrowed from where
  /// that content would have been, so its position can be recovered.
  pub content: &'a str,
  /// The whole of the element as it was written, from the `<` of its open tag
  /// to the end of whatever closes it.
  pub source: &'a str,
  pub kind: ElementKind,
  /// Whether the open tag was written with a `/>`. It is kept because a void
  /// element written as `<br />` renders the same as one written as `<br>`
  /// and neither is worth rewriting as the other. The space before the slash
  /// is written the way the configuration says, whatever was written here.
  pub self_closing_syntax: bool,
}

impl<'a> Element<'a> {
  /// The name written in the element's close tag, which may be in a different
  /// case than the open tag's. An element written without one has only the
  /// open tag's name.
  pub fn close_tag_name(&self) -> &'a str {
    match self.kind {
      ElementKind::Normal | ElementKind::RawText => self
        .source
        .rsplit_once("</")
        .map(|(_, close)| close.trim_end_matches('>').trim_end())
        .unwrap_or(self.name),
      ElementKind::Void | ElementKind::SelfClosing => self.name,
    }
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ElementKind {
  /// An element written with both tags (ex. `<p>a</p>`).
  Normal,
  /// A void element, which is written with no closing tag at all and can hold
  /// no content (ex. `<br>`).
  Void,
  /// An element written with a self-closing tag (ex. `<circle />`). Only
  /// foreign elements and void elements are read this way, because a `/>` on
  /// an ordinary html element opens it rather than closing it.
  SelfClosing,
  /// An element whose content is character data rather than markup, which is
  /// held as a single [`Node::Text`] child (ex. `<script>`).
  RawText,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Attribute<'a> {
  /// The attribute name as it was written, which may be in any case.
  pub name: &'a str,
  pub value: Option<AttributeValue<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AttributeValue<'a> {
  /// The value between the quotes, or the bare value when there are none.
  pub text: &'a str,
  /// The quote character the value was written with, if any.
  pub quote: Option<char>,
}

/// What the spec tests read to say where a node was parsed from. The
/// formatter itself works from the nodes rather than their positions, so this
/// is built only along with the tests that check them.
#[cfg(test)]
impl<'a> Node<'a> {
  /// The whole of the node as it was written.
  pub fn source(&self) -> &'a str {
    match self {
      Node::Element(element) => element.source,
      Node::Text(text)
      | Node::Comment(text)
      | Node::Doctype(text)
      | Node::ProcessingInstruction(text)
      | Node::CData(text) => text,
    }
  }

  pub fn kind(&self) -> &'static str {
    match self {
      Node::Element(_) => "Element",
      Node::Text(_) => "Text",
      Node::Comment(_) => "Comment",
      Node::Doctype(_) => "Doctype",
      Node::ProcessingInstruction(_) => "ProcessingInstruction",
      Node::CData(_) => "CData",
    }
  }
}

/// The html the parser wouldn't read, which leaves the text to be written back
/// out as it was.
///
/// None of these are errors in the sense that the file is wrong -- an html
/// block in markdown is very often a fragment, because a blank line closes the
/// block and leaves its closing tag to a block of its own. They only say that
/// this text isn't something the formatter can take apart and put back
/// together with confidence.
///
/// The tag names are borrowed from the html rather than copied out of it,
/// because html the formatter leaves alone is the common case in markdown and
/// there is nothing to say about it that the text doesn't already hold.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError<'a> {
  /// An element was opened and never closed (ex. `<div>` on its own).
  UnclosedElement { name: &'a str },
  /// A closing tag was written for an element that isn't open (ex. `</div>` on
  /// its own).
  UnexpectedClosingTag { name: &'a str },
  /// A closing tag doesn't match the element it closes, which means the html
  /// relies on a browser inferring where the elements end.
  MismatchedClosingTag { expected: &'a str, found: &'a str },
  /// A `<!--`, `<![CDATA[`, `<?` or quoted attribute value runs to the end of
  /// the text without being closed.
  UnterminatedMarkup,
  /// A tag is written in a way the parser doesn't read, ex. `<div/>`, which a
  /// browser reads as opening a div rather than as an empty one.
  MalformedTag,
  /// Laying the html out would leave a blank line in it, which closes the html
  /// block it's written in and would leave the rest of it to be read as
  /// markdown.
  WouldSplitTheBlock,
}

impl std::fmt::Display for ParseError<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ParseError::UnclosedElement { name } => write!(f, "`<{}>` was never closed", name),
      ParseError::UnexpectedClosingTag { name } => write!(f, "`</{}>` closes an element that isn't open", name),
      ParseError::MismatchedClosingTag { expected, found } => {
        write!(f, "`</{}>` was found where `</{}>` was expected", found, expected)
      }
      ParseError::UnterminatedMarkup => write!(f, "markup runs past the end of the text without being closed"),
      ParseError::MalformedTag => write!(f, "a tag is written in a way the parser doesn't read"),
      ParseError::WouldSplitTheBlock => write!(f, "laying the html out would leave a blank line in it"),
    }
  }
}
