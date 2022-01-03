use super::parse_link_url_and_title;
use super::parse_text_in_brackets;
use super::parse_text_in_parens;
use crate::generation::common::*;
use pulldown_cmark::LinkType;

/// Crudely parses out an image assuming the text is a image.
/// This is done because images have their references inlined by cmark.
pub fn parse_image(offset: usize, text: &str, link_type: LinkType) -> Result<Node, ParseError> {
  let mut char_scanner = CharScanner::new(offset, text);
  let start_pos = char_scanner.pos();

  match link_type {
    LinkType::Inline => parse_inline(start_pos, &mut char_scanner),
    LinkType::Reference | LinkType::ReferenceUnknown | LinkType::Collapsed | LinkType::CollapsedUnknown => parse_reference(start_pos, &mut char_scanner),
    LinkType::Shortcut | LinkType::ShortcutUnknown | LinkType::Email | LinkType::Autolink => Err(ParseError::new(
      Range { start: offset, end: offset },
      &format!("Link type not implemented {:?}", link_type),
    )),
  }
}

fn parse_inline(start_pos: usize, char_scanner: &mut CharScanner) -> Result<Node, ParseError> {
  char_scanner.assert_char('!')?;
  char_scanner.assert_char('[')?;
  let text = parse_text_in_brackets(start_pos, char_scanner)?;
  char_scanner.assert_char('(')?;
  let paren_text = parse_text_in_parens(start_pos, char_scanner)?;
  let (url, title) = parse_link_url_and_title(paren_text.trim());

  Ok(
    InlineImage {
      range: Range {
        start: start_pos,
        end: char_scanner.end(),
      },
      text,
      url,
      title,
    }
    .into(),
  )
}

fn parse_reference(start_pos: usize, char_scanner: &mut CharScanner) -> Result<Node, ParseError> {
  char_scanner.assert_char('!')?;
  char_scanner.assert_char('[')?;
  let text = parse_text_in_brackets(start_pos, char_scanner)?;
  char_scanner.assert_char('[')?;
  let reference = parse_text_in_brackets(start_pos, char_scanner)?;

  Ok(
    ReferenceImage {
      range: Range {
        start: start_pos,
        end: char_scanner.end(),
      },
      text,
      reference,
    }
    .into(),
  )
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn it_should_parse_image() {
    let result = parse_image(10, "![text](url)\n", LinkType::Inline);
    assert_eq!(result.is_ok(), true);
    let image = result.ok().unwrap();
    assert_eq!(image.range().start, 10);
    assert_eq!(image.range().end, 22);
  }
}
