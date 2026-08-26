//! An html parser written for formatting rather than rendering.
//!
//! It reads only the html it can put back together with confidence. Where a
//! browser would infer a closing tag that wasn't written, or read a tag in a
//! way that depends on what came before it, this parser stops with a
//! [`ParseError`] instead and leaves the text to be written out as it was. An
//! html block in markdown is very often a fragment -- a blank line closes the
//! block, so `<div>` and its `</div>` end up in separate blocks -- and
//! refusing those is what keeps the formatter from closing a tag the author
//! deliberately left open.

use super::ast::*;
use super::tags;

/// Parses an html fragment.
pub fn parse(text: &str) -> Result<Document<'_>, ParseError<'_>> {
  Parser { text }.parse_document()
}

struct Parser<'a> {
  text: &'a str,
}

/// An element that has been opened and is waiting for its closing tag.
struct Frame<'a> {
  name: &'a str,
  attributes: Vec<Attribute<'a>>,
  children: Vec<Node<'a>>,
  /// The position the element's open tag starts at.
  start: usize,
  /// The position the element's content starts at, which is just past its open
  /// tag.
  content_start: usize,
  /// Whether the element's content is svg or mathml rather than html, where a
  /// self-closing tag closes the element it's written on.
  foreign: bool,
}

/// What was read at a `<`, along with the position just past it.
struct Markup<'a> {
  kind: MarkupKind<'a>,
  end: usize,
}

enum MarkupKind<'a> {
  OpenTag {
    name: &'a str,
    attributes: Vec<Attribute<'a>>,
    self_closing: bool,
  },
  CloseTag {
    name: &'a str,
  },
  /// A comment, declaration, processing instruction or CDATA section, which
  /// the formatter keeps exactly as it was written.
  Verbatim(VerbatimKind),
}

/// Markup that is written back out as it was, and so is only told apart by
/// what the printer calls it.
#[derive(Clone, Copy)]
enum VerbatimKind {
  Comment,
  Doctype,
  ProcessingInstruction,
  CData,
}

impl<'a> Parser<'a> {
  fn parse_document(&self) -> Result<Document<'a>, ParseError<'a>> {
    let bytes = self.text.as_bytes();
    let mut stack: Vec<Frame<'a>> = Vec::new();
    let mut root: Vec<Node<'a>> = Vec::new();
    let mut text_start = 0;
    let mut pos = 0;

    while pos < bytes.len() {
      if bytes[pos] != b'<' {
        pos += 1;
        continue;
      }
      let Some(markup) = self.markup_at(pos)? else {
        // a `<` that doesn't open a tag is text like any other character
        pos += 1;
        continue;
      };
      let markup_start = pos;
      if text_start < pos {
        push_node(&mut stack, &mut root, Node::Text(&self.text[text_start..pos]));
      }
      let markup_text = &self.text[markup_start..markup.end];
      pos = markup.end;

      match markup.kind {
        MarkupKind::Verbatim(kind) => {
          let node = match kind {
            VerbatimKind::Comment => Node::Comment(markup_text),
            VerbatimKind::Doctype => Node::Doctype(markup_text),
            VerbatimKind::ProcessingInstruction => Node::ProcessingInstruction(markup_text),
            VerbatimKind::CData => Node::CData(markup_text),
          };
          push_node(&mut stack, &mut root, node);
        }
        MarkupKind::CloseTag { name } => {
          let Some(frame) = stack.pop() else {
            return Err(ParseError::UnexpectedClosingTag { name });
          };
          if !frame.name.eq_ignore_ascii_case(name) {
            return Err(ParseError::MismatchedClosingTag {
              expected: frame.name,
              found: name,
            });
          }
          push_node(
            &mut stack,
            &mut root,
            Node::Element(Element {
              name: frame.name,
              attributes: frame.attributes,
              children: frame.children,
              content: &self.text[frame.content_start..markup_start],
              source: &self.text[frame.start..pos],
              kind: ElementKind::Normal,
              self_closing_syntax: false,
            }),
          );
        }
        MarkupKind::OpenTag {
          name,
          attributes,
          self_closing,
        } => {
          let foreign = stack.last().is_some_and(|frame| frame.foreign) || tags::is_foreign_root(name);
          if tags::is_void(name) {
            push_node(
              &mut stack,
              &mut root,
              Node::Element(Element {
                name,
                attributes,
                children: Vec::new(),
                content: &self.text[pos..pos],
                source: &self.text[markup_start..pos],
                kind: ElementKind::Void,
                self_closing_syntax: self_closing,
              }),
            );
          } else if tags::is_raw_text(name) {
            let (content, end) = self.raw_text_content(name, pos)?;
            pos = end;
            push_node(
              &mut stack,
              &mut root,
              Node::Element(Element {
                name,
                attributes,
                children: if content.is_empty() {
                  Vec::new()
                } else {
                  vec![Node::Text(content)]
                },
                content,
                source: &self.text[markup_start..pos],
                kind: ElementKind::RawText,
                self_closing_syntax: false,
              }),
            );
          } else if self_closing {
            // a browser reads `<div/>` as opening a div rather than as an empty
            // one, so a self-closing tag is only read as one where it really
            // does close the element
            if !foreign {
              return Err(ParseError::MalformedTag);
            }
            push_node(
              &mut stack,
              &mut root,
              Node::Element(Element {
                name,
                attributes,
                children: Vec::new(),
                content: &self.text[pos..pos],
                source: &self.text[markup_start..pos],
                kind: ElementKind::SelfClosing,
                self_closing_syntax: true,
              }),
            );
          } else {
            stack.push(Frame {
              name,
              attributes,
              children: Vec::new(),
              start: markup_start,
              content_start: pos,
              foreign,
            });
          }
        }
      }
      text_start = pos;
    }

    if text_start < bytes.len() {
      push_node(&mut stack, &mut root, Node::Text(&self.text[text_start..]));
    }
    match stack.pop() {
      Some(frame) => Err(ParseError::UnclosedElement { name: frame.name }),
      None => Ok(Document { children: root }),
    }
  }

  /// Reads the markup at a `<`, or `None` where it's only text.
  fn markup_at(&self, start: usize) -> Result<Option<Markup<'a>>, ParseError<'a>> {
    let bytes = self.text.as_bytes();
    match bytes.get(start + 1) {
      Some(b'!') => match bytes.get(start + 2) {
        Some(b'-') if self.text[start..].starts_with("<!--") => {
          let end = self.comment_end(start + 4).ok_or(ParseError::UnterminatedMarkup)?;
          Ok(Some(Markup {
            kind: MarkupKind::Verbatim(VerbatimKind::Comment),
            end,
          }))
        }
        Some(b'[') if self.text[start..].starts_with("<![CDATA[") => {
          let end = self
            .find_after(start + 9, "]]>")
            .ok_or(ParseError::UnterminatedMarkup)?;
          Ok(Some(Markup {
            kind: MarkupKind::Verbatim(VerbatimKind::CData),
            end,
          }))
        }
        // a declaration is `<!` followed by an ascii letter
        Some(byte) if byte.is_ascii_alphabetic() => {
          let end = self.find_after(start + 2, ">").ok_or(ParseError::UnterminatedMarkup)?;
          Ok(Some(Markup {
            kind: MarkupKind::Verbatim(VerbatimKind::Doctype),
            end,
          }))
        }
        _ => Ok(None),
      },
      Some(b'?') => {
        let end = self.find_after(start + 2, "?>").ok_or(ParseError::UnterminatedMarkup)?;
        Ok(Some(Markup {
          kind: MarkupKind::Verbatim(VerbatimKind::ProcessingInstruction),
          end,
        }))
      }
      Some(b'/') => Ok(self.close_tag_at(start)),
      Some(byte) if byte.is_ascii_alphabetic() => self.open_tag_at(start),
      _ => Ok(None),
    }
  }

  fn close_tag_at(&self, start: usize) -> Option<Markup<'a>> {
    let bytes = self.text.as_bytes();
    let name_start = start + 2;
    let mut index = name_start;
    while bytes.get(index).is_some_and(|b| is_tag_name_byte(*b)) {
      index += 1;
    }
    if index == name_start {
      return None;
    }
    let name = &self.text[name_start..index];
    index = skip_whitespace(bytes, index);
    if bytes.get(index) != Some(&b'>') {
      return None;
    }
    Some(Markup {
      kind: MarkupKind::CloseTag { name },
      end: index + 1,
    })
  }

  /// Reads an open tag, or `None` where what looks like one isn't written as a
  /// tag at all and so is only text.
  fn open_tag_at(&self, start: usize) -> Result<Option<Markup<'a>>, ParseError<'a>> {
    let bytes = self.text.as_bytes();
    let name_start = start + 1;
    let mut index = name_start;
    while bytes.get(index).is_some_and(|b| is_tag_name_byte(*b)) {
      index += 1;
    }
    let name = &self.text[name_start..index];
    let mut attributes = Vec::new();

    loop {
      let after_whitespace = skip_whitespace(bytes, index);
      match bytes.get(after_whitespace) {
        Some(b'>') => {
          return Ok(Some(Markup {
            kind: MarkupKind::OpenTag {
              name,
              attributes,
              self_closing: false,
            },
            end: after_whitespace + 1,
          }))
        }
        Some(b'/') if bytes.get(after_whitespace + 1) == Some(&b'>') => {
          return Ok(Some(Markup {
            kind: MarkupKind::OpenTag {
              name,
              attributes,
              self_closing: true,
            },
            end: after_whitespace + 2,
          }))
        }
        // an attribute has to be separated from whatever came before it
        Some(byte) if is_attribute_name_start(*byte) && after_whitespace > index => {
          let (attribute, end) = self.attribute_at(after_whitespace)?;
          attributes.push(attribute);
          index = end;
        }
        _ => return Ok(None),
      }
    }
  }

  fn attribute_at(&self, start: usize) -> Result<(Attribute<'a>, usize), ParseError<'a>> {
    let bytes = self.text.as_bytes();
    let mut index = start;
    while bytes.get(index).is_some_and(|b| is_attribute_name_byte(*b)) {
      index += 1;
    }
    let name = &self.text[start..index];
    let after_name = index;
    index = skip_whitespace(bytes, index);
    if bytes.get(index) != Some(&b'=') {
      return Ok((Attribute { name, value: None }, after_name));
    }
    index = skip_whitespace(bytes, index + 1);

    match bytes.get(index) {
      Some(quote) if *quote == b'"' || *quote == b'\'' => {
        let quote = *quote;
        let value_start = index + 1;
        let mut end = value_start;
        while bytes.get(end).is_some_and(|b| *b != quote) {
          end += 1;
        }
        if bytes.get(end).is_none() {
          return Err(ParseError::UnterminatedMarkup);
        }
        Ok((
          Attribute {
            name,
            value: Some(AttributeValue {
              text: &self.text[value_start..end],
              quote: Some(quote as char),
            }),
          },
          end + 1,
        ))
      }
      Some(_) => {
        let value_start = index;
        while bytes.get(index).is_some_and(|b| is_unquoted_value_byte(*b)) {
          index += 1;
        }
        if index == value_start {
          return Err(ParseError::MalformedTag);
        }
        Ok((
          Attribute {
            name,
            value: Some(AttributeValue {
              text: &self.text[value_start..index],
              quote: None,
            }),
          },
          index,
        ))
      }
      None => Err(ParseError::UnterminatedMarkup),
    }
  }

  /// Reads the character data of a raw text element, which runs to that
  /// element's closing tag no matter what markup is written within it.
  fn raw_text_content(&self, name: &'a str, start: usize) -> Result<(&'a str, usize), ParseError<'a>> {
    let bytes = self.text.as_bytes();
    let mut index = start;
    while index < bytes.len() {
      if bytes[index] != b'<' || bytes.get(index + 1) != Some(&b'/') {
        index += 1;
        continue;
      }
      let name_start = index + 2;
      let name_end = name_start + name.len();
      if !self
        .text
        .get(name_start..name_end)
        .is_some_and(|found| found.eq_ignore_ascii_case(name))
      {
        index += 1;
        continue;
      }
      let after_name = skip_whitespace(bytes, name_end);
      if bytes.get(after_name) != Some(&b'>') {
        index += 1;
        continue;
      }
      return Ok((&self.text[start..index], after_name + 1));
    }
    Err(ParseError::UnclosedElement { name })
  }

  /// Where a comment whose text starts at the position ends. As in a browser,
  /// `<!-->` and `<!--->` are comments closed abruptly, and `--!>` closes a
  /// comment as `-->` does.
  fn comment_end(&self, start: usize) -> Option<usize> {
    let rest = &self.text[start..];
    if rest.starts_with('>') {
      return Some(start + 1);
    }
    if rest.starts_with("->") {
      return Some(start + 2);
    }
    let closed = self.find_after(start, "-->");
    let closed_abruptly = self.find_after(start, "--!>");
    match (closed, closed_abruptly) {
      (Some(a), Some(b)) => Some(a.min(b)),
      (a, b) => a.or(b),
    }
  }

  fn find_after(&self, start: usize, close: &str) -> Option<usize> {
    self.text[start..].find(close).map(|index| start + index + close.len())
  }
}

fn push_node<'a>(stack: &mut [Frame<'a>], root: &mut Vec<Node<'a>>, node: Node<'a>) {
  match stack.last_mut() {
    Some(frame) => frame.children.push(node),
    None => root.push(node),
  }
}

fn skip_whitespace(bytes: &[u8], start: usize) -> usize {
  let mut index = start;
  while matches!(bytes.get(index), Some(b' ' | b'\t' | b'\n' | b'\r')) {
    index += 1;
  }
  index
}

fn is_tag_name_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || byte == b'-'
}

fn is_attribute_name_start(byte: u8) -> bool {
  byte.is_ascii_alphabetic() || matches!(byte, b'_' | b':' | b'@' | b'#' | b'[' | b'(' | b'.')
}

fn is_attribute_name_byte(byte: u8) -> bool {
  !matches!(
    byte,
    b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'\'' | b'=' | b'<' | b'>' | b'/' | b'`'
  )
}

fn is_unquoted_value_byte(byte: u8) -> bool {
  !matches!(
    byte,
    b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'\'' | b'=' | b'<' | b'>' | b'`'
  )
}

#[cfg(test)]
mod test {
  use super::*;

  /// Renders the parsed html compactly so a test can say what it expects on
  /// one line.
  fn describe(text: &str) -> String {
    match parse(text) {
      Ok(document) => document.children.iter().map(describe_node).collect(),
      Err(err) => format!("error({})", err),
    }
  }

  fn describe_node(node: &Node) -> String {
    match node {
      Node::Text(text) => format!("text({:?})", text),
      Node::Comment(text) => format!("comment({:?})", text),
      Node::Doctype(text) => format!("doctype({:?})", text),
      Node::ProcessingInstruction(text) => format!("pi({:?})", text),
      Node::CData(text) => format!("cdata({:?})", text),
      Node::Element(element) => {
        let attributes: String = element
          .attributes
          .iter()
          .map(|attribute| match &attribute.value {
            Some(value) => format!(" {}={:?}", attribute.name, value.text),
            None => format!(" {}", attribute.name),
          })
          .collect();
        let kind = match element.kind {
          ElementKind::Normal => "",
          ElementKind::Void => "/void",
          ElementKind::SelfClosing => "/self",
          ElementKind::RawText => "/raw",
        };
        let children: String = element.children.iter().map(describe_node).collect();
        format!("<{}{}{}>[{}]", element.name, attributes, kind, children)
      }
    }
  }

  // ==== structure ====

  #[test]
  fn parses_an_element_and_its_text() {
    assert_eq!(describe("<p>a</p>"), "<p>[text(\"a\")]");
  }

  #[test]
  fn parses_nested_elements() {
    assert_eq!(describe("<div><p>a</p></div>"), "<div>[<p>[text(\"a\")]]");
  }

  #[test]
  fn parses_siblings_and_the_text_between_them() {
    assert_eq!(
      describe("<p>a</p> <p>b</p>"),
      "<p>[text(\"a\")]text(\" \")<p>[text(\"b\")]"
    );
  }

  #[test]
  fn parses_an_empty_element() {
    assert_eq!(describe("<div></div>"), "<div>[]");
  }

  #[test]
  fn matches_a_closing_tag_without_case() {
    assert_eq!(describe("<DIV>a</div>"), "<DIV>[text(\"a\")]");
    assert_eq!(describe("<div>a</DIV>"), "<div>[text(\"a\")]");
  }

  #[test]
  fn keeps_the_content_of_an_element_as_it_was_written() {
    let document = parse("<div>  a  <b>c</b>  </div>").unwrap();
    let Node::Element(element) = &document.children[0] else {
      panic!("expected an element");
    };
    assert_eq!(element.content, "  a  <b>c</b>  ");
  }

  // ==== attributes ====

  #[test]
  fn parses_a_quoted_attribute_value() {
    assert_eq!(describe("<a href=\"foo\">x</a>"), "<a href=\"foo\">[text(\"x\")]");
    assert_eq!(describe("<a href='foo'>x</a>"), "<a href=\"foo\">[text(\"x\")]");
  }

  #[test]
  fn parses_an_unquoted_attribute_value() {
    assert_eq!(describe("<a href=foo>x</a>"), "<a href=\"foo\">[text(\"x\")]");
  }

  #[test]
  fn parses_an_attribute_with_no_value() {
    assert_eq!(describe("<input disabled>"), "<input disabled/void>[]");
  }

  #[test]
  fn parses_several_attributes() {
    assert_eq!(
      describe("<div class=\"a\" id=\"b\" hidden>x</div>"),
      "<div class=\"a\" id=\"b\" hidden>[text(\"x\")]"
    );
  }

  #[test]
  fn parses_an_attribute_value_holding_the_other_quote() {
    assert_eq!(
      describe("<a title='say \"hi\"'>x</a>"),
      "<a title=\"say \\\"hi\\\"\">[text(\"x\")]"
    );
  }

  #[test]
  fn parses_an_attribute_written_across_lines() {
    assert_eq!(
      describe("<div\n  class=\"a\"\n  id=\"b\"\n>x</div>"),
      "<div class=\"a\" id=\"b\">[text(\"x\")]"
    );
  }

  #[test]
  fn parses_a_framework_attribute_name() {
    assert_eq!(
      describe("<div @click=\"a\" :value=\"b\">x</div>"),
      "<div @click=\"a\" :value=\"b\">[text(\"x\")]"
    );
  }

  // ==== the kinds of element ====

  #[test]
  fn parses_a_void_element() {
    assert_eq!(describe("<br>"), "<br/void>[]");
    assert_eq!(describe("<br />"), "<br/void>[]");
    assert_eq!(describe("<img src=\"a\">"), "<img src=\"a\"/void>[]");
  }

  #[test]
  fn parses_a_self_closing_tag_only_within_foreign_content() {
    assert_eq!(describe("<svg><circle /></svg>"), "<svg>[<circle/self>[]]");
    assert_eq!(
      describe("<div/>"),
      "error(a tag is written in a way the parser doesn't read)"
    );
  }

  #[test]
  fn parses_a_raw_text_element() {
    assert_eq!(describe("<script>a < b</script>"), "<script/raw>[text(\"a < b\")]");
    assert_eq!(describe("<style>.a {}</style>"), "<style/raw>[text(\".a {}\")]");
  }

  #[test]
  fn reads_a_tag_within_a_raw_text_element_as_text() {
    assert_eq!(
      describe("<script>const a = \"</div>\"</script>"),
      "<script/raw>[text(\"const a = \\\"</div>\\\"\")]"
    );
  }

  #[test]
  fn parses_an_empty_raw_text_element() {
    assert_eq!(describe("<script></script>"), "<script/raw>[]");
  }

  // ==== markup that is kept as it was written ====

  #[test]
  fn parses_a_comment() {
    assert_eq!(describe("<!-- a -->"), "comment(\"<!-- a -->\")");
    assert_eq!(describe("<!--\na\n-->"), "comment(\"<!--\\na\\n-->\")");
  }

  #[test]
  fn parses_a_doctype_declaration() {
    assert_eq!(describe("<!DOCTYPE html>"), "doctype(\"<!DOCTYPE html>\")");
  }

  #[test]
  fn parses_a_processing_instruction() {
    assert_eq!(describe("<?php echo 1; ?>"), "pi(\"<?php echo 1; ?>\")");
  }

  #[test]
  fn parses_a_cdata_section() {
    assert_eq!(describe("<![CDATA[ a < b ]]>"), "cdata(\"<![CDATA[ a < b ]]>\")");
  }

  // ==== text that only looks like markup ====

  #[test]
  fn reads_an_angle_bracket_that_opens_no_tag_as_text() {
    assert_eq!(describe("a < b"), "text(\"a < b\")");
    assert_eq!(describe("1 <2 3"), "text(\"1 <2 3\")");
    assert_eq!(describe("a <> b"), "text(\"a <> b\")");
  }

  #[test]
  fn reads_a_malformed_tag_as_text() {
    assert_eq!(describe("<div =foo>"), "text(\"<div =foo>\")");
  }

  // ==== html the parser refuses ====

  #[test]
  fn refuses_an_element_that_was_never_closed() {
    assert_eq!(describe("<div>"), "error(`<div>` was never closed)");
    assert_eq!(describe("<div><p>a</p>"), "error(`<div>` was never closed)");
  }

  #[test]
  fn refuses_a_closing_tag_that_closes_nothing() {
    assert_eq!(describe("</div>"), "error(`</div>` closes an element that isn't open)");
  }

  #[test]
  fn refuses_tags_that_do_not_match() {
    assert_eq!(
      describe("<div><p>a</div></p>"),
      "error(`</div>` was found where `</p>` was expected)"
    );
  }

  #[test]
  fn refuses_html_that_leaves_a_closing_tag_to_be_inferred() {
    assert_eq!(
      describe("<ul><li>a<li>b</ul>"),
      "error(`</ul>` was found where `</li>` was expected)"
    );
  }

  #[test]
  fn refuses_markup_that_was_never_terminated() {
    assert_eq!(
      describe("<!-- a"),
      "error(markup runs past the end of the text without being closed)"
    );
    assert_eq!(
      describe("<![CDATA[ a"),
      "error(markup runs past the end of the text without being closed)"
    );
    assert_eq!(
      describe("<div title=\"a"),
      "error(markup runs past the end of the text without being closed)"
    );
  }

  #[test]
  fn refuses_a_raw_text_element_that_was_never_closed() {
    assert_eq!(describe("<script>a"), "error(`<script>` was never closed)");
  }
}
