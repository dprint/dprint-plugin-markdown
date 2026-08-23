//! The pieces of markdown syntax that both phases of the parser need to match:
//! link labels, destinations and titles, autolinks and raw html.
//!
//! They all work over the flat [`InlineText`] view of a block's content, which
//! is what the inline phase parses and what the block phase builds in order to
//! look for the link reference definitions at the start of a paragraph.

use std::borrow::Cow;
use std::collections::HashSet;

use super::ast::*;
use super::inline::InlineText;
use super::source::WHITESPACE;

/// What a `]` turned out to close.
pub struct Reference<'a> {
  pub kind: ReferenceKind<'a>,
  /// The position directly after the link.
  pub end: usize,
}

pub enum ReferenceKind<'a> {
  /// The destination is written out (ex. `[text](/url "title")`).
  Inline {
    url: Cow<'a, str>,
    title: Option<Cow<'a, str>>,
  },
  /// The destination comes from a named definition (ex. `[text][name]`).
  Full { label: Cow<'a, str> },
  /// The text is the name (ex. `[name][]`).
  Collapsed,
  /// The text is the name and there are no second brackets (ex. `[name]`).
  Shortcut,
}

/// Matches whatever follows the `]` at `close_start`, where the link's text
/// runs from `content_start` up to it.
pub fn match_reference<'a>(
  text: &InlineText<'a>,
  close_start: usize,
  content_start: usize,
  labels: &HashSet<String>,
) -> Option<Reference<'a>> {
  let after_close = close_start + 1;
  // a document with no definitions has no reference to resolve against, so
  // only an inline link is possible
  if labels.is_empty() && text.byte(after_close) != Some(b'(') {
    return None;
  }

  if text.byte(after_close) == Some(b'(') {
    if let Some((end, url, title)) = match_inline_tail(text, after_close) {
      return Some(Reference {
        kind: ReferenceKind::Inline { url, title },
        end,
      });
    }
  }

  if text.byte(after_close) == Some(b'[') {
    if let Some((end, label)) = match_label(text, after_close) {
      if label.trim_matches(WHITESPACE).is_empty() {
        // ex. `[name][]`
        let name = text.slice(content_start, close_start);
        return labels.contains(&normalize_label(&name)).then_some(Reference {
          kind: ReferenceKind::Collapsed,
          end,
        });
      }
      return labels.contains(&normalize_label(&label)).then_some(Reference {
        kind: ReferenceKind::Full { label },
        end,
      });
    }
  }

  // ex. `[name]`
  let name = text.slice(content_start, close_start);
  labels.contains(&normalize_label(&name)).then_some(Reference {
    kind: ReferenceKind::Shortcut,
    end: after_close,
  })
}

/// Matches a `[^name]` footnote reference against the document's definitions.
pub fn match_footnote_reference<'a>(
  text: &InlineText<'a>,
  start: usize,
  labels: &HashSet<String>,
) -> Option<(usize, &'a str)> {
  if text.byte(start + 1) != Some(b'^') {
    return None;
  }
  let (end, _) = match_label(text, start)?;
  let name = text.source_slice(start + 2, end - 1)?;
  if name.is_empty() || !labels.contains(&normalize_label(name)) {
    return None;
  }
  Some((end, name))
}

/// The position after the backslash at `index`, which only escapes the ascii
/// punctuation that may follow it. A backslash before anything else, including
/// a line ending, is a literal one.
fn skip_escape(text: &InlineText<'_>, index: usize) -> usize {
  match text.byte(index + 1) {
    Some(byte) if byte.is_ascii_punctuation() => index + 2,
    _ => index + 1,
  }
}

/// Matches a `[label]`, returning the position after the closing bracket and
/// the text within it.
pub fn match_label<'a>(text: &InlineText<'a>, start: usize) -> Option<(usize, Cow<'a, str>)> {
  if text.byte(start) != Some(b'[') {
    return None;
  }
  let mut index = start + 1;
  while let Some(byte) = text.byte(index) {
    match byte {
      b'\\' => index = skip_escape(text, index),
      b'[' => return None, // an unescaped bracket can't appear in a label
      b']' => return Some((index + 1, text.slice(start + 1, index))),
      _ => index += 1,
    }
  }
  None
}

/// Matches the `(/url "title")` of an inline link, whose destination may be
/// empty (ex. `[text]()`).
fn match_inline_tail<'a>(text: &InlineText<'a>, start: usize) -> Option<(usize, Cow<'a, str>, Option<Cow<'a, str>>)> {
  let url_start = skip_whitespace(text, start + 1);
  let (after_url, url) = match_destination(text, url_start).unwrap_or((url_start, Cow::Borrowed("")));
  let mut index = skip_whitespace(text, after_url);

  // a title has to be separated from the destination by whitespace
  let mut title = None;
  if index > after_url || after_url == url_start {
    if let Some((after_title, matched)) = match_title(text, index) {
      title = Some(matched);
      index = skip_whitespace(text, after_title);
    }
  }

  (text.byte(index) == Some(b')')).then(|| (index + 1, url, title))
}

/// Matches a link destination, which is either wrapped in pointy brackets or a
/// run of characters with balanced parentheses.
pub fn match_destination<'a>(text: &InlineText<'a>, start: usize) -> Option<(usize, Cow<'a, str>)> {
  if text.byte(start) == Some(b'<') {
    let mut index = start + 1;
    while let Some(byte) = text.byte(index) {
      match byte {
        b'\\' => index = skip_escape(text, index),
        b'\n' | b'<' => return None,
        b'>' => return Some((index + 1, text.slice(start, index + 1))),
        _ => index += 1,
      }
    }
    return None;
  }

  let mut index = start;
  let mut depth = 0usize;
  while let Some(byte) = text.byte(index) {
    match byte {
      b'\\' => {
        index = skip_escape(text, index);
        continue;
      }
      b'(' => depth += 1,
      b')' => {
        if depth == 0 {
          break;
        }
        depth -= 1;
      }
      b' ' | b'\t' | b'\n' => break,
      byte if byte < 0x20 => break,
      _ => {}
    }
    index += 1;
  }
  (index > start).then(|| (index, text.slice(start, index)))
}

/// Matches a link title, which may be in double quotes, single quotes or
/// parentheses.
pub fn match_title<'a>(text: &InlineText<'a>, start: usize) -> Option<(usize, Cow<'a, str>)> {
  let close = match text.byte(start)? {
    b'"' => b'"',
    b'\'' => b'\'',
    b'(' => b')',
    _ => return None,
  };
  let mut index = start + 1;
  while let Some(byte) = text.byte(index) {
    match byte {
      b'\\' => index = skip_escape(text, index),
      byte if byte == close => return Some((index + 1, text.slice(start + 1, index))),
      // a blank line ends the title
      b'\n' if text.byte(index + 1) == Some(b'\n') => return None,
      _ => index += 1,
    }
  }
  None
}

/// Matches a link reference definition at the start of a block's content
/// (ex. `[name]: /url "title"`).
pub fn match_link_reference_definition<'a>(text: &InlineText<'a>, start: usize) -> Option<(usize, LinkReference<'a>)> {
  let start = skip_spaces(text, start);
  let (after_label, label) = match_label(text, start)?;
  if label.trim_matches(WHITESPACE).is_empty() || label.starts_with('^') {
    return None;
  }
  if text.byte(after_label) != Some(b':') {
    return None;
  }

  let url_start = skip_whitespace_to_next_block(text, after_label + 1)?;
  let (after_url, url) = match_destination(text, url_start)?;

  // a title has to either be on the same line as the destination or be all
  // that's on the next one
  let mut end = after_url;
  let mut title = None;
  let title_start = skip_whitespace_to_next_block(text, after_url).unwrap_or(after_url);
  if title_start > after_url {
    if let Some((after_title, matched)) = match_title(text, title_start) {
      let line_end = skip_spaces(text, after_title);
      if matches!(text.byte(line_end), None | Some(b'\n')) {
        title = Some(matched);
        end = after_title;
      }
    }
  }

  let line_end = skip_spaces(text, end);
  if !matches!(text.byte(line_end), None | Some(b'\n')) {
    return None;
  }

  Some((
    line_end,
    LinkReference {
      span: text.span(start, end),
      name: label,
      link: url,
      title,
    },
  ))
}

/// Normalizes a link label the way the CommonMark spec matches them: case
/// insensitively and with runs of whitespace collapsed.
pub fn normalize_label(label: &str) -> String {
  let mut result = String::with_capacity(label.len());
  let mut had_whitespace = false;
  for c in label.trim_matches(WHITESPACE).chars() {
    if WHITESPACE.contains(&c) {
      had_whitespace = true;
      continue;
    }
    if had_whitespace {
      result.push(' ');
      had_whitespace = false;
    }
    result.extend(c.to_lowercase());
  }
  result
}

/// Matches a `<https://example.com>` or `<name@example.com>` autolink.
pub fn match_autolink(text: &InlineText<'_>, start: usize) -> Option<usize> {
  let mut index = start + 1;
  let mut has_scheme = false;
  let mut has_at = false;

  while let Some(byte) = text.byte(index) {
    match byte {
      b'>' => {
        let is_autolink = has_scheme || has_at && index > start + 1;
        return (index > start + 1 && is_autolink).then_some(index + 1);
      }
      b' ' | b'\t' | b'\n' | b'<' => return None,
      b':' if !has_scheme && !has_at => {
        // a scheme is 2 to 32 characters of letters, digits, `+`, `.` or `-`
        // that start with a letter
        let scheme = text.str_between(start + 1, index);
        let mut chars = scheme.chars();
        let valid = (2..=32).contains(&scheme.len())
          && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
          && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
        if !valid {
          return None;
        }
        has_scheme = true;
      }
      b'@' if !has_scheme => has_at = true,
      _ => {}
    }
    index += 1;
  }

  None
}

/// Matches a raw html tag, comment, processing instruction, declaration or
/// CDATA section.
pub fn match_html_tag(text: &InlineText<'_>, start: usize) -> Option<usize> {
  match text.byte(start + 1)? {
    b'!' => match text.byte(start + 2) {
      Some(b'-') if text.starts_with_at(start, "<!--") => find_after(text, start + 4, "-->"),
      Some(b'[') if text.starts_with_at(start, "<![CDATA[") => find_after(text, start + 9, "]]>"),
      // a declaration is `<!` followed by an ascii letter
      Some(byte) if byte.is_ascii_alphabetic() => find_after(text, start + 2, ">"),
      _ => None,
    },
    b'?' => find_after(text, start + 2, "?>"),
    b'/' => match_closing_tag(text, start),
    byte if byte.is_ascii_alphabetic() => match_open_tag(text, start),
    _ => None,
  }
}

fn match_closing_tag(text: &InlineText<'_>, start: usize) -> Option<usize> {
  let mut index = start + 2;
  if !text.byte(index)?.is_ascii_alphabetic() {
    return None;
  }
  while text.byte(index).is_some_and(is_tag_name_byte) {
    index += 1;
  }
  index = skip_whitespace(text, index);
  (text.byte(index) == Some(b'>')).then_some(index + 1)
}

fn match_open_tag(text: &InlineText<'_>, start: usize) -> Option<usize> {
  let mut index = start + 1;
  while text.byte(index).is_some_and(is_tag_name_byte) {
    index += 1;
  }

  loop {
    let after_whitespace = skip_whitespace(text, index);
    match text.byte(after_whitespace)? {
      b'>' => return Some(after_whitespace + 1),
      b'/' if text.byte(after_whitespace + 1) == Some(b'>') => return Some(after_whitespace + 2),
      byte if is_attribute_name_start(byte) && after_whitespace > index => {
        index = match_attribute(text, after_whitespace)?;
      }
      _ => return None,
    }
  }
}

fn match_attribute(text: &InlineText<'_>, start: usize) -> Option<usize> {
  let mut index = start;
  while text.byte(index).is_some_and(is_attribute_name_byte) {
    index += 1;
  }
  let after_name = index;
  index = skip_whitespace(text, index);
  if text.byte(index) != Some(b'=') {
    return Some(after_name);
  }
  index = skip_whitespace(text, index + 1);

  match text.byte(index)? {
    quote @ (b'"' | b'\'') => {
      index += 1;
      while text.byte(index)? != quote {
        index += 1;
      }
      Some(index + 1)
    }
    _ => {
      let value_start = index;
      while text
        .byte(index)
        .is_some_and(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'"' | b'\'' | b'=' | b'<' | b'>' | b'`'))
      {
        index += 1;
      }
      (index > value_start).then_some(index)
    }
  }
}

fn is_tag_name_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || byte == b'-'
}

fn is_attribute_name_start(byte: u8) -> bool {
  byte.is_ascii_alphabetic() || matches!(byte, b'_' | b':')
}

fn is_attribute_name_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':' | b'.' | b'-')
}

fn find_after(text: &InlineText<'_>, start: usize, close: &str) -> Option<usize> {
  let mut index = start;
  while index < text.len() {
    if text.starts_with_at(index, close) {
      return Some(index + close.len());
    }
    index += 1;
  }
  None
}

pub fn skip_spaces(text: &InlineText<'_>, start: usize) -> usize {
  let mut index = start;
  while matches!(text.byte(index), Some(b' ') | Some(b'\t')) {
    index += 1;
  }
  index
}

pub fn skip_whitespace(text: &InlineText<'_>, start: usize) -> usize {
  let mut index = start;
  while matches!(text.byte(index), Some(b' ') | Some(b'\t') | Some(b'\n')) {
    index += 1;
  }
  index
}

/// Skips whitespace, giving up when it contains a blank line because that
/// would end the block.
fn skip_whitespace_to_next_block(text: &InlineText<'_>, start: usize) -> Option<usize> {
  let mut index = start;
  let mut newlines = 0;
  while let Some(byte) = text.byte(index) {
    match byte {
      b' ' | b'\t' => {}
      b'\n' => {
        newlines += 1;
        if newlines > 1 {
          return None;
        }
      }
      _ => break,
    }
    index += 1;
  }
  Some(index)
}
