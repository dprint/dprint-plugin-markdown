use std::borrow::Cow;
use std::ops::Range;
use std::ops::RangeInclusive;

use dprint_core::configuration::resolve_new_line_kind;
use dprint_core::configuration::NewLineKind;

use super::configuration::Configuration;
use super::format_text::format_text;
use super::format_text::format_text_inner;
use super::format_text::strip_bom;
use super::format_text::FormatError;
use super::generation::is_ignore_comment;
use super::generation::strip_metadata_header;
use super::parser::Node;
use super::parser::Ranged;
use super::parser::SourceFile;
use super::parser::WHITESPACE;

/// Formats only the part of the text within the provided byte range.
///
/// The range is widened to the lines of the blocks it touches in the innermost
/// block quote or list (going by its items) that contains it, and the text
/// outside of those is left as it was. When those blocks aren't written as the
/// same blocks once formatted, or their text can't be put back in place of the
/// original without changing how the text around it reads, the block holding
/// them is formatted instead, and so on out to the whole file. A range that
/// reaches from before the first block to after the last formats the whole
/// file and one that only touches the blank lines between blocks formats
/// nothing.
///
/// Only the code blocks within what's formatted are passed to
/// `format_code_block_text`.
pub fn format_text_range(
  file_text: &str,
  range: Range<usize>,
  config: &Configuration,
  mut format_code_block_text: impl for<'a> FnMut(&str, &'a str, u32) -> Result<Option<String>, FormatError>,
) -> Result<Option<String>, FormatError> {
  let body = strip_bom(file_text);
  let bom_len = file_text.len() - body.len();
  let end = range.end.saturating_sub(bom_len).min(body.len());
  let range = range.start.saturating_sub(bom_len).min(end)..end;
  if is_ignore_comment(strip_metadata_header(body), &config.ignore_file_directive) {
    return Ok(None);
  }
  // a lone carriage return breaks a line as much as a line feed does, which
  // is rare enough to leave to formatting the whole file
  if has_lone_carriage_return(body) {
    return format_text(file_text, config, format_code_block_text);
  }
  let Ok(source_file) = crate::parser::parse(body) else {
    return format_text(file_text, config, format_code_block_text);
  };
  let levels = match find_levels(&source_file, &range) {
    Target::File => return format_text(file_text, config, format_code_block_text),
    Target::Nothing => return Ok(None),
    Target::Levels(levels) => levels,
  };

  // the code blocks are only formatted within the innermost level at first,
  // since formatting them is what's slow, and within an outer level once
  // that's the one the replacement is made at
  let mut depth = levels.len() - 1;
  let replacement = loop {
    let formatted = format_text_inner(
      body,
      config,
      Some(levels[depth].replaced_range(body)),
      &mut format_code_block_text,
    )?
    .expect("an ignored file should have been handled");
    match find_replacement(&levels[..=depth], body, &formatted) {
      Some((replaced_depth, replacement)) if replaced_depth == depth => break replacement,
      Some((replaced_depth, _)) => depth = replaced_depth,
      None => return format_text(file_text, config, format_code_block_text),
    }
  };

  let result = format!(
    "{}{}{}",
    &file_text[..bom_len + replacement.original.start],
    replacement.text,
    &file_text[bom_len + replacement.original.end..]
  );
  if result == file_text {
    Ok(None)
  } else {
    Ok(Some(result))
  }
}

enum Target<'a, 'b> {
  /// The range touches both the first and last block of the file.
  File,
  /// The range is outside of the blocks or only touches the blank lines
  /// between them.
  Nothing,
  /// Each container from the file to the innermost one holding the range.
  Levels(Vec<Level<'a, 'b>>),
}

/// A container along with the blocks of it to format.
struct Level<'a, 'b> {
  members: Vec<&'b Node<'a>>,
  /// Indexes of the members to format. For all but the innermost level, this
  /// is only the member holding the next level.
  indexes: RangeInclusive<usize>,
}

impl Level<'_, '_> {
  fn replaced_range(&self, text: &str) -> Range<usize> {
    let first = &self.members[*self.indexes.start()];
    let last = &self.members[*self.indexes.end()];
    line_start(text, first.span().start)..member_end(text, last)
  }
}

struct Replacement {
  original: Range<usize>,
  text: String,
}

fn find_levels<'a, 'b>(source_file: &'b SourceFile<'a>, range: &Range<usize>) -> Target<'a, 'b> {
  let children = &source_file.children;
  let (Some(first), Some(last)) = (children.first(), children.last()) else {
    return Target::Nothing;
  };
  if range.start <= first.span().start && range.end >= last.span().end {
    return Target::File;
  }
  let mut levels = Vec::new();
  let mut members = children.iter().collect::<Vec<_>>();
  loop {
    let mut touched = members
      .iter()
      .enumerate()
      .filter(|(_, member)| touches(member.span(), range))
      .map(|(index, _)| index);
    let Some(first) = touched.next() else {
      // a range between the blocks of a container formats the container
      return if levels.is_empty() {
        Target::Nothing
      } else {
        Target::Levels(levels)
      };
    };
    let last = touched.next_back().unwrap_or(first);
    let member = members[first];
    let inner = Some(member)
      .filter(|member| first == last && !covers(range, member.span()))
      .and_then(|member| inner_members(member))
      .filter(|children| !children.is_empty());
    levels.push(Level {
      members,
      indexes: first..=last,
    });
    match inner {
      Some(inner) => members = inner,
      // the start and end of the file are only formatted along with all of it
      None if levels.len() == 1 && first == 0 && last == children.len() - 1 => return Target::File,
      None => return Target::Levels(levels),
    }
  }
}

/// Finds the text to replace in the original and the formatted text to
/// replace it with, starting with the innermost level and moving out towards
/// the file, along with the depth of the level it's made at.
fn find_replacement(levels: &[Level], text: &str, formatted_text: &str) -> Option<(usize, Replacement)> {
  let formatted_file = crate::parser::parse(formatted_text).ok()?;
  let mut formatted_levels = Vec::with_capacity(levels.len());
  let mut formatted_members = formatted_file.children.iter().collect::<Vec<_>>();
  for level in levels {
    if !same_kinds(&level.members, &formatted_members) {
      break;
    }
    let next = inner_members(formatted_members[*level.indexes.start()]).unwrap_or_default();
    formatted_levels.push(std::mem::replace(&mut formatted_members, next));
  }

  levels
    .iter()
    .zip(formatted_levels)
    .enumerate()
    .rev()
    .find_map(|(depth, (level, formatted_members))| {
      // the line prefixes of the containers around a nested block stay as
      // they were, so they need to already be what the formatter writes,
      // which is checked for every block of the container so that its blocks
      // don't end up indented differently from one another
      let same_prefixes = level
        .members
        .iter()
        .zip(&formatted_members)
        .all(|(original, formatted)| {
          let (original_start, formatted_start) = (original.span().start, formatted.span().start);
          text[line_start(text, original_start)..original_start]
            == formatted_text[line_start(formatted_text, formatted_start)..formatted_start]
        });
      if depth > 0 && !same_prefixes {
        return None;
      }
      let original = level.replaced_range(text);
      let formatted_range = Level {
        members: formatted_members,
        indexes: level.indexes.clone(),
      }
      .replaced_range(formatted_text);
      let replacement = Replacement {
        original,
        text: with_file_new_lines(&formatted_text[formatted_range.clone()], text).into_owned(),
      };
      reads_back(levels, depth, text, &replacement, &formatted_text[formatted_range]).then_some((depth, replacement))
    })
}

/// Whether the text with the replacement made holds the same blocks as before
/// along the path down to the level, with the replaced ones written as they
/// were formatted and the others written as they were.
fn reads_back(levels: &[Level], depth: usize, text: &str, replacement: &Replacement, formatted: &str) -> bool {
  let result = format!(
    "{}{}{}",
    &text[..replacement.original.start],
    replacement.text,
    &text[replacement.original.end..]
  );
  let Ok(source_file) = crate::parser::parse(&result) else {
    return false;
  };
  let mut members = source_file.children.iter().collect::<Vec<_>>();
  for level in &levels[..depth] {
    if !same_kinds(&level.members, &members) || !same_others(level, &members, text, &result) {
      return false;
    }
    let Some(next) = inner_members(members[*level.indexes.start()]) else {
      return false;
    };
    members = next;
  }
  let level = &levels[depth];
  if !same_kinds(&level.members, &members) || !same_others(level, &members, text, &result) {
    return false;
  }
  let replaced = Level {
    members,
    indexes: level.indexes.clone(),
  }
  .replaced_range(&result);
  normalize_new_lines(&result[replaced]) == normalize_new_lines(formatted)
}

/// The blocks within a container that can be formatted on their own.
fn inner_members<'a, 'b>(node: &'b Node<'a>) -> Option<Vec<&'b Node<'a>>> {
  match node {
    Node::List(list) => Some(list.children.iter().collect()),
    Node::BlockQuote(quote) => Some(quote.children.iter().collect()),
    Node::Item(item) => Some(item.children.iter().chain(&item.sub_lists).collect()),
    _ => None,
  }
}

/// Whether the members of a level other than the ones to format are written
/// as they were.
fn same_others(level: &Level, members: &[&Node], text: &str, result: &str) -> bool {
  level
    .members
    .iter()
    .zip(members)
    .enumerate()
    .filter(|(index, _)| !level.indexes.contains(index))
    .all(|(_, (original, member))| original.span().text(text) == member.span().text(result))
}

fn same_kinds(original: &[&Node], formatted: &[&Node]) -> bool {
  original.len() == formatted.len()
    && original
      .iter()
      .zip(formatted)
      .all(|(a, b)| std::mem::discriminant(*a) == std::mem::discriminant(*b))
}

fn touches(member: crate::parser::Span, range: &Range<usize>) -> bool {
  if range.is_empty() {
    member.start <= range.start && range.start <= member.end
  } else {
    range.start < member.end && range.end > member.start
  }
}

fn covers(range: &Range<usize>, span: crate::parser::Span) -> bool {
  range.start <= span.start && range.end >= span.end
}

fn line_start(text: &str, pos: usize) -> usize {
  text[..pos].rfind('\n').map(|index| index + 1).unwrap_or(0)
}

/// The end of a member's text along with any whitespace after it on its last
/// line, without the line break.
fn member_end(text: &str, member: &Node) -> usize {
  let span = member.span();
  let end = span.start + span.text(text).trim_end_matches(WHITESPACE).len();
  let rest = &text[end..];
  let line = rest.find('\n').map(|index| &rest[..index]).unwrap_or(rest);
  if line.trim_matches(WHITESPACE).is_empty() {
    end + line.trim_end_matches('\r').len()
  } else {
    end
  }
}

fn has_lone_carriage_return(text: &str) -> bool {
  text
    .match_indices('\r')
    .any(|(index, _)| text.as_bytes().get(index + 1) != Some(&b'\n'))
}

fn normalize_new_lines(text: &str) -> Cow<'_, str> {
  if text.contains("\r\n") {
    Cow::Owned(text.replace("\r\n", "\n"))
  } else {
    Cow::Borrowed(text)
  }
}

/// Keeps the line endings of the rest of the file since changing those is up
/// to formatting the whole file.
fn with_file_new_lines<'a>(formatted: &'a str, file_text: &str) -> Cow<'a, str> {
  if !file_text.contains('\n') {
    return Cow::Borrowed(formatted);
  }
  match resolve_new_line_kind(file_text, NewLineKind::Auto) {
    "\r\n" if formatted.contains('\n') && !formatted.contains("\r\n") => Cow::Owned(formatted.replace('\n', "\r\n")),
    "\n" if formatted.contains("\r\n") => Cow::Owned(formatted.replace("\r\n", "\n")),
    _ => Cow::Borrowed(formatted),
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::configuration::ConfigurationBuilder;

  #[test]
  fn keeps_bom_outside_range() {
    // the spec files can't express this since editors strip the bom
    let config = ConfigurationBuilder::new().build();
    let text = "\u{FEFF}a    b\n\nc    d\n";
    let output = format_c(&config, text);
    assert_eq!(output, "\u{FEFF}a    b\n\nc d\n");
  }

  #[test]
  fn keeps_file_line_endings() {
    // the spec files can't express this since they normalize line endings
    let config = ConfigurationBuilder::new().build();
    let output = format_c(&config, "a    b\r\n\r\n- c    d\r\n  - e\r\n");
    assert_eq!(output, "a    b\r\n\r\n- c d\r\n  - e\r\n");

    let config = ConfigurationBuilder::new()
      .new_line_kind(NewLineKind::CarriageReturnLineFeed)
      .build();
    let output = format_c(&config, "a    b\n\n- c    d\n  - e\n");
    assert_eq!(output, "a    b\n\n- c d\n  - e\n");
  }

  #[test]
  fn only_formats_code_blocks_within_range() {
    let config = ConfigurationBuilder::new().build();
    let text = "```js\na\n```\n\n```js\nb\n```\n\n```js\nc\n```\n";
    let start = text.find('b').unwrap();
    let mut calls = Vec::new();
    let output = format_text_range(text, start..start + 1, &config, |_, code, _| {
      calls.push(code.to_string());
      Ok(Some(format!("{}2", code)))
    })
    .unwrap();
    assert_eq!(calls, vec!["b"]);
    assert_eq!(
      output.as_deref(),
      Some("```js\na\n```\n\n```js\nb2\n```\n\n```js\nc\n```\n")
    );
  }

  #[test]
  fn ignores_file_with_ignore_file_directive() {
    let config = ConfigurationBuilder::new().build();
    let text = "<!-- dprint-ignore-file -->\n\nc    d\n";
    let start = text.find('c').unwrap();
    let output = format_text_range(text, start..start + 1, &config, |_, _, _| Ok(None)).unwrap();
    assert_eq!(output, None);
  }

  #[test]
  fn formats_whole_file_with_lone_carriage_returns() {
    let config = ConfigurationBuilder::new().build();
    let text = "> a\n> important text\r\r# h    x\n";
    let start = text.find("h ").unwrap();
    let output = format_text_range(text, start..start + 1, &config, |_, _, _| Ok(None)).unwrap();
    assert_eq!(output.as_deref(), Some("> a\n> important text\n\n# h x\n"));
  }

  #[test]
  fn formats_code_blocks_of_widened_region() {
    // the list markers change, so the whole list is formatted
    let config = ConfigurationBuilder::new().build();
    let text = "* one\n\n  ```js\n  code\n  ```\n* two    x\n\nend\n";
    let start = text.find("two").unwrap();
    let output = format_text_range(text, start..start + 1, &config, |_, code, _| {
      Ok(Some(code.to_uppercase()))
    })
    .unwrap();
    assert_eq!(
      output.as_deref(),
      Some("- one\n\n  ```js\n  CODE\n  ```\n- two x\n\nend\n")
    );
  }

  #[test]
  fn formats_unclosed_code_block_at_end_of_file() {
    let config = ConfigurationBuilder::new().build();
    let text = "a    b\n\n~~~js\ncode\n\n";
    let start = text.find("code").unwrap();
    let output = format_text_range(text, start..start + 1, &config, |_, code, _| {
      Ok(Some(code.to_uppercase()))
    })
    .unwrap();
    assert_eq!(output.as_deref(), Some("a    b\n\n```js\nCODE\n```\n\n"));
  }

  #[test]
  fn does_not_format_front_matter_outside_range() {
    let config = ConfigurationBuilder::new().build();
    let text = "---\ntitle: x\n---\n\n# Heading\n\nText    here.\n\nend\n";
    let start = text.find("Text").unwrap();
    let mut calls = Vec::new();
    let output = format_text_range(text, start..start + 1, &config, |tag, _, _| {
      calls.push(tag.to_string());
      Ok(None)
    })
    .unwrap();
    assert!(calls.is_empty(), "{:?}", calls);
    assert_eq!(
      output.as_deref(),
      Some("---\ntitle: x\n---\n\n# Heading\n\nText here.\n\nend\n")
    );
  }

  #[test]
  fn formats_whole_file_when_touching_first_and_last_blocks() {
    let config = ConfigurationBuilder::new().build();
    let output = format_text_range("a    b", 0..3, &config, |_, _, _| Ok(None)).unwrap();
    assert_eq!(output.as_deref(), Some("a b\n"));
  }

  #[test]
  fn formats_item_when_siblings_indentation_changes() {
    let config = ConfigurationBuilder::new()
      .list_indent_kind(crate::configuration::ListIndentKind::PythonMarkdown)
      .build();
    let text = "- foo\n  - bar\n\n  baz\n\nend\n";
    let start = text.find("bar").unwrap();
    let output = format_text_range(text, start..start + 1, &config, |_, _, _| Ok(None)).unwrap();
    let full = crate::format_text(text, &config, |_, _, _| Ok(None)).unwrap();
    assert_eq!(output, full);
  }

  fn format_c(config: &Configuration, text: &str) -> String {
    let start = text.find('c').unwrap();
    format_text_range(text, start..start + 1, config, |_, _, _| Ok(None))
      .unwrap()
      .unwrap()
  }
}
