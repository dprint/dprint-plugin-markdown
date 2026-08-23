// The line oriented view of the source text that block parsing works over.
//
// Markdown's block structure is defined by what prefixes a line (ex. the `> `
// of a block quote or the indentation of a list item), so the parser works
// with lines that have had the prefixes of their surrounding containers
// stripped off. A [`ContentLine`] is such a line: it borrows the remainder of
// a source line and remembers where it came from so that spans stay accurate.

use std::borrow::Cow;

use super::ast::Span;

/// The width of a tab stop, which indentation is measured against.
const TAB_STOP: usize = 4;

/// A line of content with the prefixes of its surrounding containers removed.
#[derive(Debug, Clone, Copy)]
pub struct ContentLine<'a> {
  /// The absolute byte offset `text` starts at.
  pub start: usize,
  /// The rest of the line, not including its terminator.
  pub text: &'a str,
  /// Spaces that logically precede `text`, left over from a tab that a
  /// container's prefix only partially consumed.
  pub virtual_spaces: usize,
  /// The column of the original line the content begins at, which tab stops
  /// are measured against.
  pub column: usize,
  /// Whether the line was taken as the lazy continuation of a paragraph within
  /// a container it doesn't carry the prefix of. Such a line can only ever be
  /// more of that paragraph, so nothing that would otherwise begin on it
  /// (ex. a setext heading's underline, or another list item) does.
  pub is_lazy: bool,
}

impl<'a> ContentLine<'a> {
  pub fn end(&self) -> usize {
    self.start + self.text.len()
  }

  pub fn span(&self) -> Span {
    Span::new(self.start, self.end())
  }

  pub fn is_blank(&self) -> bool {
    self.text.bytes().all(|b| b == b' ' || b == b'\t')
  }

  /// How many columns of whitespace the line begins with.
  pub fn indent_columns(&self) -> usize {
    let mut column = self.column + self.virtual_spaces;
    for byte in self.text.bytes() {
      match byte {
        b' ' => column += 1,
        b'\t' => column += TAB_STOP - (column % TAB_STOP),
        _ => break,
      }
    }
    column - self.column
  }

  /// The line's content with its leading whitespace removed.
  ///
  /// Any leftover virtual spaces are dropped, so only use this where the
  /// whitespace isn't significant.
  pub fn trimmed(&self) -> ContentLine<'a> {
    self.strip_columns(usize::MAX)
  }

  /// Removes up to `columns` columns of leading whitespace.
  ///
  /// A tab that straddles the requested column becomes virtual spaces on the
  /// returned line so that the rest of it stays put.
  pub fn strip_columns(&self, columns: usize) -> ContentLine<'a> {
    let mut line = *self;
    let mut remaining = columns;

    let virtual_spaces = line.virtual_spaces.min(remaining);
    line.virtual_spaces -= virtual_spaces;
    line.column += virtual_spaces;
    remaining -= virtual_spaces;
    if line.virtual_spaces > 0 || remaining == 0 {
      return line;
    }

    let mut offset = 0;
    for byte in line.text.bytes() {
      let width = match byte {
        b' ' => 1,
        b'\t' => TAB_STOP - (line.column % TAB_STOP),
        _ => break,
      };
      if width > remaining {
        // a tab that reaches past the requested column keeps the rest of its
        // width as virtual spaces
        line.virtual_spaces = width - remaining;
        line.column += remaining;
        offset += 1;
        break;
      }
      line.column += width;
      remaining -= width;
      offset += 1;
      if remaining == 0 {
        break;
      }
    }

    line.start += offset;
    line.text = &line.text[offset..];
    line
  }

  /// Removes `count` bytes from the start of the line, which must not be
  /// whitespace.
  pub fn strip_bytes(&self, count: usize) -> ContentLine<'a> {
    debug_assert_eq!(self.virtual_spaces, 0);
    ContentLine {
      start: self.start + count,
      text: &self.text[count..],
      virtual_spaces: 0,
      column: self.column + count,
      is_lazy: self.is_lazy,
    }
  }

  /// The line as the lazy continuation of a paragraph in the container above
  /// it.
  pub fn as_lazy(&self) -> ContentLine<'a> {
    ContentLine { is_lazy: true, ..*self }
  }

  /// The line with its trailing whitespace removed.
  pub fn trim_end(&self) -> ContentLine<'a> {
    let mut line = *self;
    line.text = line.text.trim_end_matches([' ', '\t']);
    line
  }

  /// The line's text with its leading whitespace removed, ignoring any
  /// leftover virtual spaces.
  pub fn rest(&self) -> &'a str {
    self.trimmed().text
  }

  /// The absolute position the line's text starts at, after its leading
  /// whitespace.
  pub fn rest_start(&self) -> usize {
    self.trimmed().start
  }

  /// Whether the content (ignoring leading whitespace) starts with `prefix`.
  pub fn starts_with(&self, prefix: &str) -> bool {
    self.rest().starts_with(prefix)
  }
}

/// Splits the source text into its lines, not including their terminators.
pub fn source_lines(source: &str) -> Vec<ContentLine<'_>> {
  let mut lines = Vec::new();
  let mut start = 0;
  let bytes = source.as_bytes();

  for (index, byte) in bytes.iter().enumerate() {
    if *byte != b'\n' {
      continue;
    }
    let mut end = index;
    if end > start && bytes[end - 1] == b'\r' {
      end -= 1;
    }
    lines.push(ContentLine {
      start,
      text: &source[start..end],
      virtual_spaces: 0,
      column: 0,
      is_lazy: false,
    });
    start = index + 1;
  }

  // the text after the last line terminator, which is only a line when it
  // isn't empty
  if start < source.len() {
    lines.push(ContentLine {
      start,
      text: &source[start..],
      virtual_spaces: 0,
      column: 0,
      is_lazy: false,
    });
  }

  lines
}

/// Joins the lines into a single string, separated by newlines.
///
/// Borrows from the source when the lines are contiguous within it, which is
/// the case whenever no container prefixes had to be stripped out of them.
pub fn join_lines<'a>(lines: &[ContentLine<'a>], source: &'a str, trailing_newline: bool) -> Cow<'a, str> {
  let Some(first) = lines.first() else {
    return Cow::Borrowed("");
  };
  let last = lines.last().unwrap();

  if let Some(text) = try_borrow_lines(lines, source, trailing_newline) {
    return Cow::Borrowed(text);
  }

  let mut text = String::with_capacity(last.end() - first.start + lines.len());
  for (index, line) in lines.iter().enumerate() {
    if index > 0 {
      text.push('\n');
    }
    for _ in 0..line.virtual_spaces {
      text.push(' ');
    }
    text.push_str(line.text);
  }
  if trailing_newline {
    text.push('\n');
  }
  Cow::Owned(text)
}

/// Borrows the lines directly from the source when they're contiguous within
/// it and separated by single newlines.
fn try_borrow_lines<'a>(lines: &[ContentLine<'a>], source: &'a str, trailing_newline: bool) -> Option<&'a str> {
  let bytes = source.as_bytes();
  let first = lines.first()?;
  let last = lines.last()?;

  let mut previous_end = None;
  for line in lines {
    if line.virtual_spaces > 0 {
      return None;
    }
    if let Some(previous_end) = previous_end {
      if line.start != previous_end + 1 || bytes[previous_end] != b'\n' {
        return None;
      }
    }
    previous_end = Some(line.end());
  }

  let mut end = last.end();
  if trailing_newline {
    if bytes.get(end) != Some(&b'\n') {
      return None;
    }
    end += 1;
  }

  Some(&source[first.start..end])
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn splits_lines_without_terminators() {
    let source = "a\r\nb\n\nc";
    let lines = source_lines(source);
    assert_eq!(
      lines.iter().map(|l| l.text).collect::<Vec<_>>(),
      vec!["a", "b", "", "c"]
    );
    assert_eq!(lines.iter().map(|l| l.start).collect::<Vec<_>>(), vec![0, 3, 5, 6]);
  }

  #[test]
  fn measures_indent_with_tab_stops() {
    let source = "\tfoo";
    let line = source_lines(source)[0];
    assert_eq!(line.indent_columns(), 4);

    let source = " \tfoo";
    let line = source_lines(source)[0];
    assert_eq!(line.indent_columns(), 4);
  }

  #[test]
  fn strips_nothing_when_no_columns_are_asked_for() {
    let source = "	foo";
    let line = source_lines(source)[0].strip_columns(0);
    assert_eq!(line.virtual_spaces, 0);
    assert_eq!(line.text, "	foo");
  }

  #[test]
  fn strips_a_straddling_tab_into_virtual_spaces() {
    let source = "\tfoo";
    let line = source_lines(source)[0].strip_columns(2);
    assert_eq!(line.virtual_spaces, 2);
    assert_eq!(line.text, "foo");
    assert_eq!(line.start, 1);
  }

  #[test]
  fn borrows_contiguous_lines() {
    let source = "one\ntwo\n";
    let lines = source_lines(source);
    assert!(matches!(join_lines(&lines, source, true), Cow::Borrowed("one\ntwo\n")));
  }

  #[test]
  fn allocates_for_non_contiguous_lines() {
    let source = "> one\n> two\n";
    let lines: Vec<_> = source_lines(source).iter().map(|l| l.strip_columns(0)).collect();
    let lines: Vec<_> = lines
      .iter()
      .map(|l| {
        let mut line = *l;
        line.start += 2;
        line.text = &line.text[2..];
        line
      })
      .collect();
    assert_eq!(
      join_lines(&lines, source, false),
      Cow::Owned::<str>("one\ntwo".to_string())
    );
  }
}
