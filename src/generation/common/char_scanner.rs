use super::ast_nodes::Range;
use super::ParseError;
use std::str::CharIndices;

pub struct CharScanner<'a> {
  char_indices: CharIndices<'a>,
  offset: usize,
  pos: usize,
  previous: Option<(usize, char)>,
  current: Option<(usize, char)>,
  next: Option<(usize, char)>,
  text: &'a str,
}

impl<'a> CharScanner<'a> {
  pub fn new(offset: usize, text: &'a str) -> CharScanner<'a> {
    let char_indices = text.char_indices();
    let mut scanner = CharScanner {
      char_indices,
      offset,
      pos: offset,
      previous: None,
      current: None,
      next: None,
      text,
    };

    scanner.next(); // initialize
    scanner
  }

  pub fn has_next(&self) -> bool {
    self.peek().is_some()
  }

  pub fn is_next_text(&self, text: &str) -> bool {
    let pos = self.end();
    let end = pos + text.len();
    if end > self.text.len() {
      return false;
    }

    &self.text[pos..end] == text
  }

  pub fn move_text(&mut self, text: &str) -> bool {
    for c in text.chars() {
      if let Some((_, next_char)) = self.next() {
        if c != next_char {
          return false;
        }
      } else {
        return false;
      }
    }

    true
  }

  pub fn move_new_line(&mut self) -> bool {
    if let Some((_, current_char)) = self.next() {
      match current_char {
        '\n' => return true,
        '\r' => return matches!(self.next(), Some((_, '\n'))),
        _ => return false,
      }
    }

    false
  }

  pub fn move_next_line(&mut self) {
    while let Some((_, current_char)) = self.next() {
      if current_char == '\n' {
        return;
      }
    }
  }

  pub fn assert_char(&mut self, searching_char: char) -> Result<(), ParseError> {
    while let Some((byte_pos, c)) = self.next() {
      if c == searching_char {
        return Ok(());
      } else if c.is_whitespace() {
        continue;
      } else {
        return Err(ParseError::new(
          Range {
            start: byte_pos,
            end: byte_pos + c.len_utf8(),
          },
          &format!("Unexpected token `{}` when expected `{}`.", c, searching_char),
        ));
      }
    }

    return Err(ParseError::new(
      Range {
        start: self.pos,
        end: self.end(),
      },
      &format!("Did not find expected char of `{}`", searching_char),
    ));
  }

  pub fn skip_spaces(&mut self) {
    while let Some((_, c)) = &self.next {
      if c == &'\n' || !c.is_whitespace() {
        break;
      }

      self.next();
    }
  }

  pub fn next(&mut self) -> Option<(usize, char)> {
    self.previous = self.current;
    self.current = self.next;
    self.next = self.char_indices.next().map(|(byte_pos, c)| (byte_pos + self.offset, c));

    if let Some(current) = self.current {
      self.pos = current.0;
    } else if let Some(previous) = self.previous {
      self.pos += previous.1.len_utf8();
    }

    self.current
  }

  pub fn pos(&self) -> usize {
    self.pos
  }

  pub fn end(&self) -> usize {
    self.current.as_ref().map(|(pos, c)| *pos + c.len_utf8()).unwrap_or(self.pos)
  }

  pub fn peek(&self) -> Option<(usize, char)> {
    self.next
  }
}
