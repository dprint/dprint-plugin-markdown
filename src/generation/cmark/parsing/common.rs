use crate::generation::common::*;

pub fn parse_text_in_brackets(start_pos: usize, char_scanner: &mut CharScanner) -> Result<String, ParseError> {
  parse_text_in_container(start_pos, char_scanner, '[', ']')
}

/// Parses the text up to the parenthesis that closes an inline image.
///
/// A link destination may contain parentheses, so within one they only close
/// the image when unbalanced, and not at all when it's in pointy brackets.
/// Past the destination a title could contain anything, so stay conservative
/// and bail out there like this used to everywhere.
pub fn parse_text_in_parens(start_pos: usize, char_scanner: &mut CharScanner) -> Result<String, ParseError> {
  let mut text = String::new();
  let mut destination = Destination::NotStarted;
  let mut parens_depth = 0;

  while let Some((byte_pos, c)) = char_scanner.next() {
    if c == '\\' {
      text.push(c);
      // push next char without close/open check, since it's escaped
      if let Some((_, next_c)) = char_scanner.next() {
        text.push(next_c);
      }
      if matches!(destination, Destination::NotStarted) {
        destination = Destination::Bare;
      }
      continue;
    }

    match (&destination, c) {
      (Destination::NotStarted, ')') => return Ok(text),
      (Destination::NotStarted, '<') => destination = Destination::PointyBrackets,
      (Destination::NotStarted, '(') => {
        destination = Destination::Bare;
        parens_depth += 1;
      }
      (Destination::NotStarted, c) if !c.is_whitespace() => destination = Destination::Bare,
      (Destination::PointyBrackets, '>') => destination = Destination::Ended,
      (Destination::Bare, c) if c.is_whitespace() => destination = Destination::Ended,
      (Destination::Bare, '(') => parens_depth += 1,
      (Destination::Bare | Destination::Ended, ')') => {
        if parens_depth == 0 {
          return Ok(text);
        }
        parens_depth -= 1;
      }
      (Destination::Ended, '(') => {
        return Err(ParseError::new(
          Range {
            start: byte_pos,
            end: byte_pos + c.len_utf8(),
          },
          "Unexpected open container char `(`.",
        ))
      }
      _ => (),
    }
    text.push(c);
  }

  Err(ParseError::new(
    Range {
      start: start_pos,
      end: char_scanner.pos(),
    },
    "Did not find container close char `)`.",
  ))
}

/// Where within an inline image's parentheses the destination is.
enum Destination {
  /// Only whitespace has been seen so far.
  NotStarted,
  /// Within a destination enclosed in pointy brackets.
  PointyBrackets,
  /// Within a destination not enclosed in pointy brackets.
  Bare,
  /// Past the destination, where a title may follow.
  Ended,
}

fn parse_text_in_container(
  start_pos: usize,
  char_scanner: &mut CharScanner,
  open_char: char,
  close_char: char,
) -> Result<String, ParseError> {
  let mut text = String::new();
  while let Some((byte_pos, c)) = char_scanner.next() {
    if c == close_char {
      return Ok(text);
    } else if c == '\\' {
      text.push(c);
      // push next char without close/open check, since it's escaped
      if let Some((_, next_c)) = char_scanner.next() {
        text.push(next_c);
      }
    } else if c == open_char {
      return Err(ParseError::new(
        Range {
          start: byte_pos,
          end: byte_pos + c.len_utf8(),
        },
        format!("Unexpected open container char `{}`.", open_char),
      ));
    } else {
      text.push(c);
    }
  }

  Err(ParseError::new(
    Range {
      start: start_pos,
      end: char_scanner.pos(),
    },
    format!("Did not find container close char `{}`.", close_char),
  ))
}

pub fn parse_link_url_and_title(text: &str) -> (String, Option<String>) {
  let mut char_scanner = CharScanner::new(0, text);
  let mut url = String::new();
  let mut title: Option<String> = None;

  char_scanner.skip_spaces();

  while let Some((_, c)) = char_scanner.next() {
    match c {
      '"' => match try_parse_title(&mut char_scanner) {
        Ok(text) => title = Some(text),
        Err(text) => {
          url.push('"');
          url.push_str(&text);
        }
      },
      _ => url.push(c),
    }
  }

  (url, title)
}

fn try_parse_title(char_scanner: &mut CharScanner) -> Result<String, String> {
  let mut text = String::new();

  while let Some((_, c)) = char_scanner.next() {
    match c {
      '"' => {
        if char_scanner.peek().is_some() {
          text.push('"');
          return Err(text);
        }
        return Ok(text);
      }
      _ => text.push(c),
    }
  }

  Err(text)
}
