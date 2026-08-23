// Inline parsing, the second of the parser's two phases.
//
// Block parsing hands each leaf block the lines that make up its content, and
// this turns those into inline nodes. The content of a block isn't necessarily
// contiguous in the source (ex. the lines of a paragraph in a block quote are
// separated by `> ` prefixes), so it's presented to the parser as a flat view
// by [`InlineText`], which maps back to the absolute positions the nodes need.

use std::borrow::Cow;
use std::collections::HashSet;

use super::ast::*;
use super::links;
use super::links::ReferenceKind;
use super::source::ContentLine;

/// The document wide information inline parsing needs.
pub struct InlineContext<'a> {
  pub source: &'a str,
  /// The normalized labels of the document's link reference definitions,
  /// which say whether a `[label]` is a link or plain text.
  pub link_labels: HashSet<String>,
  /// The labels of the document's footnote definitions.
  pub footnote_labels: HashSet<String>,
  /// Whether this is the pass that only looks for definitions, in which case
  /// there's no point in doing the work of parsing inlines.
  pub collect_only: bool,
}

pub fn parse_inlines<'a>(lines: &[ContentLine<'a>], context: &InlineContext<'a>) -> Vec<Node<'a>> {
  if context.collect_only || lines.is_empty() {
    return Vec::new();
  }
  let text = InlineText::new(lines, context.source);
  let mut parser = InlineParser {
    text: &text,
    context,
    pos: 0,
    pending_text_start: 0,
    nodes: Vec::new(),
    delimiters: Vec::new(),
  };
  parser.parse();
  parser.finish()
}

// ==== the flat view of a block's content ====

/// A block's content as one continuous run of bytes, mapping back to where in
/// the source each of them came from.
pub struct InlineText<'a> {
  source: &'a str,
  text: Cow<'a, str>,
  /// The absolute position `text` starts at, when it's borrowed from the
  /// source and so maps to it one to one.
  base: usize,
  /// The absolute position of every byte of `text` plus its end, when the
  /// content had to be built up rather than borrowed.
  offsets: Option<Vec<u32>>,
}

impl<'a> InlineText<'a> {
  pub fn new(lines: &[ContentLine<'a>], source: &'a str) -> InlineText<'a> {
    if let Some(text) = try_borrow_content(lines, source) {
      return InlineText {
        source,
        text: Cow::Borrowed(text),
        base: lines[0].start,
        offsets: None,
      };
    }

    let mut text = String::new();
    let mut offsets: Vec<u32> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
      if index > 0 {
        text.push('\n');
        offsets.push(lines[index - 1].end() as u32);
      }
      for _ in 0..line.virtual_spaces {
        text.push(' ');
        offsets.push(line.start as u32);
      }
      text.push_str(line.text);
      for i in 0..line.text.len() {
        offsets.push((line.start + i) as u32);
      }
    }
    offsets.push(lines.last().unwrap().end() as u32);

    InlineText {
      source,
      text: Cow::Owned(text),
      base: lines[0].start,
      offsets: Some(offsets),
    }
  }

  pub fn source(&self) -> &'a str {
    self.source
  }

  pub fn len(&self) -> usize {
    self.text.len()
  }

  pub fn bytes(&self) -> &[u8] {
    self.text.as_bytes()
  }

  pub fn byte(&self, index: usize) -> Option<u8> {
    self.bytes().get(index).copied()
  }

  /// The absolute position in the source of the byte at `index`.
  pub fn abs(&self, index: usize) -> usize {
    match &self.offsets {
      Some(offsets) => offsets[index.min(offsets.len() - 1)] as usize,
      None => self.base + index,
    }
  }

  pub fn span(&self, start: usize, end: usize) -> Span {
    Span::new(self.abs(start), self.abs(end))
  }

  /// The text between the two indexes, borrowed from the source whenever it's
  /// contiguous within it.
  pub fn slice(&self, start: usize, end: usize) -> Cow<'a, str> {
    match self.source_slice(start, end) {
      Some(text) => Cow::Borrowed(text),
      None => Cow::Owned(self.text[start..end].to_string()),
    }
  }

  /// The text between the two indexes when it's contiguous in the source.
  pub fn source_slice(&self, start: usize, end: usize) -> Option<&'a str> {
    let Some(offsets) = &self.offsets else {
      return Some(&self.source[self.base + start..self.base + end]);
    };
    // the range came from one run of the source only if each of its bytes
    // sits directly after the one before it. Comparing the length wouldn't
    // do, since the virtual spaces a stripped tab leaves behind share a
    // position and can make up for the gap a stripped prefix left.
    if offsets[start..=end].windows(2).any(|pair| pair[1] != pair[0] + 1) {
      return None;
    }
    Some(&self.source[offsets[start] as usize..offsets[end] as usize])
  }

  pub fn str_between(&self, start: usize, end: usize) -> &str {
    &self.text[start..end]
  }

  pub fn char_before(&self, index: usize) -> Option<char> {
    self.text[..index].chars().next_back()
  }

  pub fn char_at(&self, index: usize) -> Option<char> {
    self.text[index..].chars().next()
  }

  /// Whether the text at the index begins with the prefix.
  ///
  /// The index comes from scanners that walk a byte at a time, so it may land
  /// within a character and can't be used to slice the text.
  pub fn starts_with_at(&self, index: usize, prefix: &str) -> bool {
    self
      .bytes()
      .get(index..)
      .is_some_and(|rest| rest.starts_with(prefix.as_bytes()))
  }
}

/// The content of the lines when it's one contiguous slice of the source.
fn try_borrow_content<'a>(lines: &[ContentLine<'a>], source: &'a str) -> Option<&'a str> {
  let bytes = source.as_bytes();
  let first = lines.first()?;
  let mut previous_end: Option<usize> = None;
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
  Some(&source[first.start..previous_end.unwrap()])
}

// ==== the inline parser ====

struct InlineParser<'a, 'b> {
  text: &'b InlineText<'a>,
  context: &'b InlineContext<'a>,
  pos: usize,
  /// Where the text that hasn't been turned into a node yet begins.
  pending_text_start: usize,
  /// The nodes parsed so far. Emphasis leaves holes behind when it wraps the
  /// nodes it spans, so a slot may be empty.
  nodes: Vec<Option<Node<'a>>>,
  delimiters: Vec<Delimiter>,
}

/// An unmatched `*`, `_`, `~`, `[` or `![` that a later character may pair
/// with.
struct Delimiter {
  kind: DelimiterKind,
  /// The node holding the delimiter's characters.
  node_index: usize,
  /// Where the delimiter's characters start and end, which shrinks as they're
  /// used up.
  start: usize,
  end: usize,
  /// How many characters the run held to begin with, which is what the rule
  /// of three weighs.
  original_len: usize,
  can_open: bool,
  can_close: bool,
  /// Whether the delimiter is still available to be matched, which a bracket
  /// stops being once it's within a link.
  active: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum DelimiterKind {
  Emphasis(u8),
  Link,
  Image,
}

impl<'a, 'b> InlineParser<'a, 'b> {
  fn parse(&mut self) {
    while let Some(byte) = self.text.byte(self.pos) {
      match byte {
        b'\\' => self.parse_backslash(),
        b'`' => self.parse_code_span(),
        b'\n' => self.parse_line_break(),
        b'<' => self.parse_angle(),
        b'[' => self.parse_open_bracket(),
        b'!' if self.text.byte(self.pos + 1) == Some(b'[') => self.parse_open_bracket(),
        b']' => self.parse_close_bracket(),
        b'*' | b'_' | b'~' => self.parse_emphasis_delimiter(byte),
        b'$' => self.parse_math(),
        _ => self.pos += 1,
      }
    }
  }

  fn finish(mut self) -> Vec<Node<'a>> {
    self.flush_text(self.text.len());
    self.process_emphasis(0);
    let source = self.text.source();
    merge_adjacent_text(self.nodes.into_iter().flatten().collect(), source)
  }

  // ---- text ----

  /// Turns the text between the last node and `end` into a text node.
  fn flush_text(&mut self, end: usize) {
    let start = self.pending_text_start;
    self.pending_text_start = end;
    if start >= end {
      return;
    }
    if let Some(node) = self.create_text(start, end) {
      self.nodes.push(Some(node));
    }
  }

  /// Creates a text node for the range, with its surrounding spaces trimmed
  /// off because the formatter decides the spacing between nodes itself.
  fn create_text(&self, start: usize, end: usize) -> Option<Node<'a>> {
    let text = self.text.str_between(start, end);
    let trimmed_start = start + (text.len() - text.trim_start_matches([' ', '\t']).len());
    let trimmed_end = end - (text.len() - text.trim_end_matches([' ', '\t']).len());
    if trimmed_start >= trimmed_end {
      return None;
    }
    let span = self.text.span(trimmed_start, trimmed_end);
    Some(
      Text {
        span,
        text: span.text(self.text.source()),
      }
      .into(),
    )
  }

  fn push_node(&mut self, start: usize, end: usize, node: Node<'a>) {
    self.flush_text(start);
    self.nodes.push(Some(node));
    self.pos = end;
    self.pending_text_start = end;
  }

  // ---- leaf inlines ----

  /// A backslash escapes the punctuation that follows it, or ends the line as
  /// a hard break.
  fn parse_backslash(&mut self) {
    match self.text.byte(self.pos + 1) {
      Some(b'\n') => {
        let (start, end) = (self.pos, self.pos + 2);
        self.push_node(
          start,
          end,
          HardBreak {
            span: self.text.span(start, end - 1),
          }
          .into(),
        );
      }
      Some(byte) if byte.is_ascii_punctuation() => self.pos += 2,
      _ => self.pos += 1,
    }
  }

  fn parse_code_span(&mut self) {
    let start = self.pos;
    let open_len = self.run_length(start, b'`');
    let mut search = start + open_len;
    while search < self.text.len() {
      let Some(offset) = self.text.bytes()[search..].iter().position(|b| *b == b'`') else {
        break;
      };
      let close_start = search + offset;
      let close_len = self.run_length(close_start, b'`');
      if close_len == open_len {
        let end = close_start + close_len;
        let code = strip_code_span_padding(self.text.slice(start + open_len, close_start));
        self.push_node(
          start,
          end,
          Code {
            span: self.text.span(start, end),
            code,
          }
          .into(),
        );
        return;
      }
      search = close_start + close_len;
    }
    // no closing run of the same length, so the backticks are just text
    self.pos = start + open_len;
  }

  fn parse_line_break(&mut self) {
    let text = self.text.str_between(self.pending_text_start, self.pos);
    let trailing_spaces = text.len() - text.trim_end_matches(' ').len();
    let (start, end) = (self.pos, self.pos + 1);
    if trailing_spaces >= 2 {
      let span = self.text.span(self.pos - trailing_spaces, self.pos);
      self.push_node(start, end, HardBreak { span }.into());
    } else {
      // the break is the line ending itself, not the prefixes a container may
      // have contributed around it
      let newline = self.text.abs(start);
      let source = self.text.source();
      let width = if source[newline..].starts_with("\r\n") { 2 } else { 1 };
      self.push_node(
        start,
        end,
        SoftBreak {
          span: Span::new(newline, newline + width),
        }
        .into(),
      );
    }
  }

  /// A `<` starts an autolink, a raw html tag, or nothing at all.
  fn parse_angle(&mut self) {
    let start = self.pos;
    if let Some(end) = links::match_autolink(self.text, start) {
      let span = self.text.span(start, end);
      let inner = self.text.span(start + 1, end - 1);
      let children = match inner.text(self.text.source()) {
        text if inner.len() == end - start - 2 => vec![Text { span: inner, text }.into()],
        // the url wrapped onto another line, which can't be borrowed as one
        // slice, so leave the autolink without children
        _ => Vec::new(),
      };
      self.push_node(start, end, AutoLink { span, children }.into());
      return;
    }
    if let Some(end) = links::match_html_tag(self.text, start) {
      let span = self.text.span(start, end);
      let text = self.text.slice(start, end);
      self.push_node(
        start,
        end,
        Html {
          span,
          text,
          is_block: false,
        }
        .into(),
      );
      return;
    }
    self.pos += 1;
  }

  /// A `$` starts a math span, which is `$$` delimited when it's displayed on
  /// its own line.
  fn parse_math(&mut self) {
    let start = self.pos;
    let is_display = self.text.byte(start + 1) == Some(b'$');
    let delimiter = if is_display { "$$" } else { "$" };
    let content_start = start + delimiter.len();
    // an opening `$` must not be followed by whitespace
    if !is_display
      && matches!(
        self.text.byte(content_start),
        None | Some(b' ') | Some(b'\t') | Some(b'\n')
      )
    {
      self.pos += 1;
      return;
    }

    let mut search = content_start;
    while search < self.text.len() {
      if self.text.byte(search) == Some(b'\\') {
        // only ascii punctuation is escaped, so anything else leaves the
        // backslash as a character of its own
        search += match self.text.byte(search + 1) {
          Some(byte) if byte.is_ascii_punctuation() => 2,
          _ => 1,
        };
        continue;
      }
      if self.text.starts_with_at(search, delimiter) && search > content_start {
        let end = search + delimiter.len();
        let span = self.text.span(start, end);
        let text = self.text.slice(content_start, search);
        let node = if is_display {
          DisplayMath { span, text }.into()
        } else {
          InlineMath { span, text }.into()
        };
        self.push_node(start, end, node);
        return;
      }
      search += 1;
    }
    self.pos += delimiter.len();
  }

  // ---- brackets ----

  fn parse_open_bracket(&mut self) {
    let start = self.pos;
    let is_image = self.text.byte(start) == Some(b'!');
    let end = if is_image { start + 2 } else { start + 1 };

    if !is_image {
      if let Some((footnote_end, name)) =
        links::match_footnote_reference(self.text, start, &self.context.footnote_labels)
      {
        self.push_node(
          start,
          footnote_end,
          FootnoteReference {
            span: self.text.span(start, footnote_end),
            name,
          }
          .into(),
        );
        return;
      }
    }

    self.flush_text(start);
    let node_index = self.nodes.len();
    self.nodes.push(self.create_text(start, end));
    self.delimiters.push(Delimiter {
      kind: if is_image {
        DelimiterKind::Image
      } else {
        DelimiterKind::Link
      },
      node_index,
      start,
      end,
      original_len: end - start,
      can_open: true,
      can_close: false,
      active: true,
    });
    self.pos = end;
    self.pending_text_start = end;
  }

  fn parse_close_bracket(&mut self) {
    let close_start = self.pos;
    let Some(opener_index) = self.last_bracket() else {
      self.pos += 1;
      return;
    };
    if !self.delimiters[opener_index].active {
      self.delimiters.remove(opener_index);
      self.pos += 1;
      return;
    }

    let opener = &self.delimiters[opener_index];
    let (kind, node_index, open_start, content_start) = (opener.kind, opener.node_index, opener.start, opener.end);
    let is_image = kind == DelimiterKind::Image;

    let Some(reference) = links::match_reference(self.text, close_start, content_start, &self.context.link_labels)
    else {
      // not a link after all, so the `[` stays as text
      self.delimiters.remove(opener_index);
      self.pos += 1;
      return;
    };
    let end = reference.end;

    self.flush_text(close_start);
    let span = self.text.span(open_start, end);
    let node = if is_image {
      let alt = self.text.slice(content_start, close_start);
      match reference.kind {
        ReferenceKind::Inline { url, title } => InlineImage {
          span,
          text: alt,
          url,
          title,
        }
        .into(),
        ReferenceKind::Full { label } => ReferenceImage {
          span,
          text: alt,
          reference: label,
        }
        .into(),
        // an image's alt text is kept as it was written, so its name can't
        // stop matching and the empty brackets can stay
        ReferenceKind::Collapsed => ReferenceImage {
          span,
          text: alt,
          reference: Cow::Borrowed(""),
        }
        .into(),
        ReferenceKind::Shortcut => ShortcutImage { span, text: alt }.into(),
      }
    } else {
      // the text within the brackets is the link's content
      self.process_emphasis(opener_index + 1);
      let children: Vec<Node<'a>> = self.nodes.drain(node_index + 1..).flatten().collect();
      let children = merge_adjacent_text(children, self.text.source());
      self.nodes[node_index] = None;
      match reference.kind {
        ReferenceKind::Inline { url, title } => InlineLink {
          span,
          children,
          url,
          title,
        }
        .into(),
        ReferenceKind::Full { label } => ReferenceLink {
          span,
          children,
          reference: label,
          is_collapsed: false,
        }
        .into(),
        // the name of a collapsed reference is its own text, which the
        // formatter may rewrite, so keep the text as the name for it to
        // write out where it can't stay collapsed
        ReferenceKind::Collapsed => ReferenceLink {
          span,
          children,
          reference: self.text.slice(content_start, close_start),
          is_collapsed: true,
        }
        .into(),
        ReferenceKind::Shortcut => ShortcutLink { span, children }.into(),
      }
    };

    if is_image {
      // an image's content isn't parsed, so throw away what was found in it
      self.nodes.truncate(node_index);
    }
    self.delimiters.truncate(opener_index);
    if !is_image {
      // links can't be nested, so no earlier bracket can become one
      for delimiter in self.delimiters.iter_mut() {
        if matches!(delimiter.kind, DelimiterKind::Link) {
          delimiter.active = false;
        }
      }
    }

    self.nodes.push(Some(node));
    self.pos = end;
    self.pending_text_start = end;
  }

  fn last_bracket(&self) -> Option<usize> {
    self
      .delimiters
      .iter()
      .rposition(|d| matches!(d.kind, DelimiterKind::Link | DelimiterKind::Image))
  }

  // ---- emphasis ----

  fn parse_emphasis_delimiter(&mut self, byte: u8) {
    let start = self.pos;
    let length = self.run_length(start, byte);
    let end = start + length;
    // strikethrough is written with one or two tildes, so a longer run is
    // text of its own
    if byte == b'~' && length > 2 {
      self.pos = end;
      return;
    }
    let (can_open, can_close) = self.flanking(start, end, byte);

    if !can_open && !can_close {
      self.pos = end;
      return;
    }

    self.flush_text(start);
    let node_index = self.nodes.len();
    self.nodes.push(self.create_text(start, end));
    self.delimiters.push(Delimiter {
      kind: DelimiterKind::Emphasis(byte),
      node_index,
      start,
      end,
      original_len: length,
      can_open,
      can_close,
      active: true,
    });
    self.pos = end;
    self.pending_text_start = end;
  }

  /// Whether a delimiter run can open and/or close emphasis, which comes down
  /// to what surrounds it (the "flanking" rules of the CommonMark spec).
  fn flanking(&self, start: usize, end: usize, byte: u8) -> (bool, bool) {
    let before = self.text.char_before(start);
    let after = self.text.char_at(end);
    let before_whitespace = before.is_none_or(is_markdown_whitespace);
    let after_whitespace = after.is_none_or(is_markdown_whitespace);
    let before_punctuation = before.is_some_and(is_markdown_punctuation);
    let after_punctuation = after.is_some_and(is_markdown_punctuation);

    let left_flanking = !after_whitespace && (!after_punctuation || before_whitespace || before_punctuation);
    let right_flanking = !before_whitespace && (!before_punctuation || after_whitespace || after_punctuation);

    // `_` and a lone `~` can't be used within a word, while `*` and `~~` can
    let within_word = byte == b'*' || byte == b'~' && end - start == 2;
    if within_word {
      (left_flanking, right_flanking)
    } else {
      (
        left_flanking && (!right_flanking || before_punctuation),
        right_flanking && (!left_flanking || after_punctuation),
      )
    }
  }

  /// Pairs up the emphasis delimiters at or after `bottom`, wrapping the nodes
  /// they surround.
  fn process_emphasis(&mut self, bottom: usize) {
    let mut closer_index = bottom;
    while closer_index < self.delimiters.len() {
      let closer = &self.delimiters[closer_index];
      let DelimiterKind::Emphasis(byte) = closer.kind else {
        closer_index += 1;
        continue;
      };
      if !closer.can_close {
        closer_index += 1;
        continue;
      }

      let Some(opener_index) = self.find_opener(bottom, closer_index, byte) else {
        // a closer with no opener can still open emphasis of its own
        if !self.delimiters[closer_index].can_open {
          self.delimiters[closer_index].can_close = false;
        }
        closer_index += 1;
        continue;
      };

      closer_index = self.wrap_emphasis(opener_index, closer_index, byte);
    }

    self.delimiters.truncate(bottom);
  }

  fn find_opener(&self, bottom: usize, closer_index: usize, byte: u8) -> Option<usize> {
    let closer = &self.delimiters[closer_index];
    let closer_len = closer.original_len;
    for index in (bottom..closer_index).rev() {
      let opener = &self.delimiters[index];
      match opener.kind {
        // a bracket that never became a link is nothing but the text of it,
        // which emphasis reaches straight across
        DelimiterKind::Link | DelimiterKind::Image => continue,
        DelimiterKind::Emphasis(opener_byte) => {
          if opener_byte != byte || !opener.can_open {
            continue;
          }
          let opener_len = opener.original_len;
          // the "rule of three": when a delimiter is both an opener and a
          // closer, the sum of the run lengths can't be a multiple of three
          // unless both are
          let both_ways = opener.can_close || closer.can_close && closer.can_open;
          if both_ways
            && (opener_len + closer_len).is_multiple_of(3)
            && (!opener_len.is_multiple_of(3) || !closer_len.is_multiple_of(3))
          {
            continue;
          }
          return Some(index);
        }
      }
    }
    None
  }

  /// Wraps the nodes between the two delimiters, returning the index to keep
  /// looking for closers at.
  fn wrap_emphasis(&mut self, opener_index: usize, closer_index: usize, byte: u8) -> usize {
    let opener_len = self.delimiters[opener_index].end - self.delimiters[opener_index].start;
    let closer_len = self.delimiters[closer_index].end - self.delimiters[closer_index].start;
    let use_len = match byte {
      // strikethrough only pairs runs of the same length
      b'~' => {
        if opener_len != closer_len {
          return closer_index + 1;
        }
        opener_len
      }
      _ if opener_len >= 2 && closer_len >= 2 => 2,
      _ => 1,
    };

    let content_start = self.delimiters[opener_index].end;
    let content_end = self.delimiters[closer_index].start;
    let (open_node, close_node) = (
      self.delimiters[opener_index].node_index,
      self.delimiters[closer_index].node_index,
    );

    let kind = match (byte, use_len) {
      (b'~', _) => TextDecorationKind::Strikethrough,
      (_, 2) => TextDecorationKind::Strong,
      _ => TextDecorationKind::Emphasis,
    };
    let span = Span::new(
      self.text.abs(content_start - use_len),
      self.text.abs(content_end + use_len),
    );
    let children: Vec<Node<'a>> = self.nodes[open_node + 1..close_node]
      .iter_mut()
      .filter_map(|slot| slot.take())
      .collect();
    let children = merge_adjacent_text(children, self.text.source());
    let decoration: Node<'a> = TextDecoration { span, kind, children }.into();

    // shrink the delimiters by what was used, dropping the ones left empty
    self.delimiters[opener_index].end -= use_len;
    self.delimiters[closer_index].start += use_len;
    self.nodes[open_node] = {
      let delimiter = &self.delimiters[opener_index];
      self.create_text(delimiter.start, delimiter.end)
    };
    self.nodes[close_node] = {
      let delimiter = &self.delimiters[closer_index];
      self.create_text(delimiter.start, delimiter.end)
    };
    // the wrapper takes the place of the content it swallowed
    self.nodes[close_node - 1] = Some(decoration);
    if close_node - 1 == open_node {
      // the delimiters were adjacent, so nothing was swallowed and the opener
      // has to be kept somewhere else
      let opener_text = {
        let delimiter = &self.delimiters[opener_index];
        self.create_text(delimiter.start, delimiter.end)
      };
      if let Some(opener_text) = opener_text {
        self.nodes.insert(open_node, Some(opener_text));
        // the opener's text stays where it was and everything after it moved
        self.shift_node_indexes(open_node + 1, 1);
      }
    }

    // remove the delimiters between the pair, which can no longer match
    self.delimiters.drain(opener_index + 1..closer_index);
    let mut closer_index = opener_index + 1;
    if self.delimiters[closer_index].start == self.delimiters[closer_index].end {
      self.delimiters.remove(closer_index);
      closer_index = opener_index + 1;
    }
    if self.delimiters[opener_index].start == self.delimiters[opener_index].end {
      self.delimiters.remove(opener_index);
      closer_index -= 1;
    }
    closer_index
  }

  fn shift_node_indexes(&mut self, from: usize, by: usize) {
    for delimiter in self.delimiters.iter_mut() {
      if delimiter.node_index >= from {
        delimiter.node_index += by;
      }
    }
  }

  // ---- helpers ----

  fn run_length(&self, start: usize, byte: u8) -> usize {
    self.text.bytes()[start..].iter().take_while(|b| **b == byte).count()
  }
}

/// Joins the text nodes that ended up beside each other, which happens when a
/// delimiter turns out not to be one after all.
fn merge_adjacent_text<'a>(nodes: Vec<Node<'a>>, source: &'a str) -> Vec<Node<'a>> {
  if !nodes.windows(2).any(|pair| is_mergeable(&pair[0], &pair[1], source)) {
    return nodes;
  }

  let mut result: Vec<Node<'a>> = Vec::with_capacity(nodes.len());
  for node in nodes {
    match result.last_mut() {
      Some(Node::Text(previous)) if is_mergeable_text(previous, &node, source) => {
        previous.span.end = node.span().end;
        previous.text = previous.span.text(source);
      }
      _ => result.push(node),
    }
  }
  result
}

fn is_mergeable(previous: &Node<'_>, node: &Node<'_>, source: &str) -> bool {
  match previous {
    Node::Text(previous) => is_mergeable_text(previous, node, source),
    _ => false,
  }
}

/// Whether the node is text that only spaces separate from the text before it,
/// so that merging the two keeps the source text they cover unchanged.
fn is_mergeable_text(previous: &Text<'_>, node: &Node<'_>, source: &str) -> bool {
  if !matches!(node, Node::Text(_)) {
    return false;
  }
  let span = node.span();
  span.start >= previous.span.end
    && source[previous.span.end..span.start]
      .bytes()
      .all(|b| matches!(b, b' ' | b'\t'))
}

/// A code span's content has a space stripped off each end when it has one on
/// both and isn't all spaces.
fn strip_code_span_padding(code: Cow<'_, str>) -> Cow<'_, str> {
  let has_padding = code.starts_with(' ') && code.ends_with(' ') && !code.trim_matches(' ').is_empty();
  if !has_padding {
    return code;
  }
  match code {
    Cow::Borrowed(text) => Cow::Borrowed(&text[1..text.len() - 1]),
    Cow::Owned(text) => Cow::Owned(text[1..text.len() - 1].to_string()),
  }
}

fn is_markdown_whitespace(c: char) -> bool {
  c.is_whitespace()
}

fn is_markdown_punctuation(c: char) -> bool {
  c.is_ascii_punctuation()
    || matches!(
      unicode_category(c),
      UnicodeCategory::Punctuation | UnicodeCategory::Symbol
    )
}

/// The coarse unicode categories the flanking rules care about.
enum UnicodeCategory {
  Punctuation,
  Symbol,
  Other,
}

/// A rough categorization that avoids pulling in a unicode table, which is
/// enough because the flanking rules only distinguish punctuation and symbols
/// from everything else.
fn unicode_category(c: char) -> UnicodeCategory {
  if c.is_ascii() {
    return UnicodeCategory::Other;
  }
  match c {
    '\u{00A1}' | '\u{00A7}' | '\u{00AB}' | '\u{00B6}' | '\u{00B7}' | '\u{00BB}' | '\u{00BF}' => {
      UnicodeCategory::Punctuation
    }
    '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{205E}' | '\u{3001}'..='\u{3003}' | '\u{FF01}'..='\u{FF0F}' => {
      UnicodeCategory::Punctuation
    }
    '\u{00A2}'..='\u{00A6}' | '\u{00A8}' | '\u{00A9}' | '\u{00AC}' | '\u{00AE}'..='\u{00B1}' => UnicodeCategory::Symbol,
    _ => UnicodeCategory::Other,
  }
}
