use super::parse_link_url_and_title;
use super::parse_text_in_brackets;
use crate::generation::common::CharScanner;
use crate::generation::common::*;

/// Crudely parses out link reference definitions from the provided text.
pub fn parse_link_reference_definitions(offset: usize, text: &str) -> Result<Vec<LinkReference>, ParseError> {
  let mut char_scanner = CharScanner::new(offset, text);
  let mut references = Vec::new();

  while let Some((byte_pos, c)) = char_scanner.next() {
    // Could encounter a `>` at the start of a line when checking
    // markdown nodes within a block quote, so skip over any `>` chars.
    if c.is_whitespace() || c == '>' {
      continue;
    } else if c == '[' {
      let link_ref_definition = parse_link_reference_definition(byte_pos, &mut char_scanner)?;
      references.push(link_ref_definition);
    } else {
      return Err(ParseError::new(
        Range {
          start: byte_pos,
          end: byte_pos + c.len_utf8(),
        },
        format!("Unexpected token `{}` while parsing link reference definition.", c),
      ));
    }
  }

  Ok(references)
}

fn parse_link_reference_definition(
  start_pos: usize,
  char_scanner: &mut CharScanner,
) -> Result<LinkReference, ParseError> {
  let name = parse_text_in_brackets(start_pos, char_scanner)?;
  char_scanner.assert_char(':')?;
  char_scanner.skip_spaces();
  // the destination may be on the line after the label (ex. `[a]:\n/url`)
  if matches!(char_scanner.peek(), Some((_, '\n'))) {
    char_scanner.next();
    skip_line_start(char_scanner);
  }
  let final_text = parse_reference_link(start_pos, char_scanner)?;
  let (url, title) = parse_link_url_and_title(final_text.trim());

  Ok(LinkReference {
    range: Range {
      start: start_pos,
      end: char_scanner.pos(), // pos because it's at the next character
    },
    name,
    link: url,
    title,
  })
}

/// Skips over the whitespace and any block quote markers a line starts with.
fn skip_line_start(char_scanner: &mut CharScanner) {
  char_scanner.skip_spaces();
  while matches!(char_scanner.peek(), Some((_, '>'))) {
    char_scanner.next();
    char_scanner.skip_spaces();
  }
}

fn parse_reference_link(start_pos: usize, char_scanner: &mut CharScanner) -> Result<String, ParseError> {
  let mut reference_link = String::new();
  let mut is_in_title = false;
  let mut had_title = false;
  while let Some((_, c)) = char_scanner.next() {
    match c {
      // a title may span lines, so only the line ending that follows the
      // definition ends it
      '\n' if !is_in_title => {
        // a title may also start on the line after the destination
        if had_title || reference_link.trim().is_empty() {
          break;
        }
        skip_line_start(char_scanner);
        if !matches!(char_scanner.peek(), Some((_, '"'))) {
          break;
        }
        while reference_link.ends_with(|c: char| c.is_whitespace()) {
          reference_link.pop();
        }
        reference_link.push(' ');
      }
      '\\' if is_in_title => {
        reference_link.push(c);
        // push the next char without checking it, since it's escaped
        if let Some((_, next_c)) = char_scanner.next() {
          reference_link.push(next_c);
        }
      }
      '"' => {
        // a title opens after the whitespace that ends the destination, so a
        // quote anywhere else is part of the destination (ex. `[a]: a"b`)
        if is_in_title {
          is_in_title = false;
          had_title = true;
        } else if reference_link.chars().next_back().is_none_or(|c| c.is_whitespace()) {
          is_in_title = true;
        }
        reference_link.push(c);
      }
      _ => reference_link.push(c),
    }
  }

  if reference_link.is_empty() {
    return Err(ParseError::new(
      Range {
        start: start_pos,
        end: char_scanner.pos(), // pos because it's at the next character
      },
      "Unexpected empty link parsing link reference definition link.",
    ));
  }

  Ok(reference_link)
}

#[cfg(test)]
mod tests {
  use super::parse_link_reference_definitions;

  #[test]
  fn it_parses_empty_string() {
    let result = parse_link_reference_definitions(10, "");
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.ok().unwrap().is_empty(), true);
  }

  #[test]
  fn it_finds_link_reference() {
    let result = parse_link_reference_definitions(10, "[Some reference]: https://dprint.dev");
    assert_eq!(result.is_ok(), true);
    let references = result.ok().unwrap();
    assert_eq!(references.len(), 1);
    let reference = &references[0];
    assert_eq!(reference.range.start, 10);
    assert_eq!(reference.range.end, 46);
    assert_eq!(reference.name, "Some reference");
    assert_eq!(reference.link, "https://dprint.dev");
  }

  #[test]
  fn it_finds_link_reference_with_new_line_after() {
    let result = parse_link_reference_definitions(10, "[Some reference]: https://dprint.dev\n");
    assert_eq!(result.is_ok(), true);
    let references = result.ok().unwrap();
    assert_eq!(references.len(), 1);
    let reference = &references[0];
    assert_eq!(reference.range.start, 10);
    assert_eq!(reference.range.end, 46);
    assert_eq!(reference.name, "Some reference");
    assert_eq!(reference.link, "https://dprint.dev");
  }

  #[test]
  fn it_finds_multiple_link_references() {
    let result = parse_link_reference_definitions(
      10,
      "[Some reference]: https://dprint.dev\n\n[other]: https://github.com",
    );
    assert_eq!(result.is_ok(), true);
    let references = result.ok().unwrap();
    assert_eq!(references.len(), 2);
    let reference = &references[0];
    assert_eq!(reference.range.start, 10);
    assert_eq!(reference.range.end, 46);
    assert_eq!(reference.name, "Some reference");
    assert_eq!(reference.link, "https://dprint.dev");
    let reference = &references[1];
    assert_eq!(reference.range.start, 48);
    assert_eq!(reference.range.end, 75);
    assert_eq!(reference.name, "other");
    assert_eq!(reference.link, "https://github.com");
  }

  #[test]
  fn it_finds_link_reference_with_title_on_next_line() {
    let result = parse_link_reference_definitions(0, "[reference]: https://dprint.dev\n\"Some title\"");
    let references = result.ok().unwrap();
    assert_eq!(references.len(), 1);
    let reference = &references[0];
    assert_eq!(reference.name, "reference");
    // the trailing space is trimmed when generating
    assert_eq!(reference.link.trim(), "https://dprint.dev");
    assert_eq!(reference.title, Some("Some title".to_string()));
  }

  #[test]
  fn it_finds_link_reference_with_link_on_next_line() {
    let result = parse_link_reference_definitions(0, "[reference]:\nhttps://dprint.dev");
    let references = result.ok().unwrap();
    assert_eq!(references.len(), 1);
    let reference = &references[0];
    assert_eq!(reference.name, "reference");
    assert_eq!(reference.link, "https://dprint.dev");
    assert_eq!(reference.title, None);
  }

  #[test]
  fn it_does_not_treat_the_next_definition_as_a_title() {
    let result = parse_link_reference_definitions(0, "[a]: https://dprint.dev\n[b]: https://github.com");
    let references = result.ok().unwrap();
    assert_eq!(references.len(), 2);
    assert_eq!(references[0].link, "https://dprint.dev");
    assert_eq!(references[0].title, None);
    assert_eq!(references[1].link, "https://github.com");
    assert_eq!(references[1].title, None);
  }

  #[test]
  fn it_finds_link_reference_with_title_on_next_line_in_block_quote() {
    let result = parse_link_reference_definitions(0, "> [reference]: https://dprint.dev\n> \"Some title\"");
    let references = result.ok().unwrap();
    assert_eq!(references.len(), 1);
    let reference = &references[0];
    assert_eq!(reference.name, "reference");
    // the trailing space is trimmed when generating
    assert_eq!(reference.link.trim(), "https://dprint.dev");
    assert_eq!(reference.title, Some("Some title".to_string()));
  }
}
