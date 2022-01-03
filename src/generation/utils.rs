use std::borrow::Cow;

use regex::Regex;

/// Checks if the provided word is a word that could be a list.
/// Assumes the provided string is one word and doesn't have whitespace.
pub fn is_list_word(word: &str) -> bool {
  debug_assert!(!word.chars().any(|c| c.is_whitespace()));

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

pub fn has_leading_blankline(index: usize, text: &str) -> bool {
  let mut newline_count = 0;
  for c in text[0..index].chars().rev() {
    if c == '\n' {
      newline_count += 1;
      if newline_count >= 2 {
        return true;
      }
    } else if !c.is_whitespace() {
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

pub fn unindent(text: &str) -> Cow<str> {
  let lines = text.split('\n').collect::<Vec<_>>();
  let mut lines_with_indent = Vec::with_capacity(lines.len());
  for line in lines.into_iter() {
    let line_indent = line.chars().take_while(|c| c.is_whitespace()).count();
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
        .map(|(l, indent)| if indent >= min_indent { &l[min_indent..] } else { l })
        .collect::<Vec<_>>()
        .join("\n"),
    )
  } else {
    Cow::Borrowed(text)
  }
}

pub fn trim_spaces_and_newlines(text: &str) -> &str {
  text.trim_matches(is_space_tab_or_newline)
}

pub fn trim_start_spaces_and_newlines(text: &str) -> &str {
  text.trim_start_matches(is_space_tab_or_newline)
}

fn is_space_tab_or_newline(c: char) -> bool {
  matches!(c, ' ' | '\t' | '\r' | '\n')
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
  fn should_unindent() {
    assert_eq!(unindent("  1\r\n  2"), "1\r\n2");
    assert_eq!(unindent("  1\n 2"), " 1\n2");
    assert_eq!(unindent(" 1\n  2"), "1\n 2");
    assert_eq!(unindent("1\n2"), "1\n2");
  }
}
