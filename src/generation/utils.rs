use crate::parser::SPACES;
use std::borrow::Cow;

use regex::Regex;

/// Checks if the provided word would start a new block when it appears at the
/// start of a line (ex. a list item, block quote, heading, thematic break, or
/// code fence). Such a word can't be moved to the start of a line by wrapping
/// without changing the meaning of the document.
/// Assumes the provided string is one word and doesn't have whitespace.
pub fn is_block_start_word(word: &str) -> bool {
  if is_list_word(word) {
    return true;
  }

  // block quote (ex. >, >>, >text)
  if word.starts_with('>') {
    return true;
  }

  // definition list marker, which is a `:` followed by whitespace
  if word == ":" {
    return true;
  }

  // atx heading (ex. #, ###) -- only when nothing follows the hashes,
  // since #text isn't a heading
  let hash_count = word.chars().take_while(|c| *c == '#').count();
  if hash_count > 0 && hash_count <= 6 && word.len() == hash_count {
    return true;
  }

  // setext heading underline (ex. =, ===), which would turn the previous
  // line into a heading
  if !word.is_empty() && word.chars().all(|c| c == '=') {
    return true;
  }

  // thematic break or code fence (ex. ---, ***, ___, ~~~ and three backticks)
  match word.chars().next() {
    Some(c) if matches!(c, '-' | '*' | '_' | '`' | '~') => {
      word.chars().count() >= 3 && word.chars().all(|other| other == c)
    }
    _ => false,
  }
}

/// Checks if the provided word is a word that could be a list.
/// Assumes the provided string is one word, which is to say it has no space
/// or line ending in it. Other whitespace (ex. a tab) may show up within a
/// word because only those two break one.
pub fn is_list_word(word: &str) -> bool {
  debug_assert!(!word.chars().any(|c| c == ' ' || c == '\n'));

  if word == "*" || word == "-" || word == "+" {
    true
  } else {
    let mut had_number = false;
    let mut had_end_char = false;
    for c in word.chars() {
      if had_end_char {
        return false;
      }

      if !had_number {
        if c.is_numeric() {
          had_number = true;
        } else {
          return false;
        }
      } else if !c.is_numeric() {
        if c == '.' || c == ')' {
          had_end_char = true;
        } else {
          return false;
        }
      }
    }

    had_end_char
  }
}

/// Whether the position at `index` is preceded by a blank line.
///
/// When `in_block_quote` is `true`, the block quote markers (`>`) that prefix an
/// otherwise blank line are skipped while scanning backwards. Without this a blank
/// line inside a block quote (which is written as `>`) would not be recognized as
/// blank because of the leading `>` character.
pub fn has_leading_blankline(index: usize, text: &str, in_block_quote: bool) -> bool {
  let mut newline_count = 0;
  // whether the character to the right of this one was a newline, which makes
  // a carriage return here the start of the line ending it already counted
  let mut after_newline = false;
  for c in text[0..index].chars().rev() {
    let ends_line = c == '\n' || (c == '\r' && !after_newline);
    after_newline = c == '\n';
    if ends_line {
      newline_count += 1;
      if newline_count >= 2 {
        return true;
      }
    } else if c == '\r' {
      continue;
    } else if in_block_quote && c == '>' {
      // a blank line inside a block quote is written as `>`, so skip the block
      // quote markers while looking for consecutive newlines
      continue;
    } else if !SPACES.contains(&c) {
      break;
    }
  }
  false
}

pub fn file_has_ignore_file_directive(file_text: &str, directive_inner_text: &str) -> bool {
  let ignore_regex = get_ignore_comment_regex(directive_inner_text);
  ignore_regex.is_match(file_text)
}

pub fn get_ignore_comment_regex(inner_text: &str) -> Regex {
  // todo: don't use regex
  let text = format!(r"^\s*<!\-\-\s*{}\s*\-\->\s*", inner_text);
  Regex::new(&text).unwrap()
}

pub fn get_leading_non_space_tab_byte_pos(text: &str, pos: usize) -> usize {
  let text_bytes = text.as_bytes();
  for i in (0..pos).rev() {
    let current_char = text_bytes.get(i);
    if current_char != Some(&(b' ')) && current_char != Some(&(b'\t')) {
      return i + 1;
    }
  }

  0
}

pub fn unindent(text: &str) -> Cow<'_, str> {
  let lines = text.split('\n').collect::<Vec<_>>();
  let mut lines_with_indent = Vec::with_capacity(lines.len());
  for line in lines.into_iter() {
    // a character that only looks like a space, such as a non-breaking one, is
    // text of the code rather than the indentation written before it
    let line_indent = line.chars().take_while(|c| SPACES.contains(c)).count();
    if line_indent == 0 {
      return Cow::Borrowed(text);
    }
    lines_with_indent.push((line, line_indent));
  }
  let min_indent = lines_with_indent.iter().map(|(_, indent)| indent).min().copied();
  if let Some(min_indent) = min_indent {
    Cow::Owned(
      lines_with_indent
        .into_iter()
        .map(|(l, indent)| {
          if indent >= min_indent {
            let mut chars = l.chars();
            for _ in 0..min_indent {
              chars.next();
            }
            chars.as_str()
          } else {
            l
          }
        })
        .collect::<Vec<_>>()
        .join("\n"),
    )
  } else {
    Cow::Borrowed(text)
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn it_should_find_list_words() {
    assert_eq!(is_list_word("test"), false);
    assert_eq!(is_list_word("*"), true);
    assert_eq!(is_list_word("+"), true);
    assert_eq!(is_list_word("-"), true);
    assert_eq!(is_list_word("1."), true);
    assert_eq!(is_list_word("99."), true);
    assert_eq!(is_list_word("10)"), true);
    assert_eq!(is_list_word("9999)"), true);
    assert_eq!(is_list_word("9999)."), false);
  }

  #[test]
  fn it_should_find_block_start_words() {
    assert_eq!(is_block_start_word("test"), false);
    assert_eq!(is_block_start_word("-"), true);
    assert_eq!(is_block_start_word("1."), true);
    assert_eq!(is_block_start_word(">"), true);
    assert_eq!(is_block_start_word(">text"), true);
    assert_eq!(is_block_start_word("#"), true);
    assert_eq!(is_block_start_word("######"), true);
    assert_eq!(is_block_start_word("#######"), false);
    assert_eq!(is_block_start_word("#text"), false);
    assert_eq!(is_block_start_word("="), true);
    assert_eq!(is_block_start_word("==="), true);
    assert_eq!(is_block_start_word("=text"), false);
    assert_eq!(is_block_start_word("---"), true);
    assert_eq!(is_block_start_word("***"), true);
    assert_eq!(is_block_start_word("___"), true);
    assert_eq!(is_block_start_word("~~~"), true);
    assert_eq!(is_block_start_word("```"), true);
    assert_eq!(is_block_start_word("--"), false);
    assert_eq!(is_block_start_word("~~"), false);
    assert_eq!(is_block_start_word("---a"), false);
  }

  #[test]
  fn should_unindent() {
    assert_eq!(unindent("  1\r\n  2"), "1\r\n2");
    assert_eq!(unindent("  1\n 2"), " 1\n2");
    assert_eq!(unindent(" 1\n  2"), "1\n 2");
    assert_eq!(unindent("1\n2"), "1\n2");
    // a character that only looks like a space is text of the code rather than
    // the indentation written before it
    assert_eq!(unindent("\u{3000}1\n\u{3000}\u{3000}2"), "\u{3000}1\n\u{3000}\u{3000}2");
    assert_eq!(unindent("  \u{3000}1\n  \u{3000}2"), "\u{3000}1\n\u{3000}2");
  }
}
